use crate::app::{App, MENU_OPTIONS, ViewMode};
use crate::help;
use crate::reports;
use rat_text::{HasScreenCursor, text_area::TextAreaState};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

/// Overlay size as percentage of screen
const OVERLAY_SIZE_PERCENT: u16 = 75;
/// Minimum overlay dimensions
const MIN_OVERLAY_WIDTH: u16 = 40;
const MIN_OVERLAY_HEIGHT: u16 = 10;

/// Renders the user interface widgets.
pub fn render(app: &mut App, frame: &mut Frame) {
    // Update terminal dimensions
    app.terminal_width = frame.area().width;
    app.terminal_height = frame.area().height;

    // Check if we should show a report instead of the normal view
    match app.view_mode {
        ViewMode::Menu => {
            render_menu_view(app, frame);
            return;
        }
        ViewMode::Report => {
            render_report_view(app, frame);
            return;
        }
        ViewMode::Help => {
            render_help_view(app, frame);
            return;
        }
        ViewMode::Normal => {
            // Continue with normal rendering
        }
    }

    // Main layout: Header, Content, Status
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Header
            Constraint::Min(0),    // Content (3 blocks)
            Constraint::Length(3), // Status
        ])
        .split(frame.area());

    render_header(frame, main_layout[0]);

    // Content layout: Fixed 50-50 split
    let content_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50), // Left: Original text
            Constraint::Percentage(50), // Right: Answer input
        ])
        .split(main_layout[1]);

    // Render blocks
    render_original_text(app, frame, content_layout[0]);
    render_summary_input(app, frame, content_layout[1]);

    // Render evaluation overlay on top if visible
    if app.show_evaluation_overlay {
        render_evaluation_overlay(app, frame);
    }

    render_status_bar(app, frame, main_layout[2]);

    // Set cursor position if editing
    if app.is_editing
        && let Some((cx, cy)) = app.text_area_state.screen_cursor()
    {
        frame.set_cursor_position((cx, cy));
    }
}

fn render_header(frame: &mut Frame, area: Rect) {
    let title = Paragraph::new(" yomitore: 読解力トレーニング ")
        .style(Style::new().bold())
        .alignment(Alignment::Center);
    frame.render_widget(title, area);
}

fn render_original_text(app: &App, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .title("原文 (↑/↓ or j/k: スクロール)")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let paragraph = Paragraph::new(app.original_text.as_str())
        .wrap(Wrap { trim: false })
        .scroll((app.original_text_scroll, 0))
        .block(block);
    frame.render_widget(paragraph, area);
}

fn render_summary_input(app: &mut App, frame: &mut Frame, area: Rect) {
    let title = "あなたの要約 (i:入力モード Esc:通常モード Ctrl+S:送信)";

    clamp_textarea_scroll(&mut app.text_area_state);

    let border_style = if app.is_editing {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::Blue)
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    // Create TextArea widget with word-wrap enabled
    use rat_text::text_area::{TextArea, TextWrap};

    let textarea = TextArea::new()
        .block(block)
        .text_wrap(TextWrap::Word(2)) // Safer default margin; prefer near-edge wrap
        .style(Style::default());

    // Render with state
    frame.render_stateful_widget(textarea, area, &mut app.text_area_state);
}

/// rat-textはオフセットが行数を超えると描画を丸ごとスキップするため、防御的に補正する
fn clamp_textarea_scroll(state: &mut TextAreaState) {
    let max_v = state.len_lines().saturating_sub(1) as usize;
    if state.vscroll.offset > max_v {
        state.vscroll.offset = max_v;
    }
    state.hscroll.offset = state.hscroll.limited_offset(state.hscroll.offset);
}

