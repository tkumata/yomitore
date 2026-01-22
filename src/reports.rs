use crate::stats::{DailyStats, TrainingStats, WeeklyStats};
use chrono::{Datelike, Local, NaiveDate};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};
use std::collections::HashMap;

const DAYS_IN_MONTH: usize = 30;
const WEEKS_TO_SHOW: usize = 4;
/// Maximum number of badges to display in report
const MAX_BADGES_DISPLAY: usize = 20;

const PET_LEVEL_1: &str = r#"
     ヘ＿ヘ
    ミ・・ ミ
      =o==
      Kitty"#;

const PET_LEVEL_2: &str = r#"
     ヘ＿ヘ
    ミ. . ミ
      =o==
      Cat"#;

const PET_LEVEL_3: &str = r#"
     ヘ_ヘ
    ミ. . ミ
     (    ) 〜
    Hemi Neko"#;

fn get_pet_ascii(level: u32) -> &'static str {
    let art = match level {
        1 => PET_LEVEL_1,
        2 => PET_LEVEL_2,
        _ => PET_LEVEL_3,
    };
    art.strip_prefix('\n').unwrap_or(art)
}

/// Renders badge section common to both reports
fn render_badge_section(stats: &TrainingStats) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let (consecutive_badges, cumulative_badges) = stats.get_badges_by_type();

    // Consecutive streak badges (🔥)
    if !consecutive_badges.is_empty() {
        let mut badge_line = vec![Span::styled(
            "🔥 連続正解: ",
            Style::default().fg(Color::Yellow).bold(),
        )];
        for badge in consecutive_badges.iter().take(10) {
            badge_line.push(Span::raw(format!(
                "{}{} ",
                badge.get_icon(),
                badge.get_display_text()
            )));
        }
        lines.push(Line::from(badge_line));
    }

    // Cumulative milestone badges (✨)
    if !cumulative_badges.is_empty() {
        let mut badge_line = vec![Span::styled(
            "✨ 累積正解: ",
            Style::default().fg(Color::Cyan).bold(),
        )];
        for badge in cumulative_badges.iter().take(MAX_BADGES_DISPLAY) {
            badge_line.push(Span::raw(format!(
                "{}{} ",
                badge.get_icon(),
                badge.get_display_text()
            )));
        }
        lines.push(Line::from(badge_line));
    }

    if !consecutive_badges.is_empty() || !cumulative_badges.is_empty() {
        lines.push(Line::from(""));
    }

    lines
}

fn render_evaluation_summary(stats: &TrainingStats) -> Vec<Line<'static>> {
    let summary = stats.get_recent_evaluation_summary(DAYS_IN_MONTH);
    let mut lines = Vec::new();

    lines.push(Line::from(Span::styled(
        "評価スコア (直近30日)",
        Style::default().fg(Color::Cyan).bold(),
    )));

    if summary.count == 0 {
        lines.push(Line::from("評価スコア: なし"));
        lines.push(Line::from("件数: 0"));
        return lines;
    }

    let importance = summary.importance.as_ref().unwrap();
    let conciseness = summary.conciseness.as_ref().unwrap();
    let accuracy = summary.accuracy.as_ref().unwrap();

    lines.push(Line::from(format!(
        "重要情報: 平均 {:.1} / 中央値 {:.1}",
        importance.average, importance.median
    )));
    lines.push(Line::from(format!(
        "簡潔性: 平均 {:.1} / 中央値 {:.1}",
        conciseness.average, conciseness.median
    )));
    lines.push(Line::from(format!(
        "正確性: 平均 {:.1} / 中央値 {:.1}",
        accuracy.average, accuracy.median
    )));
    lines.push(Line::from(format!("件数: {}", summary.count)));

    lines
}

