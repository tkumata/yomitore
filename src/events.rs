use crate::app::{App, MENU_OPTIONS, SettingsFocus, ViewMode};
use crate::error::AppError;
use ratatui::{
    crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    widgets::{Paragraph, Wrap},
};
use ratatui_textarea::{CursorMove, DataCursor, TextArea};
use std::time::Duration;

const EVENT_POLL_INTERVAL_MS: u64 = 100;

pub enum AppAction {
    Evaluate,
    NextTraining,
    StartTraining,
    SaveSettings,
    RetrySave,
}

pub fn handle_events(app: &mut App) -> Result<Option<AppAction>, AppError> {
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
                ViewMode::Settings => return Ok(handle_settings_events(app, key)),
                ViewMode::Editing => return Ok(handle_editing_events(app, key)),
                ViewMode::Normal => return Ok(handle_normal_mode_events(app, key)),
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
                if let Some(&count) = MENU_OPTIONS.get(app.selected_menu_item) {
                    app.character_count = count;
                }
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.selected_menu_item < MENU_OPTIONS.len() - 1 {
                app.selected_menu_item += 1;
                if let Some(&count) = MENU_OPTIONS.get(app.selected_menu_item) {
                    app.character_count = count;
                }
            }
        }
        KeyCode::Enter => {
            if let Some(&count) = MENU_OPTIONS.get(app.selected_menu_item) {
                app.character_count = count;
            }
            return Some(AppAction::StartTraining);
        }
        KeyCode::Char('r') => {
            app.enter_report_view();
        }
        KeyCode::Char('h') => {
            app.enter_help_view();
        }
        KeyCode::Char('s') => app.enter_settings_view(app.settings.groq_from_env),
        KeyCode::Char('q') => {
            app.should_quit = true;
        }
        _ => {}
    }
    None
}

fn handle_settings_events(app: &mut App, key: event::KeyEvent) -> Option<AppAction> {
    use SettingsFocus::{Cancel, Groq, Jev, Save};

    let focus = app.settings.focus;
    match key.code {
        KeyCode::Esc => app.return_from_settings(),
        KeyCode::Enter if focus == Save => return Some(AppAction::SaveSettings),
        KeyCode::Enter if focus == Cancel => app.return_from_settings(),
        KeyCode::Enter | KeyCode::Tab | KeyCode::BackTab => {
            let backwards = key.code == KeyCode::BackTab
                || (key.code == KeyCode::Tab && key.modifiers.contains(KeyModifiers::SHIFT));
            app.settings.focus = match (focus, backwards) {
                (Groq, false) | (Save, true) => Jev,
                (Jev, false) | (Cancel, true) => Save,
                (Save, false) | (Groq, true) => Cancel,
                (Cancel, false) | (Jev, true) => Groq,
            };
        }
        _ => {
            let input = match focus {
                Groq => &mut app.settings.groq_input,
                Jev => &mut app.settings.jev_input,
                Save | Cancel => return None,
            };
            match key.code {
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    input.push(ch);
                }
                _ => {}
            }
        }
    }
    None
}

fn handle_editing_events(app: &mut App, key: event::KeyEvent) -> Option<AppAction> {
    if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
        if app.original_text.is_some()
            && app
                .summary_input
                .lines()
                .iter()
                .any(|line| !line.trim().is_empty())
        {
            app.stop_editing();
            return Some(AppAction::Evaluate);
        }
    } else if key.code == KeyCode::Esc {
        app.stop_editing();
    } else {
        handle_summary_key(&mut app.summary_input, key);
    }
    None
}

