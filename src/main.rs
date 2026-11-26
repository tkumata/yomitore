mod api_client;
mod app;
mod config;
mod error;
mod evaluation;
mod events;
mod help;
mod models;
mod reports;
mod stats;
mod stats_analysis;
mod stats_store;
mod tui;
mod ui;

use crate::{
    api_client::ApiClient,
    app::{App, SUMMARY_LINE_ENDING},
    config::ApiKeys,
    error::AppError,
    evaluation::{
        JevEvaluation, Judgment, format_jev_evaluation_display, parse_improvements,
        parse_jev_response,
    },
    events::AppAction,
    models::EvaluationScores,
};
use std::mem::DropGuard;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    let mut app = App::default();
    match stats_store::load() {
        Ok(stats) => app.stats = stats,
        Err(error) => {
            app.stats_load_error = Some(format!(
                "成績履歴を読み込めません。保存を停止しました。({error})"
            ));
        }
    }
    let http_client = ApiClient::http_client()?;
    if let Ok(keys) = config::load_api_keys() {
        apply_key_state(&mut app, &http_client, keys);
    } else {
        app.api_client =
            config::groq_api_key_from_env().map(|key| ApiClient::new(&http_client, key));
        app.settings.groq_from_env = app.api_client.is_some();
        app.status_message =
            "設定を読み込めませんでした。config.toml を確認してください。".to_string();
    }
    let mut tui = tui::init()?;
    let restore_guard = DropGuard::new((), tui::restore_on_drop);

    while !app.should_quit {
        tui.draw(|frame| ui::render(&mut app, frame))?;

        if let Some(action) = events::handle_events(&mut app)? {
            match action {
                AppAction::StartTraining => {
                    handle_start_training(&mut app, &mut tui, &http_client).await?;
                }
                AppAction::Evaluate => handle_evaluate(&mut app, &mut tui).await?,
                AppAction::NextTraining => handle_next_training(&mut app, &mut tui).await?,
                AppAction::SaveSettings => save_settings(&mut app, &http_client),
                AppAction::RetrySave => save_pending_evaluation(&mut app),
            }
        }
    }

    DropGuard::dismiss(restore_guard);
    tui::restore()?;
    Ok(())
}

fn apply_key_state(app: &mut App, http_client: &reqwest::Client, keys: ApiKeys) {
    app.settings.groq_from_env = keys.groq_from_env;
    app.settings.groq_saved = keys.groq_saved;
    app.settings.jev_saved = keys.jev.is_some();
    app.api_client = keys.groq.map(|key| ApiClient::new(http_client, key));
    app.jev_api_client = keys.jev.map(|key| ApiClient::new(http_client, key));
}

fn save_settings(app: &mut App, http_client: &reqwest::Client) {
    let groq = app.settings.groq_input.trim();
    let jev = app.settings.jev_input.trim();
    if config::save_api_keys(
        (!groq.is_empty()).then_some(groq),
        (!jev.is_empty()).then_some(jev),
    )
    .is_err()
    {
        app.status_message = "API キーの保存に失敗しました。".to_string();
        return;
    }
    if let Ok(keys) = config::load_api_keys() {
        apply_key_state(app, http_client, keys);
        app.settings.groq_input.clear();
        app.settings.jev_input.clear();
        app.return_from_settings();
        app.status_message = "API キーを保存しました。".to_string();
    } else {
        app.status_message =
            "設定の保存状態を確認できません。config.toml を確認してください。".to_string();
    }
}

async fn generate_text_for_training(app: &mut App) {
    let Some(client) = &app.api_client else {
        app.apply_generation_error(&"Groq API キーを設定画面 (s) で設定してください。");
        return;
    };
    match client.generate_text(&app.generate_text_prompt()).await {
        Ok(text) => app.apply_generated_text(text),
        Err(error) => app.apply_generation_error(&safe_api_error_message("Groq", &error)),
    }
}

async fn handle_start_training(
    app: &mut App,
    tui: &mut tui::Tui,
    http_client: &reqwest::Client,
) -> Result<(), AppError> {
    let key = match config::load_groq_api_key() {
        Ok(Some(key)) => key,
        Ok(None) => {
            app.status_message = "Groq API キーを設定画面 (s) で設定してください。".to_string();
            return Ok(());
        }
        Err(_) => {
            app.status_message =
                "設定を読み込めません。config.toml を確認してください。".to_string();
            return Ok(());
        }
    };
    let client = ApiClient::new(http_client, key);
    if let Err(error) = client.validate_credentials().await {
        app.status_message = safe_api_error_message("Groq", &error);
        return Ok(());
    }
    app.api_client = Some(client);
    if let Ok(keys) = config::load_api_keys() {
        app.settings.groq_from_env = keys.groq_from_env;
        app.settings.groq_saved = keys.groq_saved;
        app.settings.jev_saved = keys.jev.is_some();
        app.jev_api_client = keys.jev.map(|key| ApiClient::new(http_client, key));
    } else {
        app.jev_api_client = None;
    }
    app.begin_training_generation(false);
    tui.draw(|frame| ui::render(app, frame))?;
    generate_text_for_training(app).await;
    Ok(())
}