pub fn render_unified_report(frame: &mut Frame, area: Rect, stats: &TrainingStats) {
    let block = Block::default()
        .title("レポート (r: 閉じる)")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Split vertically: badges on top, reports below
    let vertical_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6), // Badges & Pet (needs 4 lines: 3 for Art + 1 for Exp)
            Constraint::Min(0),    // Reports
        ])
        .split(inner);

    // Split the top area horizontally: Badges (Left), Pet (Right)
    let top_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(vertical_layout[0]);

    // Render badges block at the top left
    let badge_block = Block::default()
        .title("バッジ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let badge_inner = badge_block.inner(top_layout[0]);
    frame.render_widget(badge_block, top_layout[0]);
    let badge_content = Text::from(render_badge_section(stats));
    let badge_paragraph = Paragraph::new(badge_content);
    frame.render_widget(badge_paragraph, badge_inner);

    // Render Pet at the top right
    let pet_block = Block::default()
        .title(format!("ペット (Lv.{})", stats.pet.level))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::LightBlue));
    let pet_inner = pet_block.inner(top_layout[1]);
    frame.render_widget(pet_block, top_layout[1]);

    let pet_ascii = get_pet_ascii(stats.pet.level);
    let pet_text = format!("{}\n    Exp: {}/5", pet_ascii, stats.pet.exp);
    let pet_paragraph = Paragraph::new(pet_text);
    frame.render_widget(pet_paragraph, pet_inner);

    // Split the bottom area horizontally: left for monthly, right for weekly
    let horizontal_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(vertical_layout[1]);

    // Render monthly report on the left
    let daily_stats = stats.get_daily_stats(DAYS_IN_MONTH);
    let monthly_block = Block::default()
        .title("月次 (過去30日)")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));
    let monthly_inner = monthly_block.inner(horizontal_layout[0]);
    frame.render_widget(monthly_block, horizontal_layout[0]);
    if monthly_inner.height >= 6 {
        let monthly_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(6), Constraint::Min(0)])
            .split(monthly_inner);
        let summary_text = Text::from(render_evaluation_summary(stats));
        let summary_paragraph = Paragraph::new(summary_text);
        frame.render_widget(summary_paragraph, monthly_layout[0]);

        let heatmap = create_heatmap_without_badges(
            &daily_stats,
            monthly_layout[1].width as usize,
            monthly_layout[1].height as usize,
        );
        let paragraph = Paragraph::new(heatmap);
        frame.render_widget(paragraph, monthly_layout[1]);
    } else {
        let heatmap = create_heatmap_without_badges(
            &daily_stats,
            monthly_inner.width as usize,
            monthly_inner.height as usize,
        );
        let paragraph = Paragraph::new(heatmap);
        frame.render_widget(paragraph, monthly_inner);
    }

    // Render weekly report on the right
    let weekly_stats = stats.get_weekly_stats(WEEKS_TO_SHOW);
    let weekly_block = Block::default()
        .title("週次 (過去4週)")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));
    let weekly_inner = weekly_block.inner(horizontal_layout[1]);
    frame.render_widget(weekly_block, horizontal_layout[1]);
    let chart = create_bar_chart_without_badges(
        &weekly_stats,
        weekly_inner.width as usize,
        weekly_inner.height as usize,
    );
    let paragraph = Paragraph::new(chart);
    frame.render_widget(paragraph, weekly_inner);
}

fn create_heatmap_without_badges(
    daily_stats: &HashMap<NaiveDate, DailyStats>,
    _width: usize,
    _height: usize,
) -> Text<'static> {
    let mut lines = Vec::new();
    let today = Local::now().date_naive();

    // Calculate grid dimensions (7 columns for days of week, multiple rows for weeks)
    let cols = 7;
    let rows = DAYS_IN_MONTH.div_ceil(7); // Round up to include partial weeks

    // Create week day labels
    let weekdays = vec!["日", "月", "火", "水", "木", "金", "土"];
    let mut header = vec![Span::raw("    ")];
    for day in &weekdays {
        header.push(Span::raw(format!(" {} ", day)));
    }
    lines.push(Line::from(header));

    // Build a grid structure: rows x 7 columns
    // Start from 30 days ago and go forward to today
    let start_date = today - chrono::Duration::days((DAYS_IN_MONTH - 1) as i64);

    // Find the Sunday on or before start_date to align the grid properly
    let start_weekday = start_date.weekday().num_days_from_sunday();
    let grid_start = start_date - chrono::Duration::days(start_weekday as i64);

    // Calculate number of days in grid
    let days_until_today = (today - grid_start).num_days() + 1;
    let grid_rows = (days_until_today as usize).div_ceil(7).min(rows);

    // Generate heatmap grid
    for row in 0..grid_rows {
        let mut line_spans = Vec::new();

        // Week label
        let row_start_date = grid_start + chrono::Duration::days((row * 7) as i64);
        line_spans.push(Span::raw(format!(
            "W{:02} ",
            row_start_date.iso_week().week()
        )));

        for col in 0..cols {
            let date = row_start_date + chrono::Duration::days(col as i64);

            // Check if date is in our range (from start_date to today)
            if date < start_date || date > today {
                line_spans.push(Span::raw("    "));
                continue;
            }

            if let Some(stats) = daily_stats.get(&date) {
                let total = stats.total();
                let correct = stats.correct;

                // Determine color intensity based on correct answers
                let (symbol, style) = get_heatmap_cell_style(total, correct);

                line_spans.push(Span::styled(format!(" {} ", symbol), style));
            } else {
                line_spans.push(Span::raw(" -- "));
            }
        }

        lines.push(Line::from(line_spans));
    }

    // Legend
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::raw("凡例: "),
        Span::styled("--", Style::default().fg(Color::DarkGray)),
        Span::raw(" なし  "),
        Span::styled("##", Style::default().fg(Color::Red)),
        Span::raw(" 全不正解  "),
        Span::styled("##", Style::default().fg(Color::Yellow)),
        Span::raw(" 混在  "),
        Span::styled("##", Style::default().fg(Color::LightGreen)),
        Span::raw(" 良  "),
        Span::styled("##", Style::default().fg(Color::Green)),
        Span::raw(" 優  "),
        Span::styled("##", Style::default().fg(Color::Rgb(0, 255, 0)).bold()),
        Span::raw(" 秀"),
    ]));

    Text::from(lines)
}