fn render_evaluation_overlay(app: &App, frame: &mut Frame) {
    // Get full screen area
    let full_area = frame.area();

    // Calculate center overlay area with minimum size guarantees
    let overlay_width = full_area
        .width
        .saturating_mul(OVERLAY_SIZE_PERCENT)
        .saturating_div(100)
        .max(MIN_OVERLAY_WIDTH);
    let overlay_height = full_area
        .height
        .saturating_mul(OVERLAY_SIZE_PERCENT)
        .saturating_div(100)
        .max(MIN_OVERLAY_HEIGHT);
    let x = full_area.width.saturating_sub(overlay_width) / 2;
    let y = full_area.height.saturating_sub(overlay_height) / 2;

    let overlay_area = Rect {
        x,
        y,
        width: overlay_width,
        height: overlay_height,
    };

    // Create semi-transparent effect by dimming the background
    // Fill entire screen with dark gray to dim the content behind
    let dimmed_background = Block::default().style(Style::default().bg(Color::Rgb(20, 20, 20))); // Very dark gray
    frame.render_widget(dimmed_background, full_area);

    // Clear the overlay area explicitly to reset all cells
    frame.render_widget(Clear, overlay_area);

    // Fill overlay area with solid black background using a Paragraph
    let black_background = Paragraph::new("").style(Style::default().bg(Color::Black));
    frame.render_widget(black_background, overlay_area);

    // Determine border color based on pass/fail
    let border_color = if app.evaluation_passed {
        Color::Green
    } else {
        Color::Red
    };

    // Render the block with borders
    let block = Block::default()
        .title(" 評価結果 (e: 閉じる, Shift+↑/↓ or Shift+j/k: スクロール, n: 次の問題) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(Color::Black));

    // Calculate inner area (inside the borders)
    let inner_area = block.inner(overlay_area);

    // Render the block (borders)
    frame.render_widget(block, overlay_area);

    // Render the text
    let paragraph = Paragraph::new(app.evaluation_text.as_str())
        .wrap(Wrap { trim: false })
        .scroll((app.evaluation_overlay_scroll, 0))
        .style(Style::default().bg(Color::Black).fg(Color::White));

    frame.render_widget(paragraph, inner_area);
}

fn render_status_bar(app: &App, frame: &mut Frame, area: Rect) {
    let block = Block::default().borders(Borders::TOP);
    let status_text = format!(
        " {} | r: レポート | h: ヘルプ | q: 終了 ",
        app.status_message
    );
    let paragraph = Paragraph::new(status_text)
        .alignment(Alignment::Right)
        .block(block);
    frame.render_widget(paragraph, area);
}

fn render_report_view(app: &App, frame: &mut Frame) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Header
            Constraint::Min(0),    // Report
            Constraint::Length(3), // Status
        ])
        .split(frame.area());

    render_header(frame, layout[0]);
    reports::render_unified_report(frame, layout[1], &app.stats);
    render_status_bar(app, frame, layout[2]);
}

fn render_menu_view(app: &App, frame: &mut Frame) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Header
            Constraint::Min(0),    // Menu
            Constraint::Length(3), // Status
        ])
        .split(frame.area());

    render_header(frame, layout[0]);

    // Center the menu box
    let menu_area = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Length(16),
            Constraint::Percentage(20),
        ])
        .split(layout[1])[1];

    let menu_area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(40),
            Constraint::Percentage(30),
        ])
        .split(menu_area)[1];

    let block = Block::default()
        .title("文字数を選択してください")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let mut menu_text = String::new();
    menu_text.push_str("\n\n");

    for (i, &count) in MENU_OPTIONS.iter().enumerate() {
        if i == app.selected_menu_item {
            menu_text.push_str(&format!("  > {} 文字 <\n\n", count));
        } else {
            menu_text.push_str(&format!("    {} 文字\n\n", count));
        }
    }

    let paragraph = Paragraph::new(menu_text)
        .block(block)
        .alignment(Alignment::Center)
        .style(Style::default());

    frame.render_widget(paragraph, menu_area);
    render_status_bar(app, frame, layout[2]);
}

fn render_help_view(app: &App, frame: &mut Frame) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Header
            Constraint::Min(0),    // Help content
            Constraint::Length(3), // Status
        ])
        .split(frame.area());

    render_header(frame, layout[0]);

    let help_content = help::get_help_content();
    let help_text = if help_content.is_empty() {
        "ヘルプファイルが見つかりません。\n\ndocs/HELP.md を作成してください。".to_string()
    } else {
        help_content.to_string()
    };

    let block = Block::default()
        .title("ヘルプ (↑/↓ or j/k: スクロール, h: 閉じる)")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));

    let paragraph = Paragraph::new(help_text)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.help_scroll, 0))
        .style(Style::default());

    frame.render_widget(paragraph, layout[1]);
    render_status_bar(app, frame, layout[2]);
}
