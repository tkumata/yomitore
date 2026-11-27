use crate::app::{App, MENU_OPTIONS, OVERLAY_MARGIN, SettingsFocus, ViewMode};
use crate::evaluation::Outcome;
use crate::help;
use crate::reports;
use ratatui::{
    buffer::CellWidth,
    prelude::*,
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Padding, Paragraph, Wrap},
};

const MENU_TITLE_ART: [&str; 6] = [
    "██╗   ██╗ ██████╗ ███╗   ███╗██╗████████╗ ██████╗ ██████╗ ███████╗",
    "╚██╗ ██╔╝██╔═══██╗████╗ ████║██║╚══██╔══╝██╔═══██╗██╔══██╗██╔════╝",
    " ╚████╔╝ ██║   ██║██╔████╔██║██║   ██║   ██║   ██║██████╔╝█████╗  ",
    "  ╚██╔╝  ██║   ██║██║╚██╔╝██║██║   ██║   ██║   ██║██╔══██╗██╔══╝  ",
    "   ██║   ╚██████╔╝██║ ╚═╝ ██║██║   ██║   ╚██████╔╝██║  ██║███████╗",
    "   ╚═╝    ╚═════╝ ╚═╝     ╚═╝╚═╝   ╚═╝    ╚═════╝ ╚═╝  ╚═╝╚══════╝",
];
const MENU_TITLE_COLOR: Color = Color::LightBlue;
const MENU_LOGO_GAP_HEIGHT: u16 = 1;
const MENU_TITLE_BLOCK_GAP_HEIGHT: u16 = 3;

pub fn render(app: &mut App, frame: &mut Frame) {
    app.update_terminal_size(frame.area().width, frame.area().height);

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
        ViewMode::Settings => {
            render_settings_view(app, frame);
            return;
        }
        ViewMode::Normal | ViewMode::Editing => {}
    }

    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(frame.area());

    let [header_area, body_area, status_area] = main_layout.as_ref() else {
        return;
    };
    render_header(app, frame, *header_area);

    let content_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(*body_area);
    let [original_area, summary_area] = content_layout.as_ref() else {
        return;
    };

    render_original_text(app, frame, *original_area);
    render_summary_input(app, frame, *summary_area);

    if app.show_evaluation_overlay {
        render_evaluation_overlay(app, frame);
    }

    render_status_bar(app, frame, *status_area);
}

fn render_header(app: &App, frame: &mut Frame, area: Rect) {
    let title = if app.diagnosis_mode {
        " yomitore: 診断モード（成績は保存しません） "
    } else {
        " yomitore: 読解力トレーニング "
    };
    let title = Paragraph::new(title)
        .style(Style::new().bold())
        .alignment(Alignment::Center);
    frame.render_widget(title, area);
}

fn render_original_text(app: &mut App, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .title("原文 (↑/↓ or j/k: スクロール)")
        .borders(Borders::ALL);
    let inner = block.inner(area);
    let text = app
        .original_text
        .as_deref()
        .or(app.generation_error.as_deref())
        .unwrap_or("原文を生成しています...");
    let lines = wrap_original_text(text, inner.width);
    let max_scroll = lines.len().saturating_sub(usize::from(inner.height));
    app.original_text_scroll = app
        .original_text_scroll
        .min(u16::try_from(max_scroll).unwrap_or(u16::MAX));
    let paragraph = Paragraph::new(lines)
        .scroll((app.original_text_scroll, 0))
        .block(block);
    frame.render_widget(paragraph, area);
}

fn wrap_original_text(text: &str, width: u16) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    // Ratatui 0.30.2 can wrap a wide grapheme across the pane border.
    for source_line in text.lines() {
        let mut line = String::new();
        let mut line_width = 0;
        for grapheme in Span::raw(source_line).styled_graphemes(Style::default()) {
            let grapheme_width = grapheme.symbol.cell_width();
            if grapheme_width > width {
                continue;
            }
            if grapheme_width > width - line_width {
                lines.push(Line::raw(std::mem::take(&mut line)));
                line_width = 0;
            }
            line.push_str(grapheme.symbol);
            line_width += grapheme_width;
        }
        lines.push(Line::raw(line));
    }
    lines
}

