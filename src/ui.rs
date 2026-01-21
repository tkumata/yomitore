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
const OVERLAY_MARGIN: u16 = 2;
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
    let full_area = frame.area();
    let overlay_area = calculate_overlay_area(full_area);
    let margin = OVERLAY_MARGIN;

    let outer_area = Rect {
        x: overlay_area.x.saturating_sub(margin),
        y: overlay_area.y.saturating_sub(margin),
        width: overlay_area.width.saturating_add(margin.saturating_mul(2)),
        height: overlay_area.height.saturating_add(margin.saturating_mul(2)),
    };

    // 余白リングだけを消して、外側の本文は残す
    if margin > 0 {
        let top = Rect {
            x: outer_area.x,
            y: outer_area.y,
            width: outer_area.width,
            height: margin,
        };
        let bottom = Rect {
            x: outer_area.x,
            y: outer_area.y + outer_area.height.saturating_sub(margin),
            width: outer_area.width,
            height: margin,
        };
        let left = Rect {
            x: outer_area.x,
            y: outer_area.y.saturating_add(margin),
            width: margin,
            height: outer_area.height.saturating_sub(margin.saturating_mul(2)),
        };
        let right = Rect {
            x: outer_area.x + outer_area.width.saturating_sub(margin),
            y: outer_area.y.saturating_add(margin),
            width: margin,
            height: outer_area.height.saturating_sub(margin.saturating_mul(2)),
        };

        frame.render_widget(Clear, top);
        frame.render_widget(Clear, bottom);
        frame.render_widget(Clear, left);
        frame.render_widget(Clear, right);
    }

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

fn calculate_overlay_area(full_area: Rect) -> Rect {
    let margin = OVERLAY_MARGIN;

    let max_overlay_width = full_area.width.saturating_sub(margin.saturating_mul(2));
    let max_overlay_height = full_area.height.saturating_sub(margin.saturating_mul(2));

    // 余白リングを確保した上でオーバーレイを中央に配置する
    let overlay_width = full_area
        .width
        .saturating_mul(OVERLAY_SIZE_PERCENT)
        .saturating_div(100)
        .max(MIN_OVERLAY_WIDTH)
        .min(max_overlay_width);
    let overlay_height = full_area
        .height
        .saturating_mul(OVERLAY_SIZE_PERCENT)
        .saturating_div(100)
        .max(MIN_OVERLAY_HEIGHT)
        .min(max_overlay_height);
    let x = full_area.x + full_area.width.saturating_sub(overlay_width) / 2;
    let y = full_area.y + full_area.height.saturating_sub(overlay_height) / 2;

    Rect {
        x,
        y,
        width: overlay_width,
        height: overlay_height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_overlay_area_standard() {
        let full_area = Rect::new(0, 0, 100, 40);
        let overlay = calculate_overlay_area(full_area);

        // 75% of 100 is 75, 75% of 40 is 30
        assert_eq!(overlay.width, 75);
        assert_eq!(overlay.height, 30);
        assert_eq!(overlay.x, 12); // (100 - 75) / 2 = 12.5 -> 12
        assert_eq!(overlay.y, 5); // (40 - 30) / 2 = 5
    }

    #[test]
    fn test_calculate_overlay_area_min_size_constraint() {
        // Small screen: 75% would be 30x7.5, which is below MIN_OVERLAY_WIDTH(40) and MIN_OVERLAY_HEIGHT(10)
        let full_area = Rect::new(0, 0, 40, 10);
        let overlay = calculate_overlay_area(full_area);

        // Should be capped by max_overlay_width/height (saturating_sub(margin*2))
        // margin=2 -> max_width = 40-4 = 36, max_height = 10-4 = 6
        assert_eq!(overlay.width, 36);
        assert_eq!(overlay.height, 6);
    }

    #[test]
    fn test_calculate_overlay_area_margins_preserved() {
        let full_area = Rect::new(0, 0, 100, 40);
        let overlay = calculate_overlay_area(full_area);

        // Overlay should never be closer than OVERLAY_MARGIN to any edge
        assert!(overlay.x >= OVERLAY_MARGIN);
        assert!(overlay.y >= OVERLAY_MARGIN);
        assert!(overlay.x + overlay.width <= full_area.width - OVERLAY_MARGIN);
        assert!(overlay.y + overlay.height <= full_area.height - OVERLAY_MARGIN);
    }
}