fn handle_summary_key(input: &mut TextArea<'_>, key: event::KeyEvent) {
    let control = KeyModifiers::CONTROL;
    let shift = KeyModifiers::SHIFT;
    match (key.code, key.modifiers) {
        (KeyCode::Home | KeyCode::End, modifiers)
            if modifiers.difference(control | shift).is_empty() =>
        {
            let home = key.code == KeyCode::Home;
            let target = if modifiers.contains(control) {
                if home {
                    CursorMove::Top
                } else {
                    CursorMove::Bottom
                }
            } else {
                summary_row_end(input, home)
            };
            if modifiers.contains(shift) {
                if !input.is_selecting() {
                    input.start_selection();
                }
            } else {
                input.cancel_selection();
            }
            input.move_cursor(target);
        }
        (KeyCode::Char('a'), modifiers) if modifiers == control => input.select_all(),
        (KeyCode::Char('c'), modifiers) if modifiers == control => {
            let selected = input
                .selection_range()
                .map_or_else(String::new, |(start, end)| {
                    selected_summary_text(input, start, end)
                });
            input.set_yank_text(selected);
        }
        (KeyCode::Char('x'), modifiers) if modifiers == control => {
            if !input.cut() {
                input.set_yank_text(String::new());
            }
        }
        (KeyCode::Char('v'), modifiers) if modifiers == control => {
            input.paste();
        }
        (KeyCode::Char('d'), modifiers) if modifiers == control => {
            duplicate_summary_text(input);
        }
        (KeyCode::Char('y'), modifiers) if modifiers == control => {
            let row = input.cursor().0;
            if let Some(line) = input.lines().get(row) {
                let line_len = line.chars().count() + usize::from(row + 1 < input.lines().len());
                input.cancel_selection();
                input.move_cursor(CursorMove::Head);
                input.delete_str(line_len);
            }
        }
        (KeyCode::Char('z'), modifiers) if modifiers == control => {
            input.undo();
        }
        (KeyCode::Char('Z' | 'z'), modifiers) if modifiers == control | shift => {
            input.redo();
        }
        (KeyCode::Backspace, modifiers)
            if modifiers == control || modifiers == KeyModifiers::ALT =>
        {
            input.delete_word();
        }
        (KeyCode::Delete, modifiers) if modifiers == control || modifiers == KeyModifiers::ALT => {
            input.delete_next_word();
        }
        (KeyCode::Enter, modifiers) if modifiers.difference(shift).is_empty() => {
            insert_summary_newline(input);
        }
        (KeyCode::Tab | KeyCode::BackTab, modifiers) if modifiers.difference(shift).is_empty() => {
            indent_summary_selection(input, key.code == KeyCode::BackTab);
        }
        (KeyCode::Char(ch), modifiers) if modifiers == control | KeyModifiers::ALT => {
            input.insert_char(ch);
        }
        (KeyCode::Left | KeyCode::Right, modifiers)
            if modifiers.difference(control | shift).is_empty() =>
        {
            input.input(key);
        }
        (
            KeyCode::Char(_)
            | KeyCode::Backspace
            | KeyCode::Delete
            | KeyCode::Up
            | KeyCode::Down
            | KeyCode::PageUp
            | KeyCode::PageDown,
            modifiers,
        ) if modifiers.difference(shift).is_empty() => {
            input.input(key);
        }
        _ => {}
    }
}

fn insert_summary_newline(input: &mut TextArea<'_>) {
    let DataCursor(row, col) = input
        .selection_range()
        .map_or_else(|| input.cursor(), |(start, _)| start.into());
    let mut newline = String::from("\n");
    if let Some(line) = input.lines().get(row) {
        newline.extend(
            line.chars()
                .take(col)
                .take_while(|ch| matches!(ch, ' ' | '\t')),
        );
    }
    input.insert_str(newline);
}