fn render_summary_input(app: &App, frame: &mut Frame, area: Rect) {
    let title = "あなたの要約 (i:入力モード Esc:通常モード Ctrl+S:送信)";

    let border_style = if app.view_mode == ViewMode::Editing {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.is_empty() {
        return;
    }
    frame.render_widget(&app.summary_input, inner);
    if app.view_mode == ViewMode::Editing {
        let cursor = app.summary_input.screen_cursor();
        let (top, left) = app.summary_input.scroll_offset();
        let x = cursor.col.saturating_sub(usize::from(left));
        let y = cursor.row.saturating_sub(usize::from(top));
        if let Ok(y) = u16::try_from(y)
            && y < inner.height
        {
            let x = u16::try_from(x).unwrap_or(u16::MAX).min(inner.width - 1);
            frame.set_cursor_position((inner.x + x, inner.y + y));
        }
    }
}

fn render_evaluation_overlay(app: &App, frame: &mut Frame) {
    let overlay_area = app.calculate_overlay_area();

    let outer_area = Rect {
        x: overlay_area.x.saturating_sub(OVERLAY_MARGIN),
        y: overlay_area.y.saturating_sub(OVERLAY_MARGIN),
        width: overlay_area
            .width
            .saturating_add(OVERLAY_MARGIN.saturating_mul(2)),
        height: overlay_area
            .height
            .saturating_add(OVERLAY_MARGIN.saturating_mul(2)),
    };

    if OVERLAY_MARGIN > 0 {
        let top = Rect {
            x: outer_area.x,
            y: outer_area.y,
            width: outer_area.width,
            height: OVERLAY_MARGIN,
        };
        let bottom = Rect {
            x: outer_area.x,
            y: outer_area
                .y
                .saturating_add(outer_area.height.saturating_sub(OVERLAY_MARGIN)),
            width: outer_area.width,
            height: OVERLAY_MARGIN,
        };
        let left = Rect {
            x: outer_area.x,
            y: outer_area.y.saturating_add(OVERLAY_MARGIN),
            width: OVERLAY_MARGIN,
            height: outer_area
                .height
                .saturating_sub(OVERLAY_MARGIN.saturating_mul(2)),
        };
        let right = Rect {
            x: outer_area
                .x
                .saturating_add(outer_area.width.saturating_sub(OVERLAY_MARGIN)),
            y: outer_area.y.saturating_add(OVERLAY_MARGIN),
            width: OVERLAY_MARGIN,
            height: outer_area
                .height
                .saturating_sub(OVERLAY_MARGIN.saturating_mul(2)),
        };

        frame.render_widget(Clear, top);
        frame.render_widget(Clear, bottom);
        frame.render_widget(Clear, left);
        frame.render_widget(Clear, right);
    }

    frame.render_widget(Clear, overlay_area);

    let black_background = Paragraph::new("").style(Style::default().bg(Color::Black));
    frame.render_widget(black_background, overlay_area);

    let border_color = match app.evaluation_outcome {
        Some(Outcome::Pass) => Color::Green,
        Some(Outcome::Review) => Color::Yellow,
        Some(Outcome::Fail) | None => Color::Red,
    };

    let block = Block::default()
        .title(" 評価結果 (e: 閉じる, Shift+↑/↓ or Shift+j/k: スクロール, n: 次の問題) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(Color::Black));

    let inner_area = block.inner(overlay_area);

    frame.render_widget(block, overlay_area);

    let paragraph = Paragraph::new(app.evaluation_text.as_str())
        .wrap(Wrap { trim: false })
        .scroll((app.evaluation_overlay_scroll, 0))
        .style(Style::default().bg(Color::Black).fg(Color::White));

    frame.render_widget(paragraph, inner_area);
}

fn render_status_bar(app: &App, frame: &mut Frame, area: Rect) {
    let block = Block::default().borders(Borders::TOP);
    let status_message = &app.status_message;
    let settings_hint = if matches!(
        app.view_mode,
        ViewMode::Menu | ViewMode::Normal | ViewMode::Editing
    ) {
        " | s: 設定"
    } else {
        ""
    };
    let status_text =
        format!(" {status_message} | r: レポート | h: ヘルプ{settings_hint} | q: 終了 ");
    let mut lines = vec![Line::from(status_text)];
    if let Some(error) = &app.stats_load_error {
        lines.push(Line::from(error.as_str()).style(Style::default().fg(Color::Yellow)));
    } else if app.pending_evaluation.is_some() {
        lines.push(
            Line::from("成績は未保存です。Ctrl+S で保存を再試行します。")
                .style(Style::default().fg(Color::Yellow)),
        );
    }
    let paragraph = Paragraph::new(lines)
        .alignment(Alignment::Right)
        .block(block);
    frame.render_widget(paragraph, area);
}

fn render_report_view(app: &App, frame: &mut Frame) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(frame.area());
    let [header_area, body_area, status_area] = layout.as_ref() else {
        return;
    };
    render_header(app, frame, *header_area);
    reports::render_unified_report(frame, *body_area, &app.stats);
    render_status_bar(app, frame, *status_area);
}

fn render_menu_view(app: &App, frame: &mut Frame) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(frame.area());
    let [header_area, body_area, status_area] = layout.as_ref() else {
        return;
    };
    frame.render_widget(Paragraph::new(""), *header_area);

    let body_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(menu_logo_height()),
            Constraint::Length(MENU_LOGO_GAP_HEIGHT),
            Constraint::Length(1),
            Constraint::Length(MENU_TITLE_BLOCK_GAP_HEIGHT),
            Constraint::Length(menu_block_height()),
            Constraint::Min(0),
        ])
        .split(*body_area);
    let [logo_area, _, title_area, _, menu_area, _] = body_layout.as_ref() else {
        return;
    };

    let logo = Paragraph::new(build_menu_title_lines())
        .alignment(Alignment::Center)
        .style(Style::default().fg(MENU_TITLE_COLOR));
    frame.render_widget(logo, *logo_area);

    let title = Paragraph::new(" yomitore: 読解力トレーニング ")
        .style(Style::new().bold())
        .alignment(Alignment::Center);
    frame.render_widget(title, *title_area);

    let menu_area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(40),
            Constraint::Percentage(30),
        ])
        .split(*menu_area);
    let [_, menu_area, _] = menu_area.as_ref() else {
        return;
    };

    let block = Block::default()
        .title("トレーニング")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL);

    let menu_lines = build_menu_lines(app.selected_menu_item, app.diagnosis_mode);

    let paragraph = Paragraph::new(menu_lines)
        .block(block)
        .alignment(Alignment::Center)
        .style(Style::default());

    frame.render_widget(paragraph, *menu_area);
    render_status_bar(app, frame, *status_area);
}

