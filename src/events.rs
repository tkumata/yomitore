use crate::app::{App, MENU_OPTIONS, ViewMode};
use crate::error::AppError;
use rat_text::event::HandleEvent;
use ratatui::{
    crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    widgets::{Paragraph, Wrap},
};
use std::time::Duration;

const EVENT_POLL_INTERVAL_MS: u64 = 100;

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
            app.enter_report_view();
        }
        KeyCode::Char('h') => {
            app.enter_help_view();
        }
        KeyCode::Char('q') => {
            app.should_quit = true;
        }
        _ => {}
    }
    None
}

fn handle_editing_events(app: &mut App, ev: Event, key: event::KeyEvent) -> Option<AppAction> {
    if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
        let content = app.text_area_state.value().to_string();
        if !content.trim().is_empty() {
            app.stop_editing();
            return Some(AppAction::Evaluate);
        }
    } else if key.code == KeyCode::Esc {
        app.stop_editing();
    } else {
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
                app.begin_editing();
            }
        }
        KeyCode::Char('e') => {
            if !app.evaluation_text.is_empty() {
                app.show_evaluation_overlay = !app.show_evaluation_overlay;
                if app.show_evaluation_overlay {
                    app.evaluation_overlay_scroll = 0;
                }
            }
        }
        KeyCode::Char('n') => {
            if app.show_evaluation_overlay {
                app.show_evaluation_overlay = false;
                return Some(AppAction::NextTraining);
            }
        }
        KeyCode::Char('r') => {
            app.enter_report_view();
        }
        KeyCode::Char('h') => {
            app.enter_help_view();
        }
        KeyCode::Char('q') => {
            app.should_quit = true;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.show_evaluation_overlay && key.modifiers.contains(KeyModifiers::SHIFT) {
                let (visible_height, visible_width) = app.evaluation_viewport_size();
                let max_scroll =
                    calculate_max_scroll(&app.evaluation_text, visible_height, visible_width);
                app.evaluation_overlay_scroll = app
                    .evaluation_overlay_scroll
                    .saturating_add(1)
                    .min(max_scroll);
            } else {
                let (visible_height, visible_width) = app.original_text_viewport_size();
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

fn calculate_max_scroll(text: &str, visible_height: u16, visible_width: u16) -> u16 {
    if visible_width == 0 || visible_height == 0 {
        return 0;
    }
    let paragraph = Paragraph::new(text).wrap(Wrap { trim: false });
    let total_lines = paragraph.line_count(visible_width) as u16;
    total_lines.saturating_sub(visible_height)
}

#[cfg(test)]
mod tests {
    use super::calculate_max_scroll;

    #[test]
    fn calculate_max_scroll_uses_inner_height_without_extra_border_adjustment() {
        let text = "1\n2\n3\n4\n5";
        assert_eq!(calculate_max_scroll(text, 3, 10), 2);
    }

    #[test]
    fn calculate_max_scroll_returns_zero_when_content_fits() {
        let text = "1\n2\n3";
        assert_eq!(calculate_max_scroll(text, 3, 10), 0);
    }

    #[test]
    fn calculate_max_scroll_returns_zero_for_zero_sized_viewport() {
        assert_eq!(calculate_max_scroll("1\n2\n3", 0, 10), 0);
        assert_eq!(calculate_max_scroll("1\n2\n3", 3, 0), 0);
    }
}
