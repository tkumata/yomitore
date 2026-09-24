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
mod tui;
mod ui;

use crate::{
    api_client::ApiClient,
    app::App,
    config::ApiKeys,
    error::AppError,
    evaluation::{
        JevEvaluation, format_jev_evaluation_display, parse_improvements, parse_jev_response,
    },
    events::AppAction,
    models::EvaluationScores,
};

#[tokio::main]
async fn main() -> Result<(), AppError> {
    let mut app = App::default();
    if let Ok(keys) = config::load_api_keys() {
        apply_key_state(&mut app, keys);
    } else {
        app.api_client = config::groq_api_key_from_env().map(ApiClient::new);
        app.settings.groq_from_env = app.api_client.is_some();
        app.status_message =
            "設定を読み込めませんでした。config.toml を確認してください。".to_string();
    }
    let mut tui = tui::init()?;

    while !app.should_quit {
        tui.draw(|frame| ui::render(&mut app, frame))?;

        if let Some(action) = events::handle_events(&mut app)? {
            match action {
                AppAction::StartTraining => handle_start_training(&mut app, &mut tui).await?,
                AppAction::Evaluate => handle_evaluate(&mut app, &mut tui).await?,
                AppAction::NextTraining => handle_next_training(&mut app, &mut tui).await?,
                AppAction::SaveSettings => save_settings(&mut app),
            }
        }
    }

    tui::restore()?;
    Ok(())
}

fn apply_key_state(app: &mut App, keys: ApiKeys) {
    app.settings.groq_from_env = keys.groq_from_env;
    app.settings.groq_saved = keys.groq_saved;
    app.settings.jev_saved = keys.jev.is_some();
    app.api_client = keys.groq.map(ApiClient::new);
    app.jev_api_client = keys.jev.map(ApiClient::new);
}

fn save_settings(app: &mut App) {
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
        apply_key_state(app, keys);
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
    if let Some(client) = &app.api_client {
        match client.generate_text(&app.generate_text_prompt()).await {
            Ok(text) => app.apply_generated_text(text),
            Err(error) => app.apply_generation_error(&safe_api_error_message("Groq", &error)),
        }
    }
}

async fn handle_start_training(app: &mut App, tui: &mut tui::Tui) -> Result<(), AppError> {
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
    let client = ApiClient::new(key);
    if let Err(error) = client.validate_credentials().await {
        app.status_message = safe_api_error_message("Groq", &error);
        return Ok(());
    }
    app.api_client = Some(client);
    if let Ok(keys) = config::load_api_keys() {
        app.settings.groq_from_env = keys.groq_from_env;
        app.settings.groq_saved = keys.groq_saved;
        app.settings.jev_saved = keys.jev.is_some();
        app.jev_api_client = keys.jev.map(ApiClient::new);
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
    let Some(jev_client) = &app.jev_api_client else {
        app.fail_evaluation_request(&"Jev API キーを設定画面 (s) で設定してください。");
        return Ok(());
    };
    let Some(groq_client) = &app.api_client else {
        app.fail_evaluation_request(&"Groq API キーを設定画面 (s) で設定してください。");
        return Ok(());
    };
    let summary = app.text_area_state.value().clone();
    let response = match jev_client
        .evaluate_with_jev(&app.original_text, &summary)
        .await
    {
        Ok(response) => response,
        Err(AppError::InvalidApiKey) => {
            app.fail_evaluation_request(
                &"Jev API キーが無効です。設定画面 (s) で確認してください。",
            );
            return Ok(());
        }
        Err(error) => {
            app.fail_evaluation_request(&safe_api_error_message("Jev", &error));
            return Ok(());
        }
    };
    let Ok(jev_result) = parse_jev_response(&response) else {
        app.fail_evaluation_format();
        return Ok(());
    };
    let text = match groq_client
        .evaluate_summary(&app.original_text, &summary)
        .await
    {
        Ok(text) => text,
        Err(error) => {
            app.fail_evaluation_request(&safe_api_error_message("Groq", &error));
            return Ok(());
        }
    };
    let Ok(improvements) = parse_improvements(&text) else {
        app.fail_evaluation_format();
        return Ok(());
    };
    record_evaluation(app, &jev_result, &improvements);
    Ok(())
}

fn safe_api_error_message(service: &str, error: &AppError) -> String {
    match error {
        AppError::InvalidApiKey => {
            format!("{service} API キーが無効です。設定画面 (s) で確認してください。")
        }
        AppError::ApiError(error) => match (error.status(), error.is_timeout()) {
            (Some(status), _) => format!("{service} API が HTTP {status} を返しました。"),
            (None, true) => format!("{service} API の通信がタイムアウトしました。"),
            (None, false) => format!("{service} API との通信に失敗しました。"),
        },
        AppError::ResponseParseError(_) => {
            format!("{service} API の JSON 応答を解析できませんでした。")
        }
        AppError::NoChoicesInResponse => format!("{service} API の応答に結果がありません。"),
        AppError::IoError(_) => "設定の読み書きに失敗しました。".to_string(),
    }
}

fn record_evaluation(app: &mut App, jev_result: &JevEvaluation, improvements: &[String; 3]) {
    let overall_passed = jev_result.overall == evaluation::OverallEvaluation::Pass;
    let total_score = jev_result.total_score();
    let display = format_jev_evaluation_display(jev_result, improvements);
    let scores = EvaluationScores {
        appropriate: jev_result.appropriate,
        importance: JevEvaluation::displayed_score(jev_result.importance),
        conciseness: JevEvaluation::displayed_score(jev_result.conciseness),
        accuracy: JevEvaluation::displayed_score(jev_result.accuracy),
        improvement1: improvements.first().cloned().unwrap_or_default(),
        improvement2: improvements.get(1).cloned().unwrap_or_default(),
        improvement3: improvements.get(2).cloned().unwrap_or_default(),
        overall_passed,
        total_score: Some(total_score),
    };
    app.finish_evaluation(display, overall_passed);
    app.stats
        .add_result_with_evaluation(overall_passed, Some(scores));
    if app.stats.save().is_err() {
        app.status_message = "警告: 統計の保存に失敗しました。".to_string();
    }
}

async fn handle_next_training(app: &mut App, tui: &mut tui::Tui) -> Result<(), AppError> {
    app.prepare_next_training();
    tui.draw(|frame| ui::render(app, frame))?;
    generate_text_for_training(app).await;
    Ok(())
}