fn indent_summary_selection(input: &mut TextArea<'_>, dedent: bool) {
    let Some((start, end)) = input.selection_range().filter(|(start, end)| start != end) else {
        if !dedent {
            input.insert_tab();
        }
        return;
    };
    let (Ok(first), Ok(last)) = (u16::try_from(start.0), u16::try_from(end.0)) else {
        return;
    };
    let backwards = input.cursor() == DataCursor::from(start);
    let width = usize::from(input.tab_length());
    let indent = " ".repeat(width);
    let mut start_col = start.1;
    let mut end_col = end.1;
    input.cancel_selection();
    for row in first..=last {
        input.move_cursor(CursorMove::Jump(row, 0));
        let (removed, added) = if dedent {
            let removed = input.lines().get(usize::from(row)).map_or(0, |line| {
                line.bytes()
                    .take(width)
                    .take_while(|byte| matches!(byte, b' ' | b'\t'))
                    .count()
            });
            input.delete_str(removed);
            (removed, 0)
        } else {
            input.insert_str(&indent);
            (0, width)
        };
        if row == first {
            start_col = start_col.saturating_sub(removed) + added;
        }
        if row == last {
            end_col = end_col.saturating_sub(removed) + added;
        }
    }
    let (Ok(start_col), Ok(end_col)) = (u16::try_from(start_col), u16::try_from(end_col)) else {
        return;
    };
    let (start, end) = (
        CursorMove::Jump(first, start_col),
        CursorMove::Jump(last, end_col),
    );
    let (anchor, cursor) = if backwards {
        (end, start)
    } else {
        (start, end)
    };
    input.move_cursor(anchor);
    input.start_selection();
    input.move_cursor(cursor);
}

