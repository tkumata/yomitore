use crate::app::{App, MENU_OPTIONS, ViewMode};
use crate::error::AppError;
use rat_text::event::HandleEvent;
use ratatui::{
    crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    widgets::{Paragraph, Wrap},
};
use std::time::Duration;

/// Event polling interval in milliseconds
const EVENT_POLL_INTERVAL_MS: u64 = 100;

/// Overlay size as percentage of screen
const OVERLAY_SIZE_PERCENT: u16 = 75;

pub enum AppAction {
    Evaluate,
    NextTraining,
    StartTraining,
}

pub async fn handle_events(app: &mut App) -> Result<Option<AppAction>, AppError> {
    if event::poll(Duration::from_millis(EVENT_POLL_INTERVAL_MS))? {
        let ev = event::read()?;
        if let Event::Key(key) = ev {
            if key.kind != KeyEventKind::Press {
                return Ok(None);
            }

            match app.view_mode {
                ViewMode::Menu => return Ok(handle_menu_events(app, key)),
                ViewMode::Report => {
                    handle_report_events(app, key);
                    return Ok(None);
                }
                ViewMode::Help => {
                    handle_help_events(app, key);
                    return Ok(None);
                }
                ViewMode::Normal => {
                    if app.is_editing {
                        return Ok(handle_editing_events(app, ev, key));
                    } else {
                        return Ok(handle_normal_mode_events(app, key));
                    }
                }
            }
        }
    }
    Ok(None)
}

fn handle_menu_events(app: &mut App, key: event::KeyEvent) -> Option<AppAction> {
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
            return Some(AppAction::StartTraining);
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
    None
}

fn handle_editing_events(app: &mut App, ev: Event, key: event::KeyEvent) -> Option<AppAction> {
    // Check for Ctrl+S to submit (Shift+Enter doesn't work in most terminals)
    if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
        // Ctrl+S: Submit for evaluation
        let content = app.text_area_state.value().to_string();
        if !content.trim().is_empty() {
            app.is_editing = false;
            app.text_area_state.focus.set(false); // Disable focus
            return Some(AppAction::Evaluate);
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
    None
}

fn handle_report_events(app: &mut App, key: event::KeyEvent) {
    match key.code {
        KeyCode::Char('r') => {
            app.return_from_aux_view();
        }
        KeyCode::Char('q') => {
            app.should_quit = true;
        }
        _ => {}
    }
}

fn handle_help_events(app: &mut App, key: event::KeyEvent) {
    match key.code {
        KeyCode::Char('h') => {
            app.return_from_aux_view();
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
}

fn handle_normal_mode_events(app: &mut App, key: event::KeyEvent) -> Option<AppAction> {
    match key.code {
        KeyCode::Char('i') | KeyCode::Enter => {
            if !app.show_evaluation_overlay {
                app.is_editing = true;
                app.text_area_state.focus.set(true); // Enable focus for input!
                app.text_area_state.scroll_cursor_to_visible(); // Keep cursor viewport sane
                app.status_message = "Editing Mode. Press 'Esc' to exit.".to_string();
            }
        }
        KeyCode::Char('e') => {
            // Toggle evaluation overlay (only if evaluation exists)
            if !app.evaluation_text.is_empty() {
                app.show_evaluation_overlay = !app.show_evaluation_overlay;
                if app.show_evaluation_overlay {
                    app.evaluation_overlay_scroll = 0;
                }
            }
        }
        KeyCode::Char('n') => {
            // Next training: close evaluation overlay and proceed
            if app.show_evaluation_overlay {
                app.show_evaluation_overlay = false;
                return Some(AppAction::NextTraining);
            }
        }
        KeyCode::Char('r') => {
            // Toggle report
            app.view_mode = ViewMode::Report;
            app.status_message = "Report. Press 'r' to close.".to_string();
        }
        KeyCode::Char('h') => {
            // Toggle help
            app.view_mode = ViewMode::Help;
            app.status_message = "Help. Press 'h' to close.".to_string();
        }
        KeyCode::Char('q') => {
            app.should_quit = true;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.show_evaluation_overlay && key.modifiers.contains(KeyModifiers::SHIFT) {
                // Scroll evaluation overlay with bounds checking
                // Calculate visible height: overlay percent of screen minus borders and headers
                let visible_height =
                    (app.terminal_height * OVERLAY_SIZE_PERCENT / 100).saturating_sub(4);
                let visible_width =
                    (app.terminal_width * OVERLAY_SIZE_PERCENT / 100).saturating_sub(2);
                let max_scroll =
                    calculate_max_scroll(&app.evaluation_text, visible_height, visible_width);
                app.evaluation_overlay_scroll = app
                    .evaluation_overlay_scroll
                    .saturating_add(1)
                    .min(max_scroll);
            } else {
                // Scroll original text with bounds checking
                // Calculate visible height: half screen minus header and status bar
                let visible_height = (app.terminal_height / 2).saturating_sub(3);
                let visible_width = (app.terminal_width / 2).saturating_sub(2);
                let max_scroll =
                    calculate_max_scroll(&app.original_text, visible_height, visible_width);
                app.original_text_scroll =
                    app.original_text_scroll.saturating_add(1).min(max_scroll);
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.show_evaluation_overlay && key.modifiers.contains(KeyModifiers::SHIFT) {
                app.evaluation_overlay_scroll = app.evaluation_overlay_scroll.saturating_sub(1);
            } else {
                app.original_text_scroll = app.original_text_scroll.saturating_sub(1);
            }
        }
        _ => {}
    }
    None
}

/// Calculate the maximum scroll offset for given text content
fn calculate_max_scroll(text: &str, visible_height: u16, visible_width: u16) -> u16 {
    if visible_width == 0 {
        return 0;
    }
    let paragraph = Paragraph::new(text).wrap(Wrap { trim: false });
    let total_lines = paragraph.line_count(visible_width) as u16;
    total_lines.saturating_sub(visible_height.saturating_sub(2)) // -2 for borders
}
