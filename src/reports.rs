use crate::models::{DailyStats, WeeklyStats};
use crate::stats::{TrainingStats, required_exp_for_level};
use chrono::{Datelike, Local, NaiveDate};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};
use std::collections::HashMap;

const DAYS_IN_MONTH: usize = 30;
const WEEKS_TO_SHOW: usize = 4;
const MAX_BADGES_DISPLAY: usize = 20;
const HEATMAP_CELL: &str = "■";
const HEATMAP_EMPTY_CELL: &str = "·";
const HEATMAP_LABEL_SUFFIX: &str = " ";

const BUDDY_LEVEL_1_A: &str = r"
          ╱|、
        (˚ˎ。7
         |、˜〵〜";

const BUDDY_LEVEL_1_B: &str = r"
          ╱|、
        (˚ˎ< 7
         |、˜〵∫";

const BUDDY_LEVEL_2_A: &str = r"
         ヘ_ヘ
        ミ. . ミ
         |、 〵〜";

const BUDDY_LEVEL_2_B: &str = r"
         ヘ_ヘ
        ミ> . ミ
         |、 〵∫";

const BUDDY_LEVEL_3_A: &str = r"
         ヘ_ヘ
        ミ. . ミ
         (    )〜";

const BUDDY_LEVEL_3_B: &str = r"
         ヘ_ヘ    ✨
        ミ> < ミ
         (    )∫";

fn get_buddy_ascii(level: u32) -> &'static str {
    let frame = (Local::now().timestamp_millis() / 500) % 2;

    let art = match (level, frame) {
        (1, 0) => BUDDY_LEVEL_1_A,
        (1, _) => BUDDY_LEVEL_1_B,
        (2, 0) => BUDDY_LEVEL_2_A,
        (2, _) => BUDDY_LEVEL_2_B,
        (_, 0) => BUDDY_LEVEL_3_A,
        (_, _) => BUDDY_LEVEL_3_B,
    };
    art.strip_prefix('\n').unwrap_or(art)
}

fn render_badge_section(stats: &TrainingStats) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let (consecutive_badges, cumulative_badges) = stats.get_badges_by_type();

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

    let Some(importance) = summary.importance.as_ref() else {
        return lines;
    };
    let Some(conciseness) = summary.conciseness.as_ref() else {
        return lines;
    };
    let Some(accuracy) = summary.accuracy.as_ref() else {
        return lines;
    };

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

    let vertical_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(6), Constraint::Min(0)])
        .split(inner);
    let [top_area, bottom_area] = vertical_layout.as_ref() else {
        return;
    };

    let top_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(*top_area);
    let [badge_area, buddy_area] = top_layout.as_ref() else {
        return;
    };

    let badge_block = Block::default()
        .title("バッジ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let badge_inner = badge_block.inner(*badge_area);
    frame.render_widget(badge_block, *badge_area);
    let badge_content = Text::from(render_badge_section(stats));
    let badge_paragraph = Paragraph::new(badge_content);
    frame.render_widget(badge_paragraph, badge_inner);

    let buddy_block = Block::default()
        .title(format!("バディ (レベル {})", stats.buddy.level))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::LightBlue));
    let buddy_inner = buddy_block.inner(*buddy_area);
    frame.render_widget(buddy_block, *buddy_area);

    let buddy_ascii = get_buddy_ascii(stats.buddy.level);
    let required_exp = required_exp_for_level(stats.buddy.level);
    let buddy_text = format!(
        "{}\n        経験値: {}/{}",
        buddy_ascii, stats.buddy.exp, required_exp
    );
    let buddy_paragraph = Paragraph::new(buddy_text);
    frame.render_widget(buddy_paragraph, buddy_inner);

    let horizontal_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(*bottom_area);
    let [monthly_area, weekly_area] = horizontal_layout.as_ref() else {
        return;
    };

    let daily_stats = stats.get_daily_stats(DAYS_IN_MONTH);
    let monthly_block = Block::default()
        .title("月次 (過去30日)")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));
    let monthly_inner = monthly_block.inner(*monthly_area);
    frame.render_widget(monthly_block, *monthly_area);
    if monthly_inner.height >= 6 {
        let monthly_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(6), Constraint::Min(0)])
            .split(monthly_inner);
        let [summary_area, heatmap_area] = monthly_layout.as_ref() else {
            return;
        };
        let summary_text = Text::from(render_evaluation_summary(stats));
        let summary_paragraph = Paragraph::new(summary_text);
        frame.render_widget(summary_paragraph, *summary_area);

        let heatmap = create_heatmap_without_badges(
            &daily_stats,
            usize::from(heatmap_area.width),
            usize::from(heatmap_area.height),
        );
        let paragraph = Paragraph::new(heatmap);
        frame.render_widget(paragraph, *heatmap_area);
    } else {
        let heatmap = create_heatmap_without_badges(
            &daily_stats,
            usize::from(monthly_inner.width),
            usize::from(monthly_inner.height),
        );
        let paragraph = Paragraph::new(heatmap);
        frame.render_widget(paragraph, monthly_inner);
    }

    let weekly_stats = stats.get_weekly_stats(WEEKS_TO_SHOW);
    let weekly_block = Block::default()
        .title("週次 (過去4週)")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));
    let weekly_inner = weekly_block.inner(*weekly_area);
    frame.render_widget(weekly_block, *weekly_area);
    let chart = create_bar_chart_without_badges(
        &weekly_stats,
        usize::from(weekly_inner.width),
        usize::from(weekly_inner.height),
    );
    let paragraph = Paragraph::new(chart);
    frame.render_widget(paragraph, weekly_inner);
}