fn render_settings_view(app: &App, frame: &mut Frame) {
    let settings = &app.settings;
    let block = Block::default()
        .title("API キー設定")
        .borders(Borders::ALL)
        .padding(Padding::new(2, 2, 1, 1));
    let inner = block.inner(frame.area());
    frame.render_widget(block, frame.area());
    if inner.is_empty() {
        return;
    }

    let groq_state = if settings.groq_from_env {
        "GROQ_API_KEY 環境変数を使用中"
    } else if settings.groq_saved {
        "保存済み"
    } else {
        "未設定"
    };
    let jev_state = if settings.jev_saved {
        "保存済み"
    } else {
        "未設定"
    };
    let fields = [
        (
            SettingsFocus::Groq,
            "Groq",
            groq_state,
            &settings.groq_input,
            settings.groq_saved,
        ),
        (
            SettingsFocus::Jev,
            "Jev",
            jev_state,
            &settings.jev_input,
            settings.jev_saved,
        ),
    ];
    let mut lines = Vec::new();
    for (focus, service, state, input, saved) in fields {
        lines.push(Line::from(Span::styled(
            format!("{service} API キー: ({state})"),
            Style::default().fg(Color::Yellow),
        )));
        let focused = settings.focus == focus;
        let length = input.chars().count();
        if focused
            && let Ok(row) = u16::try_from(lines.len())
            && row < inner.height
        {
            let column = u16::try_from(length)
                .unwrap_or(u16::MAX)
                .min(inner.width - 1);
            frame.set_cursor_position((inner.x + column, inner.y + row));
        }
        let mask = if input.is_empty() && saved {
            "********".to_owned()
        } else {
            let width = inner.width - u16::from(focused);
            "*".repeat(length.min(usize::from(width)))
        };
        lines.push(Line::from(mask));
        lines.push(Line::default());
    }

    let button = |label, focus| {
        let mut style = Style::default().add_modifier(Modifier::REVERSED);
        if settings.focus == focus {
            style = style.add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
        }
        Span::styled(label, style)
    };
    lines.extend([
        Line::from(vec![
            button(" Save ", SettingsFocus::Save),
            Span::raw(" "),
            button(" Cancel ", SettingsFocus::Cancel),
        ]),
        Line::from("Tab / Shift+Tab: 移動 | Enter: 次へ・実行 | Esc: キャンセル"),
        Line::from("空欄のまま保存すると、現在のキーを保持します。"),
        Line::from(app.status_message.as_str()),
    ]);
    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_help_view(app: &App, frame: &mut Frame) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(frame.area());
    let [header_area, body_area, status_area] = layout.as_ref() else {
        return;
    };
    render_header(app, frame, *header_area);

    let help_content = help::HELP_CONTENT;
    let help_text = if help_content.is_empty() {
        "ヘルプファイルが見つかりません。\n\ndocs/HELP.md を作成してください。"
    } else {
        help_content
    };

    let block = Block::default()
        .title("ヘルプ (↑/↓ or j/k: スクロール, h: 閉じる)")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL);

    let paragraph = Paragraph::new(help_text)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.help_scroll, 0))
        .style(Style::default());

    frame.render_widget(paragraph, *body_area);
    render_status_bar(app, frame, *status_area);
}

