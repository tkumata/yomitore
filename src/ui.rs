use crate::app::App;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Wrap},
};

/// Renders the user interface widgets.
pub fn render(app: &mut App, frame: &mut Frame) {
    let main_layout = if app.show_evaluation {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),      // Header
                Constraint::Percentage(30), // Original Text
                Constraint::Percentage(25), // Summary Input
                Constraint::Min(0),         // Evaluation Result
                Constraint::Length(3),      // Status
            ])
            .split(frame.area())
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),      // Header
                Constraint::Percentage(45), // Original Text
                Constraint::Min(0),         // Summary Input
                Constraint::Length(3),      // Status
            ])
            .split(frame.area())
    };

    render_header(frame, main_layout[0]);
    render_original_text(app, frame, main_layout[1]);
    render_summary_input(app, frame, main_layout[2]);

    if app.show_evaluation {
        render_evaluation(app, frame, main_layout[3]);
        render_status_bar(app, frame, main_layout[4]);
    } else {
        render_status_bar(app, frame, main_layout[3]);
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
        .borders(Borders::ALL);
    let paragraph = Paragraph::new(app.original_text.as_str())
        .wrap(Wrap { trim: false })
        .scroll((app.original_text_scroll, 0))
        .block(block);
    frame.render_widget(paragraph, area);
}

fn render_summary_input(app: &mut App, frame: &mut Frame, area: Rect) {
    let title = "あなたの要約 (i:入力モード Esc:通常モード Ctrl+S:送信)";

    let border_style = if app.is_editing {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    // Create wrapped text display with cursor
    let text = if app.summary_input.is_empty() {
        "Press 'i' to start typing...".to_string()
    } else {
        app.summary_input.clone()
    };

    // Add cursor visual indicator when in edit mode
    let display_text = if app.is_editing {
        let before = &text[..app.cursor_position.min(text.len())];
        let after = if app.cursor_position < text.len() {
            &text[app.cursor_position..]
        } else {
            " "
        };
        format!("{}█{}", before, after)
    } else {
        text
    };

    let paragraph = Paragraph::new(display_text)
        .block(block)
        .wrap(Wrap { trim: false })
        .style(Style::default());

    frame.render_widget(paragraph, area);
}

fn render_evaluation(app: &App, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .title("評価結果 (Shift+↑/↓ or Shift+j/k: スクロール, n: 次のトレーニング)")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));

    let paragraph = Paragraph::new(app.evaluation_text.as_str())
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.evaluation_text_scroll, 0))
        .style(Style::default());

    frame.render_widget(paragraph, area);
}

fn render_status_bar(app: &App, frame: &mut Frame, area: Rect) {
    let block = Block::default().borders(Borders::TOP);
    let status_text = format!(" {} | q: 終了 ", app.status_message);
    let paragraph = Paragraph::new(status_text)
        .alignment(Alignment::Right)
        .block(block);
    frame.render_widget(paragraph, area);
}
