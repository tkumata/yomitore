use crate::stats::{DailyStats, TrainingStats, WeeklyStats};
use chrono::{Datelike, Local, NaiveDate};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};
use std::collections::HashMap;

const DAYS_IN_MONTH: usize = 30;
const WEEKS_TO_SHOW: usize = 4;

pub fn render_monthly_report(frame: &mut Frame, area: Rect, stats: &TrainingStats) {
    let daily_stats = stats.get_daily_stats(DAYS_IN_MONTH);

    let block = Block::default()
        .title("月次レポート (m: 閉じる)")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Create heatmap
    let heatmap = create_heatmap(&daily_stats, inner.width as usize, inner.height as usize);
    let paragraph = Paragraph::new(heatmap);
    frame.render_widget(paragraph, inner);
}

pub fn render_weekly_report(frame: &mut Frame, area: Rect, stats: &TrainingStats) {
    let weekly_stats = stats.get_weekly_stats(WEEKS_TO_SHOW);

    let block = Block::default()
        .title("週次レポート (w: 閉じる)")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Create bar chart
    let chart = create_bar_chart(&weekly_stats, inner.width as usize, inner.height as usize);
    let paragraph = Paragraph::new(chart);
    frame.render_widget(paragraph, inner);
}

fn create_heatmap(daily_stats: &HashMap<NaiveDate, DailyStats>, _width: usize, _height: usize) -> Text<'static> {
    let mut lines = Vec::new();
    let today = Local::now().date_naive();

    // Title
    lines.push(Line::from(vec![
        Span::styled("過去30日間の成績", Style::default().bold()),
    ]));
    lines.push(Line::from(""));

    // Calculate grid dimensions (7 columns for days of week, multiple rows for weeks)
    let cols = 7;
    let rows = (DAYS_IN_MONTH + 6) / 7; // Round up to include partial weeks

    // Create week day labels
    let weekdays = vec!["日", "月", "火", "水", "木", "金", "土"];
    let mut header = vec![Span::raw("    ")];
    for day in &weekdays {
        header.push(Span::raw(format!(" {} ", day)));
    }
    lines.push(Line::from(header));

    // Generate heatmap grid
    for row in 0..rows {
        let mut line_spans = Vec::new();

        // Week label
        let week_offset = rows - row - 1;
        let date = today - chrono::Duration::days((week_offset * 7) as i64);
        line_spans.push(Span::raw(format!("W{:02} ", date.iso_week().week())));

        for col in 0..cols {
            let days_ago = (week_offset * 7) + col;
            if days_ago >= DAYS_IN_MONTH {
                line_spans.push(Span::raw("   "));
                continue;
            }

            let date = today - chrono::Duration::days(days_ago as i64);

            if let Some(stats) = daily_stats.get(&date) {
                let total = stats.total();
                let correct = stats.correct;

                // Determine color intensity based on correct answers
                let (symbol, style) = match (total, correct) {
                    (0, _) => ("□", Style::default().fg(Color::DarkGray)),
                    (_, c) if c == 0 => ("■", Style::default().fg(Color::Red)),
                    (t, c) if c == t => {
                        // All correct - varying shades of green
                        if t >= 5 {
                            ("■", Style::default().fg(Color::Green).bold())
                        } else if t >= 3 {
                            ("■", Style::default().fg(Color::Green))
                        } else {
                            ("■", Style::default().fg(Color::LightGreen))
                        }
                    }
                    (t, c) => {
                        // Mixed results
                        let ratio = c as f64 / t as f64;
                        if ratio >= 0.7 {
                            ("■", Style::default().fg(Color::LightGreen))
                        } else if ratio >= 0.4 {
                            ("■", Style::default().fg(Color::Yellow))
                        } else {
                            ("■", Style::default().fg(Color::Red))
                        }
                    }
                };

                line_spans.push(Span::styled(format!(" {} ", symbol), style));
            } else {
                line_spans.push(Span::raw(" □ "));
            }
        }

        lines.push(Line::from(line_spans));
    }

    // Legend
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::raw("凡例: "),
        Span::styled("□", Style::default().fg(Color::DarkGray)),
        Span::raw(" なし  "),
        Span::styled("■", Style::default().fg(Color::Red)),
        Span::raw(" 全不正解  "),
        Span::styled("■", Style::default().fg(Color::Yellow)),
        Span::raw(" 混在  "),
        Span::styled("■", Style::default().fg(Color::LightGreen)),
        Span::raw(" 良  "),
        Span::styled("■", Style::default().fg(Color::Green)),
        Span::raw(" 優  "),
        Span::styled("■", Style::default().fg(Color::Green).bold()),
        Span::raw(" 秀"),
    ]));

    Text::from(lines)
}

fn create_bar_chart(weekly_stats: &[WeeklyStats], _width: usize, height: usize) -> Text<'static> {
    let mut lines = Vec::new();

    // Title
    lines.push(Line::from(vec![
        Span::styled("過去4週間の成績", Style::default().bold()),
    ]));
    lines.push(Line::from(""));

    // Find max value for scaling
    let max_value = weekly_stats
        .iter()
        .map(|s| s.correct.max(s.incorrect))
        .max()
        .unwrap_or(1);

    let chart_height = (height.saturating_sub(6)).max(8);

    // Display each week
    for stats in weekly_stats {
        let correct_bars = if max_value > 0 {
            (stats.correct as f64 / max_value as f64 * chart_height as f64) as usize
        } else {
            0
        };

        let incorrect_bars = if max_value > 0 {
            (stats.incorrect as f64 / max_value as f64 * chart_height as f64) as usize
        } else {
            0
        };

        let mut line_spans = vec![
            Span::raw(format!("第{}週: ", stats.week_number)),
        ];

        // Correct bar (green)
        line_spans.push(Span::styled(
            "█".repeat(correct_bars),
            Style::default().fg(Color::Green),
        ));
        line_spans.push(Span::raw(format!(" {}", stats.correct)));

        lines.push(Line::from(line_spans));

        // Incorrect bar (red)
        let mut incorrect_line = vec![
            Span::raw("       "),
        ];
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
