mod api_client;
mod app;
mod config;
mod error;
mod events;
mod help;
mod reports;
mod stats;
mod tui;
mod ui;

use crate::{
    api_client::{
        ApiClient, ApiClientLike, OverallEvaluation, format_evaluation_display, parse_evaluation,
    },
    app::{App, ViewMode},
    error::AppError,
    events::AppAction,
    stats::EvaluationScores,
};
use std::env;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    let mut app = App::default();

    let api_client = authenticate().await?;
    app.api_client = Some(api_client);

    let mut tui = tui::init()?;

    // Main loop
    while !app.should_quit {
        tui.draw(|frame| ui::render(&mut app, frame))?;

        if let Some(result) = events::handle_events(&mut app).await? {
            match result {
                AppAction::StartTraining => {
                    app.view_mode = ViewMode::Normal;
                    app.status_message = "Generating text...".to_string();
                    tui.draw(|frame| ui::render(&mut app, frame))?;

                    generate_text_for_training(&mut app).await;
                }
                AppAction::Evaluate => {
                    app.is_evaluating = true;
                    app.status_message = "Evaluating your summary...".to_string();
                    tui.draw(|frame| ui::render(&mut app, frame))?;

                    if let Some(client) = &app.api_client {
                        // Get summary from text_area_state
                        let summary = app.text_area_state.value().to_string();

                        match client
                            .evaluate_summary(app.original_text.clone(), summary)
                            .await
                        {
                            Ok(evaluation) => match parse_evaluation(&evaluation) {
                                Ok(parsed) => {
                                    app.evaluation_passed =
                                        matches!(parsed.overall, OverallEvaluation::Pass);
                                    app.evaluation_text = format_evaluation_display(&parsed);
                                    app.show_evaluation_overlay = true;
                                    app.evaluation_overlay_scroll = 0;
                                    app.is_evaluating = false;
                                    app.status_message =
                                        "評価が完了しました。'e'で切替、'n'で次へ。".to_string();

                                    let scores = EvaluationScores {
                                        appropriate: parsed.appropriate,
                                        importance: parsed.importance,
                                        conciseness: parsed.conciseness,
                                        accuracy: parsed.accuracy,
                                        improvement1: parsed.improvement1,
                                        improvement2: parsed.improvement2,
                                        improvement3: parsed.improvement3,
                                        overall_passed: app.evaluation_passed,
                                    };

                                    app.stats.add_result_with_evaluation(
                                        app.evaluation_passed,
                                        Some(scores),
                                    );
                                    if let Err(e) = app.stats.save() {
                                        app.status_message =
                                            format!("警告: 統計の保存に失敗しました: {}", e);
                                        eprintln!("Failed to save stats: {}", e);
                                    }
                                }
                                Err(_) => {
                                    app.evaluation_text = "評価結果の形式が不正です".to_string();
                                    app.evaluation_passed = false;
                                    app.show_evaluation_overlay = true;
                                    app.evaluation_overlay_scroll = 0;
                                    app.is_evaluating = false;
                                    app.status_message = "評価結果の形式が不正です".to_string();
                                }
                            },
                            Err(e) => {
                                app.evaluation_text = format!("Error: {}", e);
                                app.evaluation_passed = false;
                                app.show_evaluation_overlay = true;
                                app.evaluation_overlay_scroll = 0;
                                app.is_evaluating = false;
                                app.status_message = "Error occurred.".to_string();
                            }
                        }
                    }
                }
                AppAction::NextTraining => {
                    // Reset all evaluation-related state
                    app.show_evaluation_overlay = false;
                    app.evaluation_text.clear();
                    app.evaluation_passed = false;
                    app.text_area_state = App::new_text_area_state();
                    app.original_text_scroll = 0;
                    app.evaluation_overlay_scroll = 0;
                    app.status_message = "Generating new text...".to_string();
                    tui.draw(|frame| ui::render(&mut app, frame))?;

                    generate_text_for_training(&mut app).await;
                }
            }
        }
    }

    tui::restore()?;
    Ok(())
}

/// Generate text using the API client and update app state
async fn generate_text_for_training(app: &mut App) {
    if let Some(client) = &app.api_client {
        match client.generate_text(app.generate_text_prompt()).await {
            Ok(text) => {
                app.original_text = text;
                app.status_message = "Normal Mode. Press 'i' to edit.".to_string();
            }
            Err(e) => {
                app.original_text = format!("Failed to generate text: {}", e);
                app.status_message = "Error".to_string();
            }
        }
    }
}

async fn authenticate() -> Result<Box<dyn ApiClientLike>, AppError> {
    if let Some(key) = config::load_api_key()?
        && !key.is_empty()
    {
        let client = ApiClient::new(key);
        if client.validate_credentials().await.is_ok() {
            return Ok(Box::new(client));
        }
    }

    if let Ok(key) = env::var("GROQ_API_KEY")
        && !key.is_empty()
    {
        let client = ApiClient::new(key.clone());
        if client.validate_credentials().await.is_ok() {
            if config::save_api_key(&key).is_err() {
                // Ignore saving error
            }
            return Ok(Box::new(client));
        }
    }
    Err(AppError::InvalidApiKey)
}