fn create_bar_chart_without_badges(
    weekly_stats: &[WeeklyStats],
    width: usize,
    _height: usize,
) -> Text<'static> {
    let mut lines = Vec::new();

    // Find max value for scaling
    let max_value = weekly_stats
        .iter()
        .map(|s| s.correct.max(s.incorrect))
        .max()
        .unwrap_or(1);

    // Calculate max bar width based on available width
    // Reserve space for label "第XX週: " (~7 chars) and number suffix (~4 chars)
    let max_bar_width = width.saturating_sub(15).max(10);

    // Display each week
    for stats in weekly_stats {
        let correct_bars = calculate_bar_height(stats.correct, max_value, max_bar_width);
        let incorrect_bars = calculate_bar_height(stats.incorrect, max_value, max_bar_width);

        let mut line_spans = vec![Span::raw(format!("第{}週: ", stats.week_number))];

        // Correct bar (green)
        line_spans.push(Span::styled(
            "█".repeat(correct_bars),
            Style::default().fg(Color::Green),
        ));
        line_spans.push(Span::raw(format!(" {}", stats.correct)));

        lines.push(Line::from(line_spans));

        // Incorrect bar (red)
        let mut incorrect_line = vec![Span::raw("       ")];
        incorrect_line.push(Span::styled(
            "█".repeat(incorrect_bars),
            Style::default().fg(Color::Red),
        ));
        incorrect_line.push(Span::raw(format!(" {}", stats.incorrect)));

        lines.push(Line::from(incorrect_line));
        lines.push(Line::from(""));
    }

    // Legend
    lines.push(Line::from(vec![
        Span::raw("凡例: "),
        Span::styled("█", Style::default().fg(Color::Green)),
        Span::raw(" 正解  "),
        Span::styled("█", Style::default().fg(Color::Red)),
        Span::raw(" 不正解"),
    ]));

    Text::from(lines)
}

fn get_heatmap_cell_style(total: usize, correct: usize) -> (&'static str, Style) {
    if total == 0 {
        return ("--", Style::default().fg(Color::DarkGray));
    }

    if correct == 0 {
        return ("##", Style::default().fg(Color::Red));
    }

    if correct == total {
        return ("##", Style::default().fg(Color::Rgb(0, 255, 0)).bold());
    }

    let ratio = correct as f64 / total as f64;
    let color = if ratio >= 0.8 {
        Color::Green
    } else if ratio >= 0.5 {
        Color::LightGreen
    } else {
        Color::Yellow
    };

    ("##", Style::default().fg(color))
}

fn calculate_bar_height(value: usize, max_value: usize, max_len: usize) -> usize {
    if max_value == 0 {
        return 0;
    }
    let len = (value as f64 * max_len as f64 / max_value as f64).round() as usize;
    len.min(max_len)
}