fn create_heatmap_without_badges(
    daily_stats: &HashMap<NaiveDate, DailyStats>,
    width: usize,
    height: usize,
) -> Text<'static> {
    create_heatmap_for_date(daily_stats, width, height, Local::now().date_naive())
}

fn create_heatmap_for_date(
    daily_stats: &HashMap<NaiveDate, DailyStats>,
    _width: usize,
    _height: usize,
    today: NaiveDate,
) -> Text<'static> {
    let mut lines = Vec::new();

    let start_offset = i64::try_from(DAYS_IN_MONTH.saturating_sub(1)).unwrap_or(i64::MAX);
    let start_date = today - chrono::Duration::days(start_offset);

    let grid_start =
        start_date - chrono::Duration::days(i64::from(start_date.weekday().num_days_from_sunday()));
    let days_in_grid = (today - grid_start).num_days() + 1;
    let week_count = usize::try_from(days_in_grid)
        .unwrap_or(DAYS_IN_MONTH)
        .div_ceil(7);

    let week_starts: Vec<NaiveDate> = (0..week_count)
        .map(|week| {
            let day_offset = i64::try_from(week.saturating_mul(7)).unwrap_or(i64::MAX);
            grid_start + chrono::Duration::days(day_offset)
        })
        .collect();

    let weekdays = [
        ("土", 6_u32),
        ("金", 5_u32),
        ("木", 4_u32),
        ("水", 3_u32),
        ("火", 2_u32),
        ("月", 1_u32),
        ("日", 0_u32),
    ];

    for (weekday_label, weekday_index) in weekdays {
        let mut line_spans = Vec::new();
        line_spans.push(Span::raw(format!("{weekday_label}{HEATMAP_LABEL_SUFFIX}")));

        for week_start in &week_starts {
            let date = *week_start + chrono::Duration::days(i64::from(weekday_index));
            if date < start_date || date > today {
                line_spans.push(Span::raw(HEATMAP_EMPTY_CELL));
                continue;
            }

            if let Some(stats) = daily_stats.get(&date) {
                let total = stats.total();
                let correct = stats.correct;

                let (symbol, style) = get_heatmap_cell_style(total, correct);

                line_spans.push(Span::styled(symbol, style));
            } else {
                line_spans.push(Span::styled(
                    HEATMAP_CELL,
                    Style::default().fg(Color::DarkGray),
                ));
            }
        }

        lines.push(Line::from(line_spans));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::raw("凡例: "),
        Span::styled(HEATMAP_CELL, Style::default().fg(Color::DarkGray)),
        Span::raw(" なし  "),
        Span::styled(HEATMAP_CELL, Style::default().fg(Color::Red)),
        Span::raw(" 全不正解  "),
        Span::styled(HEATMAP_CELL, Style::default().fg(Color::Yellow)),
        Span::raw(" 混在  "),
        Span::styled(HEATMAP_CELL, Style::default().fg(Color::LightGreen)),
        Span::raw(" 良  "),
        Span::styled(HEATMAP_CELL, Style::default().fg(Color::Green)),
        Span::raw(" 優  "),
        Span::styled(
            HEATMAP_CELL,
            Style::default().fg(Color::Rgb(0, 255, 0)).bold(),
        ),
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

    let max_value = weekly_stats
        .iter()
        .map(|s| s.correct.max(s.incorrect))
        .max()
        .unwrap_or(1);

    let max_bar_width = width.saturating_sub(15).max(10);

    for stats in weekly_stats {
        let correct_bars = calculate_bar_height(stats.correct, max_value, max_bar_width);
        let incorrect_bars = calculate_bar_height(stats.incorrect, max_value, max_bar_width);

        let mut line_spans = vec![Span::raw(format!("第{}週: ", stats.week_number))];

        line_spans.push(Span::styled(
            "█".repeat(correct_bars),
            Style::default().fg(Color::Green),
        ));
        line_spans.push(Span::raw(format!(" {}", stats.correct)));

        lines.push(Line::from(line_spans));

        let mut incorrect_line = vec![Span::raw("       ")];
        incorrect_line.push(Span::styled(
            "█".repeat(incorrect_bars),
            Style::default().fg(Color::Red),
        ));
        incorrect_line.push(Span::raw(format!(" {}", stats.incorrect)));

        lines.push(Line::from(incorrect_line));
        lines.push(Line::from(""));
    }

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
        return (HEATMAP_CELL, Style::default().fg(Color::DarkGray));
    }

    if correct == 0 {
        return (HEATMAP_CELL, Style::default().fg(Color::Red));
    }

    if correct == total {
        return (
            HEATMAP_CELL,
            Style::default().fg(Color::Rgb(0, 255, 0)).bold(),
        );
    }

    let color = if correct.saturating_mul(10) >= total.saturating_mul(8) {
        Color::Green
    } else if correct.saturating_mul(10) >= total.saturating_mul(5) {
        Color::LightGreen
    } else {
        Color::Yellow
    };

    (HEATMAP_CELL, Style::default().fg(color))
}