fn build_menu_lines(selected_menu_item: usize, diagnosis_mode: bool) -> Vec<Line<'static>> {
    let mut lines = Vec::with_capacity(MENU_OPTIONS.len().saturating_add(3));
    lines.push(Line::default());
    for (index, &count) in MENU_OPTIONS.iter().enumerate() {
        lines.push(build_menu_option_line(
            format!("{count:>4} 文字"),
            index == selected_menu_item,
        ));
    }
    let state = if diagnosis_mode { "ON" } else { "OFF" };
    lines.push(build_menu_option_line(
        format!("診断モード {state} (Space)"),
        selected_menu_item == MENU_OPTIONS.len(),
    ));
    lines.push(Line::default());

    lines
}

fn build_menu_title_lines() -> Vec<Line<'static>> {
    MENU_TITLE_ART
        .into_iter()
        .map(|art| Line::from(Span::styled(art, Style::default().fg(MENU_TITLE_COLOR))))
        .collect()
}

fn menu_logo_height() -> u16 {
    u16::try_from(MENU_TITLE_ART.len()).unwrap_or(u16::MAX)
}

fn menu_options_height() -> u16 {
    u16::try_from(MENU_OPTIONS.len().saturating_add(1)).unwrap_or(u16::MAX)
}

fn menu_block_height() -> u16 {
    menu_options_height().saturating_add(4)
}

