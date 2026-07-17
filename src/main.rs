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
    error::AppError,
    evaluation::{OverallEvaluation, format_evaluation_display, parse_evaluation},
    events::AppAction,
    models::EvaluationScores,
};

#[tokio::main]
async fn main() -> Result<(), AppError> {
    let mut app = App::default();

    let api_client = authenticate().await?;
    app.api_client = Some(api_client);

    let mut tui = tui::init()?;

    while !app.should_quit {
        tui.draw(|frame| ui::render(&mut app, frame))?;

        if let Some(action) = events::handle_events(&mut app)? {
            match action {
                AppAction::StartTraining => handle_start_training(&mut app, &mut tui).await?,
                AppAction::Evaluate => handle_evaluate(&mut app, &mut tui).await?,
                AppAction::NextTraining => handle_next_training(&mut app, &mut tui).await?,
            }
        }
    }

    tui::restore()?;
    Ok(())
}

async fn generate_text_for_training(app: &mut App) {
    if let Some(client) = &app.api_client {
        match client.generate_text(&app.generate_text_prompt()).await {
            Ok(text) => app.apply_generated_text(text),
            Err(e) => app.apply_generation_error(&e),
        }
    }
}

async fn handle_start_training(app: &mut App, tui: &mut tui::Tui) -> Result<(), AppError> {
    app.begin_training_generation(false);
    tui.draw(|frame| ui::render(app, frame))?;

    generate_text_for_training(app).await;
    Ok(())
}

async fn handle_evaluate(app: &mut App, tui: &mut tui::Tui) -> Result<(), AppError> {
    app.begin_evaluation();
    tui.draw(|frame| ui::render(app, frame))?;

    let Some(client) = &app.api_client else {
        return Ok(());
    };

    let summary = app.text_area_state.value().clone();

    match client.evaluate_summary(&app.original_text, &summary).await {
        Ok(evaluation) => match parse_evaluation(&evaluation) {
            Ok(parsed) => {
                let evaluation_passed = matches!(parsed.overall, OverallEvaluation::Pass);
                let evaluation_text = format_evaluation_display(&parsed);
                let scores = EvaluationScores {
                    appropriate: parsed.appropriate,
                    importance: parsed.importance,
                    conciseness: parsed.conciseness,
                    accuracy: parsed.accuracy,
                    improvement1: parsed.improvement1,
                    improvement2: parsed.improvement2,
                    improvement3: parsed.improvement3,
                    overall_passed: evaluation_passed,
                };

                app.finish_evaluation(evaluation_text, evaluation_passed);

                app.stats
                    .add_result_with_evaluation(evaluation_passed, Some(scores));
                if let Err(e) = app.stats.save() {
                    app.status_message = format!("警告: 統計の保存に失敗しました: {e}");
                    eprintln!("統計の保存に失敗しました: {e}");
                }
            }
            Err(_) => app.fail_evaluation_format(),
        },
        Err(e) => app.fail_evaluation_request(&e),
    }
    Ok(())
}

async fn handle_next_training(app: &mut App, tui: &mut tui::Tui) -> Result<(), AppError> {
    app.prepare_next_training();
    tui.draw(|frame| ui::render(app, frame))?;

    generate_text_for_training(app).await;
    Ok(())
}

async fn authenticate() -> Result<ApiClient, AppError> {
    if let Some(key) = config::load_api_key()?
        && let Some(client) = authenticate_with_key(&key).await
    {
        return Ok(client);
    }
    Err(AppError::InvalidApiKey)
}

async fn authenticate_with_key(key: &str) -> Option<ApiClient> {
    if key.is_empty() {
        return None;
    }

    let client = ApiClient::new(key.to_string());
    client.validate_credentials().await.ok()?;
    Some(client)
}