fn selected_summary_text(
    input: &TextArea<'_>,
    (first_row, first_col): (usize, usize),
    (last_row, last_col): (usize, usize),
) -> String {
    input
        .lines()
        .iter()
        .enumerate()
        .skip(first_row)
        .take(last_row - first_row + 1)
        .map(|(row, line)| {
            let start = if row == first_row { first_col } else { 0 };
            let end = if row == last_row {
                last_col
            } else {
                usize::MAX
            };
            line.chars()
                .skip(start)
                .take(end - start)
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn duplicate_summary_text(input: &mut TextArea<'_>) {
    if let Some((start, end)) = input.selection_range()
        && start != end
    {
        let (Ok(row), Ok(col)) = (u16::try_from(end.0), u16::try_from(end.1)) else {
            return;
        };
        let duplicate = selected_summary_text(input, start, end);
        input.cancel_selection();
        input.move_cursor(CursorMove::Jump(row, col));
        input.insert_str(duplicate);
    } else {
        let DataCursor(row, col) = input.cursor();
        if let Some(line) = input.lines().get(row) {
            let duplicate = if row + 1 < input.lines().len() {
                format!("{line}\n")
            } else {
                line.clone()
            };
            input.cancel_selection();
            input.move_cursor(CursorMove::Head);
            input.insert_str(duplicate);
            let cursor = input.cursor();
            if let (Ok(row), Ok(col)) = (u16::try_from(cursor.0), u16::try_from(cursor.1 + col)) {
                input.move_cursor(CursorMove::Jump(row, col));
            }
        }
    }
}

fn summary_row_end(input: &TextArea<'_>, home: bool) -> CursorMove {
    let row = input.screen_cursor().row;
    let start = input.screen_to_data(row, 0);
    let end = input.screen_to_data(row, usize::MAX);
    let col = if home {
        let indent = input.lines().get(start.0).map_or(0, |line| {
            line.chars()
                .skip(start.1)
                .take(end.1 - start.1)
                .take_while(|ch| matches!(ch, ' ' | '\t'))
                .count()
        });
        let after_indent = start.1 + indent;
        if after_indent == input.cursor().1 {
            start.1
        } else {
            after_indent
        }
    } else if end != start && end == input.screen_to_data(row + 1, 0) {
        end.1 - 1
    } else {
        end.1
    };

    match (u16::try_from(start.0), u16::try_from(col)) {
        (Ok(row), Ok(col)) => CursorMove::Jump(row, col),
        _ if home => CursorMove::Head,
        _ => CursorMove::End,
    }
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
            if app.original_text.is_some()
                && app.pending_evaluation.is_none()
                && !app.show_evaluation_overlay
            {
                app.begin_editing();
            }
        }
        KeyCode::Char('e') => {
            if !app.evaluation_text.is_empty() {
                app.show_evaluation_overlay.toggle();
                if app.show_evaluation_overlay {
                    app.evaluation_overlay_scroll = 0;
                }
            }
        }
        KeyCode::Char('n') => {
            if app.pending_evaluation.is_none()
                && (app.show_evaluation_overlay || app.generation_error.is_some())
            {
                app.show_evaluation_overlay = false;
                return Some(AppAction::NextTraining);
            }
        }
        KeyCode::Char('r') => {
            app.enter_report_view();
        }
        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if app.pending_evaluation.is_some() {
                return Some(AppAction::RetrySave);
            }
        }
        KeyCode::Char('s') => app.enter_settings_view(app.settings.groq_from_env),
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
                app.original_text_scroll = app.original_text_scroll.saturating_add(1);
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
    let total_lines = u16::try_from(paragraph.line_count(visible_width)).unwrap_or(u16::MAX);
    total_lines.saturating_sub(visible_height)
}

#[cfg(test)]
mod tests {
    use super::{
        App, AppAction, KeyCode, KeyModifiers, SettingsFocus, ViewMode, calculate_max_scroll,
        handle_editing_events, handle_normal_mode_events, handle_settings_events,
        handle_summary_key,
    };
    use crate::app::SUMMARY_LINE_ENDING;
    use ratatui::{
        Terminal,
        backend::{Backend, TestBackend},
        crossterm::event::KeyEvent,
        style::Modifier,
    };
    use ratatui_textarea::TextArea;

    #[test]
    fn settings_form_masks_input_and_routes_save_and_cancel() {
        let mut app = App::default();
        app.settings.groq_saved = true;
        app.enter_settings_view(false);
        let Ok(mut terminal) = Terminal::new(TestBackend::new(100, 30));
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        let enter = key(KeyCode::Enter);

        let Ok(_) = terminal.draw(|frame| crate::ui::render(&mut app, frame));
        assert!(terminal.backend().cursor_visible());
        assert_eq!(terminal.backend().cursor_position(), (3, 3).into());
        let screen = terminal.backend().buffer();
        assert!((3..11).all(|column| screen[(column, 3)].symbol() == "*"));
        assert_eq!(screen[(11, 3)].symbol(), " ");
        assert_eq!(screen[(3, 6)].symbol(), " ");

        handle_settings_events(&mut app, key(KeyCode::Char('a')));
        handle_settings_events(&mut app, key(KeyCode::Char('日')));
        handle_settings_events(&mut app, key(KeyCode::Backspace));
        assert_eq!(app.settings.groq_input, "a");
        let Ok(_) = terminal.draw(|frame| crate::ui::render(&mut app, frame));
        assert_eq!(terminal.backend().cursor_position(), (4, 3).into());
        assert_eq!(terminal.backend().buffer()[(3, 3)].symbol(), "*");
        assert_eq!(terminal.backend().buffer()[(4, 3)].symbol(), " ");

        handle_settings_events(&mut app, key(KeyCode::Tab));
        assert!(app.settings.focus == SettingsFocus::Jev);
        let Ok(_) = terminal.draw(|frame| crate::ui::render(&mut app, frame));
        assert_eq!(terminal.backend().cursor_position(), (3, 6).into());
        assert!(handle_settings_events(&mut app, enter).is_none());
        let Ok(_) = terminal.draw(|frame| crate::ui::render(&mut app, frame));
        assert!(!terminal.backend().cursor_visible());
        let screen = terminal.backend().buffer();
        assert!(
            screen[(3, 8)]
                .modifier
                .contains(Modifier::REVERSED | Modifier::UNDERLINED)
        );
        assert_eq!(screen[(10, 8)].modifier, Modifier::REVERSED);
        assert!(matches!(
            handle_settings_events(&mut app, enter),
            Some(AppAction::SaveSettings)
        ));

        let back_tab = KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT);
        handle_settings_events(&mut app, back_tab);
        assert!(app.settings.focus == SettingsFocus::Jev);
        handle_settings_events(&mut app, back_tab);
        assert!(app.settings.focus == SettingsFocus::Groq);
        handle_settings_events(&mut app, back_tab);
        assert!(app.settings.focus == SettingsFocus::Cancel);
        assert!(handle_settings_events(&mut app, enter).is_none());
        assert!(app.view_mode == ViewMode::Menu);
        assert_eq!(app.settings.groq_input, "");

        app.enter_settings_view(false);
        handle_settings_events(&mut app, key(KeyCode::Char('b')));
        assert!(handle_settings_events(&mut app, key(KeyCode::Esc)).is_none());
        assert!(app.view_mode == ViewMode::Menu);
        assert_eq!(app.settings.groq_input, "");
    }

    #[test]
    fn generation_failure_disables_submission_and_allows_retry() {
        let mut app = App::default();
        let next = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE);
        let edit = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        let submit = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL);
        for next_question in [false, true] {
            app.begin_training_generation(next_question);
            app.apply_generation_error(&"通信エラー");
            app.summary_input.insert_str("要約");
            assert!(handle_normal_mode_events(&mut app, edit).is_none());
            assert!(app.view_mode == ViewMode::Normal);
            assert!(handle_editing_events(&mut app, submit).is_none());
            assert!(matches!(
                handle_normal_mode_events(&mut app, next),
                Some(AppAction::NextTraining)
            ));

            app.prepare_next_training();
            app.apply_generated_text("原文".to_owned());
            handle_normal_mode_events(&mut app, edit);
            app.summary_input.insert_str("要約");
            assert!(matches!(
                handle_editing_events(&mut app, submit),
                Some(AppAction::Evaluate)
            ));
        }
    }

    #[test]
    fn summary_input_wraps_and_keeps_edits_in_a_narrow_terminal() {
        let mut app = App::default();
        let Ok(mut terminal) = Terminal::new(TestBackend::new(52, 9));
        app.begin_training_generation(false);
        app.apply_generated_text("問題文".to_owned());
        let Ok(_) = terminal.draw(|frame| crate::ui::render(&mut app, frame));

        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        let submit = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL);
        handle_normal_mode_events(&mut app, key(KeyCode::Char('i')));
        handle_editing_events(&mut app, key(KeyCode::Char(' ')));
        assert!(handle_editing_events(&mut app, submit).is_none());
        handle_editing_events(&mut app, key(KeyCode::Backspace));

        let long_line = format!("先{}末", "日".repeat(38));
        for ch in long_line.chars() {
            handle_editing_events(&mut app, key(KeyCode::Char(ch)));
        }
        let Ok(_) = terminal.draw(|frame| crate::ui::render(&mut app, frame));
        let Ok(cursor) = terminal.backend_mut().get_cursor_position();
        let buffer = terminal.backend().buffer();
        assert!(buffer.content().iter().any(|cell| cell.symbol() == "末"));

        handle_editing_events(&mut app, key(KeyCode::Home));
        let Ok(_) = terminal.draw(|frame| crate::ui::render(&mut app, frame));
        let Ok(visual_home) = terminal.backend_mut().get_cursor_position();
        assert_eq!(visual_home.x, 27);
        assert_eq!(visual_home.y, cursor.y);
        for _ in 0..4 {
            handle_editing_events(&mut app, key(KeyCode::Up));
            let Ok(_) = terminal.draw(|frame| crate::ui::render(&mut app, frame));
        }
        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(27, 2)].symbol(), "先");
        assert_eq!(buffer[(27, 3)].symbol(), "日");
        assert_eq!(buffer[(51, 2)].symbol(), "│");

        for _ in 0..4 {
            handle_editing_events(&mut app, key(KeyCode::Down));
            let Ok(_) = terminal.draw(|frame| crate::ui::render(&mut app, frame));
        }
        for code in [KeyCode::End, KeyCode::Enter, KeyCode::Char('要')] {
            handle_editing_events(&mut app, key(code));
            let Ok(_) = terminal.draw(|frame| crate::ui::render(&mut app, frame));
        }
        handle_editing_events(&mut app, key(KeyCode::Esc));
        assert!(app.view_mode == ViewMode::Normal);
        handle_normal_mode_events(&mut app, key(KeyCode::Enter));
        assert!(app.view_mode == ViewMode::Editing);
        assert!(matches!(
            handle_editing_events(&mut app, submit),
            Some(AppAction::Evaluate)
        ));
        assert_eq!(
            app.summary_input.lines().join(SUMMARY_LINE_ENDING),
            format!("{long_line}{SUMMARY_LINE_ENDING}要")
        );
        assert!(app.view_mode == ViewMode::Normal);
    }

    #[test]
    fn summary_input_preserves_clipboard_and_line_editing_shortcuts() {
        let mut input = TextArea::from(["一行目", "二行目"]);
        let control = KeyModifiers::CONTROL;
        let ctrl = |ch| KeyEvent::new(KeyCode::Char(ch), control);

        handle_summary_key(&mut input, ctrl('d'));
        assert_eq!(input.lines(), ["一行目", "一行目", "二行目"]);
        handle_summary_key(&mut input, ctrl('y'));
        assert_eq!(input.lines(), ["一行目", "二行目"]);
        handle_summary_key(&mut input, ctrl('z'));
        assert_eq!(input.lines(), ["一行目", "一行目", "二行目"]);
        handle_summary_key(
            &mut input,
            KeyEvent::new(KeyCode::Char('Z'), control | KeyModifiers::SHIFT),
        );
        assert_eq!(input.lines(), ["一行目", "二行目"]);

        handle_summary_key(&mut input, ctrl('a'));
        let selection = input.selection_range();
        handle_summary_key(&mut input, ctrl('c'));
        assert_eq!(input.selection_range(), selection);
        handle_summary_key(&mut input, ctrl('x'));
        assert!(input.is_empty());
        handle_summary_key(&mut input, ctrl('v'));
        assert_eq!(input.lines(), ["一行目", "二行目"]);

        handle_summary_key(&mut input, KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
        handle_summary_key(&mut input, ctrl('k'));
        assert_eq!(input.lines(), ["一行目", "二行目"]);

        handle_summary_key(&mut input, ctrl('a'));
        handle_summary_key(&mut input, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(input.lines(), ["    一行目", "    二行目"]);
        handle_summary_key(
            &mut input,
            KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT),
        );
        assert_eq!(input.lines(), ["一行目", "二行目"]);
    }

    #[test]
    fn summary_cursor_stays_visible_when_text_fills_the_row() {
        let mut app = App::default();
        let Ok(mut terminal) = Terminal::new(TestBackend::new(52, 9));
        app.begin_editing();
        app.summary_input.insert_str("日".repeat(12));

        let Ok(_) = terminal.draw(|frame| crate::ui::render(&mut app, frame));

        assert!(terminal.backend().cursor_visible());
        assert_eq!(terminal.backend().cursor_position().x, 50);
    }

    #[test]
    fn original_text_keeps_wide_characters_inside_the_pane() {
        let mut app = App::default();
        let Ok(mut terminal) = Terminal::new(TestBackend::new(150, 49));
        app.begin_training_generation(false);
        app.apply_generated_text("始本文末".to_owned());
        let Ok(_) = terminal.draw(|frame| crate::ui::render(&mut app, frame));
        assert_eq!(terminal.backend().buffer()[(1, 2)].symbol(), "始");
        assert_eq!(terminal.backend().buffer()[(7, 2)].symbol(), "末");

        let text = format!("始\n{}a {}末", "本文\n".repeat(41), "日".repeat(35));
        app.apply_generated_text(text.clone());
        let Ok(_) = terminal.draw(|frame| crate::ui::render(&mut app, frame));

        let screen = terminal.backend().buffer();
        assert_eq!(app.original_text.as_deref(), Some(text.as_str()));
        assert_eq!(screen[(1, 2)].symbol(), "始");
        assert_eq!(screen[(74, 44)].symbol(), "│");
        assert_ne!(screen[(73, 44)].symbol(), "末");

        for _ in 0..2 {
            let _ = handle_normal_mode_events(
                &mut app,
                KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            );
            let Ok(_) = terminal.draw(|frame| crate::ui::render(&mut app, frame));
            assert_eq!(terminal.backend().buffer()[(1, 44)].symbol(), "末");
        }
    }

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