fn build_menu_option_line(label: String, is_selected: bool) -> Line<'static> {
    let style = if is_selected {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    Line::from(Span::styled(label, style))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evaluation::{Judgment, format_jev_evaluation_display};
    use ratatui::backend::TestBackend;

    #[test]
    #[expect(
        clippy::panic_in_result_fn,
        reason = "Test assertions report the failed invariant"
    )]
    fn evaluation_overlay_shows_each_outcome_with_a_distinct_label()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut scores =
            crate::evaluation::parse_jev_response(&crate::evaluation::tests::response())
                .map_err(|error| std::io::Error::other(format!("{error:?}")))?;
        let improvements = ["改善1".into(), "改善2".into(), "改善3".into()];
        let mut app = App::default();
        let mut terminal = Terminal::new(TestBackend::new(100, 30))?;
        let total = scores.total_score();
        for (probability, expected, color) in [
            (0.30, "優秀・合格", Color::Green),
            (0.5, "要確認", Color::Yellow),
            (0.70, "不合格（重大な誤り", Color::Red),
        ] {
            scores.critical_error = probability;
            let judgment = Judgment::from_scores(total, probability);
            app.finish_evaluation(
                format_jev_evaluation_display(&scores, total, judgment, &improvements),
                judgment.outcome(),
            );
            terminal.draw(|frame| render_evaluation_overlay(&app, frame))?;
            let cells = terminal.backend().buffer().content();
            let rendered: String = cells.iter().map(ratatui::buffer::Cell::symbol).collect();
            assert!(rendered.replace(' ', "").contains(expected));
            assert!(cells.iter().any(|cell| cell.fg == color));
        }
        Ok(())
    }

    #[test]
    fn test_calculate_overlay_area_standard() {
        let full_area = Rect::new(0, 0, 100, 40);
        let overlay = App::calculate_overlay_area_for_size(full_area.width, full_area.height);

        assert_eq!(overlay.width, 75);
        assert_eq!(overlay.height, 30);
        assert_eq!(overlay.x, 12);
        assert_eq!(overlay.y, 5);
    }

    #[test]
    fn test_calculate_overlay_area_min_size_constraint() {
        let full_area = Rect::new(0, 0, 40, 10);
        let overlay = App::calculate_overlay_area_for_size(full_area.width, full_area.height);

        assert_eq!(overlay.width, 36);
        assert_eq!(overlay.height, 6);
    }

    #[test]
    fn test_calculate_overlay_area_margins_preserved() {
        let full_area = Rect::new(0, 0, 100, 40);
        let overlay = App::calculate_overlay_area_for_size(full_area.width, full_area.height);

        assert!(overlay.x >= OVERLAY_MARGIN);
        assert!(overlay.y >= OVERLAY_MARGIN);
        assert!(overlay.x + overlay.width <= full_area.width - OVERLAY_MARGIN);
        assert!(overlay.y + overlay.height <= full_area.height - OVERLAY_MARGIN);
    }

    #[test]
    fn test_build_menu_lines_center_selected_without_widening() {
        let lines = build_menu_lines(1, false);

        assert_eq!(lines.len(), MENU_OPTIONS.len().saturating_add(3));
        assert_eq!(lines.first().map(|line| line.spans.len()), Some(0));
        assert_eq!(lines.last().map(|line| line.spans.len()), Some(0));

        let selected_line = lines.iter().find(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
                == " 720 文字"
        });
        assert!(selected_line.is_some(), "selected menu line should exist");
        let Some(selected_line) = selected_line else {
            return;
        };

        let selected_text: String = selected_line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect();
        assert_eq!(selected_text, " 720 文字");
        assert!(
            !selected_line.spans.is_empty(),
            "selected menu line should contain a span"
        );
        let Some(selected_span) = selected_line.spans.first() else {
            return;
        };
        assert_eq!(selected_span.style.fg, Some(Color::Cyan));
        assert!(selected_span.style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_build_menu_title_lines() {
        let lines = build_menu_title_lines();

        assert_eq!(lines.len(), MENU_TITLE_ART.len());
        let Some(first_line) = lines.first() else {
            return;
        };
        assert!(
            first_line
                .spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
                .starts_with("██╗")
        );
        let Some(first_span) = first_line.spans.first() else {
            return;
        };
        assert_eq!(first_span.style.fg, Some(MENU_TITLE_COLOR));
    }

    #[test]
    fn test_menu_layout_heights() {
        assert_eq!(menu_logo_height(), 6);
        assert_eq!(MENU_LOGO_GAP_HEIGHT, 1);
        assert_eq!(MENU_TITLE_BLOCK_GAP_HEIGHT, 3);
        assert_eq!(menu_options_height(), 5);
        assert_eq!(menu_block_height(), 9);
    }
}
