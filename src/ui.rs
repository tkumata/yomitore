use crate::app::{App, ViewMode};
use crate::reports;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Wrap},
};

/// Renders the user interface widgets.
pub fn render(app: &mut App, frame: &mut Frame) {
    // Check if we should show a report instead of the normal view
    match app.view_mode {
        ViewMode::Menu => {
            render_menu_view(app, frame);
            return;
        }
        ViewMode::MonthlyReport => {
            render_monthly_report_view(app, frame);
            return;
        }
        ViewMode::WeeklyReport => {
            render_weekly_report_view(app, frame);
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
            Constraint::Length(1),  // Header
            Constraint::Min(0),     // Content (3 blocks)
            Constraint::Length(3),  // Status
        ])
        .split(frame.area());

    render_header(frame, main_layout[0]);

    // Content layout: Left (Original) and Right (Answer + Evaluation)
    let content_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50), // Left: Original text
            Constraint::Percentage(50), // Right: Answer + Evaluation
        ])
        .split(main_layout[1]);

    // Right side layout: Answer (top) and Evaluation (bottom)
    let right_layout = if app.show_evaluation {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(50), // Answer block
                Constraint::Percentage(50), // Evaluation block
            ])
            .split(content_layout[1])
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(100), // Answer block only
            ])
            .split(content_layout[1])
    };

    // Render blocks
    render_original_text(app, frame, content_layout[0]);
    render_summary_input(app, frame, right_layout[0]);

    if app.show_evaluation {
        render_evaluation(app, frame, right_layout[1]);
    }

    render_status_bar(app, frame, main_layout[2]);
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

    let border_style = if app.is_editing {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::Blue)
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
    let border_color = if app.evaluation_passed {
        Color::Green
    } else {
        Color::Red
    };

    let block = Block::default()
        .title("評価結果 (Shift+↑/↓ or Shift+j/k: スクロール, n: 次のトレーニング)")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let paragraph = Paragraph::new(app.evaluation_text.as_str())
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.evaluation_text_scroll, 0))
        .style(Style::default());

    frame.render_widget(paragraph, area);
}

fn render_status_bar(app: &App, frame: &mut Frame, area: Rect) {
    let block = Block::default().borders(Borders::TOP);
    let status_text = format!(" {} | m: 月次 | w: 週次 | q: 終了 ", app.status_message);
    let paragraph = Paragraph::new(status_text)
        .alignment(Alignment::Right)
        .block(block);
    frame.render_widget(paragraph, area);
}

fn render_monthly_report_view(app: &App, frame: &mut Frame) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),      // Header
            Constraint::Min(0),         // Report
            Constraint::Length(3),      // Status
        ])
        .split(frame.area());

    render_header(frame, layout[0]);
    reports::render_monthly_report(frame, layout[1], &app.stats);
    render_status_bar(app, frame, layout[2]);
}

fn render_weekly_report_view(app: &App, frame: &mut Frame) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),      // Header
            Constraint::Min(0),         // Report
            Constraint::Length(3),      // Status
        ])
        .split(frame.area());

    render_header(frame, layout[0]);
    reports::render_weekly_report(frame, layout[1], &app.stats);
    render_status_bar(app, frame, layout[2]);
}

fn render_menu_view(app: &App, frame: &mut Frame) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),      // Header
            Constraint::Min(0),         // Menu
            Constraint::Length(3),      // Status
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

    let menu_options = vec![400, 720, 1440, 2880];
    let mut menu_text = String::new();
    menu_text.push_str("\n\n");

    for (i, &count) in menu_options.iter().enumerate() {
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