fn calculate_bar_height(value: usize, max_value: usize, max_len: usize) -> usize {
    if max_value == 0 {
        return 0;
    }
    value.saturating_mul(max_len) / max_value
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: i32, month: u32, day: u32) -> Result<NaiveDate, String> {
        NaiveDate::from_ymd_opt(year, month, day)
            .ok_or_else(|| format!("invalid test date: {year}-{month}-{day}"))
    }

    fn text_content(text: Text<'static>) -> Vec<String> {
        text.lines
            .into_iter()
            .map(|line| {
                line.spans
                    .into_iter()
                    .map(|span| span.content.into_owned())
                    .collect::<String>()
            })
            .collect()
    }

    #[test]
    fn heatmap_uses_weekdays_as_rows_from_saturday_to_sunday() -> Result<(), String> {
        let today = date(2026, 7, 2)?;
        let lines = text_content(create_heatmap_for_date(&HashMap::new(), 80, 12, today));

        let weekday_rows = lines
            .get(0..7)
            .ok_or_else(|| "heatmap did not render all weekday rows".to_string())?;
        let weekday_labels = weekday_rows
            .iter()
            .map(|line| {
                line.chars()
                    .next()
                    .ok_or_else(|| "weekday row was empty".to_string())
            })
            .collect::<Result<String, String>>()?;

        if weekday_labels != "土金木水火月日" {
            return Err(format!("unexpected weekday labels: {weekday_labels}"));
        }
        Ok(())
    }

    #[test]
    fn heatmap_uses_unicode_blocks_without_ascii_cell_fallbacks() -> Result<(), String> {
        let today = date(2026, 7, 2)?;
        let mut daily_stats = HashMap::new();
        daily_stats.insert(
            today,
            DailyStats {
                correct: 1,
                incorrect: 0,
            },
        );

        let rendered =
            text_content(create_heatmap_for_date(&daily_stats, 80, 12, today)).join("\n");

        if !rendered.contains(HEATMAP_CELL) {
            return Err("heatmap did not contain unicode block cells".to_string());
        }
        if rendered.contains("##") {
            return Err("heatmap contained deprecated ## cells".to_string());
        }
        if rendered.contains("--") {
            return Err("heatmap contained deprecated -- cells".to_string());
        }
        Ok(())
    }

    #[test]
    fn heatmap_uses_compact_week_columns_without_header() -> Result<(), String> {
        let today = date(2026, 7, 2)?;
        let lines = text_content(create_heatmap_for_date(&HashMap::new(), 80, 12, today));
        let first_line = lines
            .first()
            .ok_or_else(|| "heatmap did not render any rows".to_string())?;

        if first_line.starts_with(' ') || first_line.contains("06/03") {
            return Err(format!(
                "heatmap rendered an unexpected header: {first_line}"
            ));
        }

        let weekday_rows = lines
            .get(0..7)
            .ok_or_else(|| "heatmap did not render all weekday rows".to_string())?;
        for row in weekday_rows {
            let cells = row
                .get("土 ".len()..)
                .ok_or_else(|| format!("row was too short: {row}"))?;
            let cell_count = cells.chars().count();
            if cell_count != 5 {
                return Err(format!("expected 5 week columns, got {cell_count}"));
            }
            if cells.contains(' ') {
                return Err(format!("heatmap cells were not compact: {row}"));
            }
        }
        Ok(())
    }

    #[test]
    fn heatmap_marks_out_of_range_cells_as_empty() -> Result<(), String> {
        let today = date(2026, 7, 2)?;
        let lines = text_content(create_heatmap_for_date(&HashMap::new(), 80, 12, today));
        let saturday_row = lines
            .first()
            .ok_or_else(|| "heatmap did not render saturday row".to_string())?;
        let friday_row = lines
            .get(1)
            .ok_or_else(|| "heatmap did not render friday row".to_string())?;
        let sunday_row = lines
            .get(6)
            .ok_or_else(|| "heatmap did not render sunday row".to_string())?;

        if !saturday_row.ends_with(HEATMAP_EMPTY_CELL) {
            return Err(format!(
                "last saturday cell should be out of range: {saturday_row}"
            ));
        }
        if !friday_row.ends_with(HEATMAP_EMPTY_CELL) {
            return Err(format!(
                "last friday cell should be out of range: {friday_row}"
            ));
        }
        if !sunday_row.starts_with(&format!("日 {HEATMAP_EMPTY_CELL}")) {
            return Err(format!(
                "first sunday cell should be out of range: {sunday_row}"
            ));
        }
        Ok(())
    }
}