async fn handle_evaluate(app: &mut App, tui: &mut tui::Tui) -> Result<(), AppError> {
    app.begin_evaluation();
    tui.draw(|frame| ui::render(app, frame))?;
    let Some(original_text) = app.original_text.as_deref() else {
        app.fail_evaluation_request(&"評価に必要な原文がありません。");
        return Ok(());
    };
    let Some(jev_client) = &app.jev_api_client else {
        app.fail_evaluation_request(&"Jev API キーを設定画面 (s) で設定してください。");
        return Ok(());
    };
    let Some(groq_client) = &app.api_client else {
        app.fail_evaluation_request(&"Groq API キーを設定画面 (s) で設定してください。");
        return Ok(());
    };
    let summary = app.summary_input.lines().join(SUMMARY_LINE_ENDING);
    match evaluate_responses(
        jev_client.evaluate_with_jev(original_text, &summary),
        groq_client.evaluate_summary(original_text, &summary),
    )
    .await
    {
        Ok((jev_result, improvements)) => record_evaluation(app, jev_result, improvements),
        Err(EvaluationFailure::Request(service, error)) => {
            app.fail_evaluation_request(&safe_api_error_message(service, &error));
        }
        Err(EvaluationFailure::Format) => app.fail_evaluation_format(),
    }
    Ok(())
}

enum EvaluationFailure {
    Request(&'static str, AppError),
    Format,
}

async fn evaluate_responses(
    jev: impl Future<Output = Result<serde_json::Value, AppError>>,
    groq: impl Future<Output = Result<String, AppError>>,
) -> Result<(JevEvaluation, [String; 3]), EvaluationFailure> {
    let response = jev
        .await
        .map_err(|error| EvaluationFailure::Request("Jev", error))?;
    let evaluation = parse_jev_response(&response).map_err(|_| EvaluationFailure::Format)?;
    let text = groq
        .await
        .map_err(|error| EvaluationFailure::Request("Groq", error))?;
    let improvements = parse_improvements(&text).map_err(|_| EvaluationFailure::Format)?;
    Ok((evaluation, improvements))
}

fn safe_api_error_message(service: &str, error: &AppError) -> String {
    match error {
        AppError::InvalidApiKey => {
            format!("{service} API キーが無効です。設定画面 (s) で確認してください。")
        }
        AppError::ApiClientInitialization => error.to_string(),
        AppError::HttpStatus(status) => format!("{service} API が HTTP {status} を返しました。"),
        AppError::ApiError(error) => match (error.status(), error.is_timeout()) {
            (Some(status), _) => format!("{service} API が HTTP {status} を返しました。"),
            (None, true) => format!("{service} API の通信がタイムアウトしました。"),
            (None, false) => format!("{service} API との通信に失敗しました。"),
        },
        AppError::ResponseParseError(_) => {
            format!("{service} API の JSON 応答を解析できませんでした。")
        }
        AppError::NoChoicesInResponse => format!("{service} API の応答に結果がありません。"),
        AppError::EmptyChatContent => format!("{service} API の応答本文が空です。"),
        AppError::IoError(_) => "設定の読み書きに失敗しました。".to_string(),
    }
}

fn record_evaluation(app: &mut App, jev_result: JevEvaluation, improvements: [String; 3]) {
    let total_score = jev_result.total_score();
    let judgment = Judgment::from_scores(total_score, jev_result.critical_error);
    let display = format_jev_evaluation_display(&jev_result, total_score, judgment, &improvements);
    let scores = EvaluationScores {
        scores: jev_result,
        total_score,
        judgment,
        improvements,
    };
    app.finish_evaluation(display, judgment.outcome());
    if app.stats_load_error.is_none() {
        app.pending_evaluation = Some(scores);
        save_pending_evaluation(app);
    }
}

fn save_pending_evaluation(app: &mut App) {
    if let Some(scores) = &app.pending_evaluation {
        match stats_store::record(scores) {
            Ok(stats) => {
                app.stats = stats;
                app.pending_evaluation = None;
                app.status_message = app::STATUS_EVALUATED.to_string();
            }
            Err(error) => {
                app.status_message = format!("成績の保存に失敗しました。({error})");
            }
        }
    }
}

async fn handle_next_training(app: &mut App, tui: &mut tui::Tui) -> Result<(), AppError> {
    app.prepare_next_training();
    tui.draw(|frame| ui::render(app, frame))?;
    generate_text_for_training(app).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, future::ready};

    #[tokio::test]
    async fn only_complete_api_responses_produce_a_recordable_evaluation() {
        let groq_called = Cell::new(false);
        let result = evaluate_responses(ready(Err(AppError::InvalidApiKey)), async {
            groq_called.set(true);
            Ok(String::new())
        })
        .await;
        assert!(matches!(
            result,
            Err(EvaluationFailure::Request("Jev", AppError::InvalidApiKey))
        ));
        assert!(!groq_called.get());

        let result = evaluate_responses(ready(Ok(serde_json::json!({"answers": {}}))), async {
            groq_called.set(true);
            Ok(String::new())
        })
        .await;
        assert!(matches!(result, Err(EvaluationFailure::Format)));
        assert!(!groq_called.get());

        let result = evaluate_responses(
            ready(Ok(evaluation::tests::response())),
            ready(Err(AppError::InvalidApiKey)),
        )
        .await;
        assert!(matches!(
            result,
            Err(EvaluationFailure::Request("Groq", AppError::InvalidApiKey))
        ));

        let result = evaluate_responses(
            ready(Ok(evaluation::tests::response())),
            ready(Ok("改善点1: 不足".to_owned())),
        )
        .await;
        assert!(matches!(result, Err(EvaluationFailure::Format)));

        let result = evaluate_responses(
            ready(Ok(evaluation::tests::response())),
            ready(Ok(
                "改善点1: 不足\n改善点2: 冗長\n改善点3: 不正確".to_owned()
            )),
        )
        .await;
        assert!(result.is_ok());
    }
}
