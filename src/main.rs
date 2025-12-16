mod api_client;
mod app;
mod config;
mod error;
mod help;
mod reports;
mod stats;
mod tui;
mod ui;

use crate::{
    api_client::ApiClient,
    app::{App, MENU_OPTIONS, ViewMode},
    error::AppError,
};
use rat_text::event::HandleEvent;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use std::{env, time::Duration};

/// Event polling interval in milliseconds
const EVENT_POLL_INTERVAL_MS: u64 = 100;

/// Overlay size as percentage of screen
const OVERLAY_SIZE_PERCENT: u16 = 75;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    let mut app = App::default();

    let api_client = authenticate().await?;
    app.api_client = Some(api_client);

    let mut tui = tui::init()?;

    // Main loop
    while !app.should_quit {
        tui.draw(|frame| ui::render(&mut app, frame))?;

        if let Some(result) = handle_events(&mut app).await? {
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

                        match client.evaluate_summary(&app.original_text, &summary).await {
                            Ok(evaluation) => {
                                // Check if the evaluation starts with "はい" (yes) to determine pass/fail
                                // More robust: only check the first line
                                app.evaluation_passed = evaluation
                                    .lines()
                                    .next()
                                    .map(|line| line.trim().starts_with("はい"))
                                    .unwrap_or(false);
                                app.evaluation_text = evaluation;
                                app.show_evaluation_overlay = true;
                                app.evaluation_overlay_scroll = 0;
                                app.is_evaluating = false;
                                app.status_message =
                                    "Evaluation complete. Press 'e' to toggle, 'n' for next."
                                        .to_string();

                                // Save the result to stats
                                app.stats.add_result(app.evaluation_passed);
                                if let Err(e) = app.stats.save() {
                                    app.status_message =
                                        format!("Warning: Failed to save stats: {}", e);
                                    eprintln!("Failed to save stats: {}", e);
                                }
                            }
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

enum AppAction {
    Evaluate,
    NextTraining,
    StartTraining,
}

async fn handle_events(app: &mut App) -> Result<Option<AppAction>, AppError> {
    if event::poll(Duration::from_millis(EVENT_POLL_INTERVAL_MS))? {
        let ev = event::read()?;
        if let Event::Key(key) = ev {
            if key.kind != KeyEventKind::Press {
                return Ok(None);
            }
            // Handle menu navigation
            if app.view_mode == ViewMode::Menu {
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        if app.selected_menu_item > 0 {
                            app.selected_menu_item -= 1;
                            app.character_count = MENU_OPTIONS[app.selected_menu_item];
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if app.selected_menu_item < MENU_OPTIONS.len() - 1 {
                            app.selected_menu_item += 1;
                            app.character_count = MENU_OPTIONS[app.selected_menu_item];
                        }
                    }
                    KeyCode::Enter => {
                        app.character_count = MENU_OPTIONS[app.selected_menu_item];
                        return Ok(Some(AppAction::StartTraining));
                    }
                    KeyCode::Char('r') => {
                        // Show report from menu
                        app.view_mode = ViewMode::Report;
                        app.status_message = "Report. Press 'r' to close.".to_string();
                    }
                    KeyCode::Char('h') => {
                        // Show help from menu
                        app.view_mode = ViewMode::Help;
                        app.status_message = "Help. Press 'h' to close.".to_string();
                    }
                    KeyCode::Char('q') => {
                        app.should_quit = true;
                    }
                    _ => {}
                }
                return Ok(None);
            }

            if app.is_editing {
                // Check for Ctrl+S to submit (Shift+Enter doesn't work in most terminals)
                if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    // Ctrl+S: Submit for evaluation
                    let content = app.text_area_state.value().to_string();
                    if !content.trim().is_empty() {
                        app.is_editing = false;
                        app.text_area_state.focus.set(false); // Disable focus
                        return Ok(Some(AppAction::Evaluate));
                    }
                } else if key.code == KeyCode::Esc {
                    app.is_editing = false;
                    app.text_area_state.focus.set(false); // Disable focus
                    app.status_message = "Normal Mode. Press 'i' to edit.".to_string();
                } else {
                    // Pass all other input to rat-text TextArea
                    // Use HandleEvent trait
                    let _ = app.text_area_state.handle(&ev, rat_text::event::Regular);
                }
            } else {
                // Handle Report view
                if app.view_mode == ViewMode::Report {
                    match key.code {
                        KeyCode::Char('r') => {
                            app.return_from_report();
                        }
                        KeyCode::Char('q') => {
                            app.should_quit = true;
                        }
                        _ => {}
                    }
                    return Ok(None);
                }

                // Handle Help view
                if app.view_mode == ViewMode::Help {
                    match key.code {
                        KeyCode::Char('h') => {
                            app.return_from_report();
                            app.help_scroll = 0;
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            app.help_scroll = app.help_scroll.saturating_add(1);
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            app.help_scroll = app.help_scroll.saturating_sub(1);
                        }
                        KeyCode::Char('q') => {
                            app.should_quit = true;
                        }
                        _ => {}
                    }
                    return Ok(None);
                }

                match key.code {
                    KeyCode::Char('i') | KeyCode::Enter => {
                        if !app.show_evaluation_overlay && app.view_mode == ViewMode::Normal {
                            app.is_editing = true;
                            app.text_area_state.focus.set(true); // Enable focus for input!
                            app.text_area_state.scroll_cursor_to_visible(); // Keep cursor viewport sane
                            app.status_message = "Editing Mode. Press 'Esc' to exit.".to_string();
                        }
                    }
                    KeyCode::Char('e') => {
                        // Toggle evaluation overlay (only if evaluation exists)
                        if app.view_mode == ViewMode::Normal && !app.evaluation_text.is_empty() {
                            app.show_evaluation_overlay = !app.show_evaluation_overlay;
                            if app.show_evaluation_overlay {
                                app.evaluation_overlay_scroll = 0;
                            }
                        }
                    }
                    KeyCode::Char('n') => {
                        // Next training: close evaluation overlay and proceed
                        if app.show_evaluation_overlay && app.view_mode == ViewMode::Normal {
                            app.show_evaluation_overlay = false;
                            return Ok(Some(AppAction::NextTraining));
                        }
                    }
                    KeyCode::Char('r') => {
                        // Toggle report
                        if app.view_mode == ViewMode::Report {
                            app.return_from_report();
                        } else {
                            app.view_mode = ViewMode::Report;
                            app.status_message = "Report. Press 'r' to close.".to_string();
                        }
                    }
                    KeyCode::Char('h') => {
                        // Toggle help
                        if app.view_mode == ViewMode::Help {
                            app.return_from_report();
                            app.help_scroll = 0;
                        } else {
                            app.view_mode = ViewMode::Help;
                            app.status_message = "Help. Press 'h' to close.".to_string();
                        }
                    }
                    KeyCode::Char('q') => {
                        app.should_quit = true;
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if app.view_mode == ViewMode::Normal {
                            if app.show_evaluation_overlay
                                && key.modifiers.contains(KeyModifiers::SHIFT)
                            {
                                // Scroll evaluation overlay with bounds checking
                                // Calculate visible height: overlay percent of screen minus borders and headers
                                let visible_height = (app.terminal_height * OVERLAY_SIZE_PERCENT
                                    / 100)
                                    .saturating_sub(4);
                                let max_scroll =
                                    calculate_max_scroll(&app.evaluation_text, visible_height);
                                app.evaluation_overlay_scroll = app
                                    .evaluation_overlay_scroll
                                    .saturating_add(1)
                                    .min(max_scroll);
                            } else {
                                // Scroll original text with bounds checking
                                // Calculate visible height: half screen minus header and status bar
                                let visible_height = (app.terminal_height / 2).saturating_sub(3);
                                let max_scroll =
                                    calculate_max_scroll(&app.original_text, visible_height);
                                app.original_text_scroll =
                                    app.original_text_scroll.saturating_add(1).min(max_scroll);
                            }
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if app.view_mode == ViewMode::Normal {
                            if app.show_evaluation_overlay
                                && key.modifiers.contains(KeyModifiers::SHIFT)
                            {
                                app.evaluation_overlay_scroll =
                                    app.evaluation_overlay_scroll.saturating_sub(1);
                            } else {
                                app.original_text_scroll =
                                    app.original_text_scroll.saturating_sub(1);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(None)
}

/// Calculate the maximum scroll offset for given text content
fn calculate_max_scroll(text: &str, visible_height: u16) -> u16 {
    let total_lines = text.lines().count() as u16;
    total_lines.saturating_sub(visible_height.saturating_sub(2)) // -2 for borders
}

/// Generate text using the API client and update app state
async fn generate_text_for_training(app: &mut App) {
    if let Some(client) = &app.api_client {
        match client.generate_text(&app.generate_text_prompt()).await {
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

async fn authenticate() -> Result<ApiClient, AppError> {
    if let Some(key) = config::load_api_key()?
        && !key.is_empty()
    {
        let client = ApiClient::new(key);
        if client.validate_credentials().await.is_ok() {
            return Ok(client);
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
            return Ok(client);
        }
    }
    Err(AppError::InvalidApiKey)
}
