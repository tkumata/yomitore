mod app;
mod api_client;
mod config;
mod error;
mod tui;
mod ui;

use crate::{api_client::ApiClient, app::App, error::AppError};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use std::{env, time::Duration};

#[tokio::main]
async fn main() -> Result<(), AppError> {
    let mut app = App::default();

    let api_client = authenticate().await?;
    app.api_client = Some(api_client);

    let mut tui = tui::init()?;

    if app.api_client.is_some() {
        app.status_message = "Generating text...".to_string();
        tui.draw(|frame| ui::render(&mut app, frame))?;

        let generation_prompt = "日本語の公的文書のようなお堅い文章を400文字程度で生成してください。";
        if let Some(client) = &app.api_client {
            match client.generate_text(generation_prompt).await {
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

    // Main loop
    while !app.should_quit {
        tui.draw(|frame| ui::render(&mut app, frame))?;

        if let Some(result) = handle_events(&mut app).await? {
            match result {
                AppAction::Evaluate => {
                    app.is_evaluating = true;
                    app.status_message = "Evaluating your summary...".to_string();
                    tui.draw(|frame| ui::render(&mut app, frame))?;

                    if let Some(client) = &app.api_client {
                        match client.evaluate_summary(&app.original_text, &app.summary_input).await {
                            Ok(evaluation) => {
                                app.evaluation_text = evaluation;
                                app.show_evaluation = true;
                                app.is_evaluating = false;
                                app.status_message = "Evaluation complete. Press 'n' for next training.".to_string();
                            }
                            Err(e) => {
                                app.evaluation_text = format!("Error: {}", e);
                                app.show_evaluation = true;
                                app.is_evaluating = false;
                                app.status_message = "Error occurred.".to_string();
                            }
                        }
                    }
                }
                AppAction::NextTraining => {
                    app.show_evaluation = false;
                    app.evaluation_text.clear();
                    app.summary_input.clear();
                    app.cursor_position = 0;
                    app.original_text_scroll = 0;
                    app.evaluation_text_scroll = 0;
                    app.status_message = "Generating new text...".to_string();
                    tui.draw(|frame| ui::render(&mut app, frame))?;

                    if let Some(client) = &app.api_client {
                        let generation_prompt = "日本語の公的文書のようなお堅い文章を400文字程度で生成してください。";
                        match client.generate_text(generation_prompt).await {
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
            }
        }
    }

    tui::restore()?;
    Ok(())
}

enum AppAction {
    Evaluate,
    NextTraining,
}

async fn handle_events(app: &mut App) -> Result<Option<AppAction>, AppError> {
    if event::poll(Duration::from_millis(100))? {
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                if app.is_editing {
                    match key.code {
                        KeyCode::Esc => {
                            app.is_editing = false;
                            app.status_message = "Normal Mode. Press 'i' to edit.".to_string();
                        }
                        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            if !app.summary_input.is_empty() {
                                app.is_editing = false;
                                return Ok(Some(AppAction::Evaluate));
                            }
                        }
                        KeyCode::Char(c) => {
                            app.summary_input.insert(app.cursor_position, c);
                            app.cursor_position += c.len_utf8();
                        }
                        KeyCode::Backspace => {
                            if app.cursor_position > 0 {
                                let mut idx = app.cursor_position - 1;
                                while idx > 0 && !app.summary_input.is_char_boundary(idx) {
                                    idx -= 1;
                                }
                                app.summary_input.remove(idx);
                                app.cursor_position = idx;
                            }
                        }
                        KeyCode::Delete => {
                            if app.cursor_position < app.summary_input.len() {
                                app.summary_input.remove(app.cursor_position);
                            }
                        }
                        KeyCode::Left => {
                            if app.cursor_position > 0 {
                                let mut idx = app.cursor_position - 1;
                                while idx > 0 && !app.summary_input.is_char_boundary(idx) {
                                    idx -= 1;
                                }
                                app.cursor_position = idx;
                            }
                        }
                        KeyCode::Right => {
                            if app.cursor_position < app.summary_input.len() {
                                let mut idx = app.cursor_position + 1;
                                while idx < app.summary_input.len() && !app.summary_input.is_char_boundary(idx) {
                                    idx += 1;
                                }
                                app.cursor_position = idx;
                            }
                        }
                        KeyCode::Home => {
                            app.cursor_position = 0;
                        }
                        KeyCode::End => {
                            app.cursor_position = app.summary_input.len();
                        }
                        KeyCode::Enter => {
                            app.summary_input.insert(app.cursor_position, '\n');
                            app.cursor_position += 1;
                        }
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Char('i') | KeyCode::Enter => {
                            if !app.show_evaluation {
                                app.is_editing = true;
                                app.status_message = "Editing Mode. Press 'Esc' to exit.".to_string();
                            }
                        }
                        KeyCode::Char('n') => {
                            if app.show_evaluation {
                                return Ok(Some(AppAction::NextTraining));
                            }
                        }
                        KeyCode::Char('q') => {
                            app.should_quit = true;
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            if app.show_evaluation && key.modifiers.contains(KeyModifiers::SHIFT) {
                                app.evaluation_text_scroll = app.evaluation_text_scroll.saturating_add(1);
                            } else {
                                app.original_text_scroll = app.original_text_scroll.saturating_add(1);
                            }
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            if app.show_evaluation && key.modifiers.contains(KeyModifiers::SHIFT) {
                                app.evaluation_text_scroll = app.evaluation_text_scroll.saturating_sub(1);
                            } else {
                                app.original_text_scroll = app.original_text_scroll.saturating_sub(1);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    Ok(None)
}

async fn authenticate() -> Result<ApiClient, AppError> {
    if let Some(key) = config::load_api_key()? {
        if !key.is_empty() {
            let client = ApiClient::new(key);
            if client.validate_credentials().await.is_ok() {
                return Ok(client);
            }
        }
    }

    if let Ok(key) = env::var("GROQ_API_KEY") {
        if !key.is_empty() {
            let client = ApiClient::new(key.clone());
            if client.validate_credentials().await.is_ok() {
                if config::save_api_key(&key).is_err() {
                    // Ignore saving error
                }
                return Ok(client);
            }
        }
    }
    Err(AppError::InvalidApiKey)
}
