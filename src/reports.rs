use crate::evaluation::SCORE_LABELS;
use crate::models::{DailyStats, EvaluationSummary, WeeklyStats};
use crate::stats::{TrainingStats, required_exp_for_level};
use crate::stats_analysis::get_recent_evaluation_summaries;
use jiff::{Span as DateSpan, Timestamp, civil::Date, tz::TimeZone};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};
use std::collections::HashMap;

const REPORT_DAYS: usize = 180;
const WEEKS_TO_SHOW: usize = 4;
const MAX_BADGES_DISPLAY: usize = 20;
const HEATMAP_CELL: &str = "■";
const HEATMAP_REVIEW_CELL: &str = "◆";
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
    let frame = (Timestamp::now().as_millisecond() / 500) % 2;

    let art = match (level, frame) {
        (1, 0) => BUDDY_LEVEL_1_A,
        (1, _) => BUDDY_LEVEL_1_B,
        (2, 0) => BUDDY_LEVEL_2_A,
        (2, _) => BUDDY_LEVEL_2_B,
        (_, 0) => BUDDY_LEVEL_3_A,
        (_, _) => BUDDY_LEVEL_3_B,
    };
    art.trim_prefix('\n')
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

fn append_score_summary<const N: usize>(
    lines: &mut Vec<Line<'static>>,
    title: &str,
    labels: [&str; N],
    summary: EvaluationSummary<N>,
    width: usize,
) {
    if summary.count == 0 {
        return;
    }
    lines.push(Line::from(Span::styled(
        format!("{title}・{}件（平均/中央値）", summary.count),
        Style::default().fg(Color::Cyan).bold(),
    )));
    let mut row = Line::default();
    for (label, score) in labels
        .into_iter()
        .zip(summary.scores)
        .filter_map(|(label, score)| score.map(|score| (label, score)))
    {
        let item = Span::raw(format!("{label} {:.1}/{:.1}", score.average, score.median));
        if !row.spans.is_empty() {
            if row.width() + 2 + item.width() <= width {
                row.spans.push(Span::raw("  "));
            } else {
                lines.push(std::mem::take(&mut row));
            }
        }
        row.spans.push(item);
    }
    if !row.spans.is_empty() {
        lines.push(row);
    }
}

fn render_evaluation_summary(
    stats: &TrainingStats,
    today: Date,
    zone: &TimeZone,
    width: usize,
) -> Vec<Line<'static>> {
    let (legacy, weighted) =
        get_recent_evaluation_summaries(&stats.results, REPORT_DAYS, today, zone);
    let [faithfulness, coverage, relevance, conciseness, clarity] = SCORE_LABELS;
    let mut lines = Vec::new();
    append_score_summary(
        &mut lines,
        "新 v2 0〜4点",
        [
            faithfulness,
            coverage,
            relevance,
            conciseness,
            clarity,
            "総合/100",
        ],
        weighted,
        width,
    );
    append_score_summary(
        &mut lines,
        "旧 v1 1〜5点",
        ["重要情報", "簡潔性", "正確性"],
        legacy,
        width,
    );
    if lines.is_empty() {
        lines.push(Line::from("評価スコアなし"));
    }
    lines
}

pub fn render_unified_report(frame: &mut Frame, area: Rect, stats: &TrainingStats) {
    let now = Timestamp::now();
    let zone = TimeZone::system();
    let today = zone.to_datetime(now).date();
    let block = Block::default().title("レポート (r: 閉じる)");

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

    let badge_block = Block::default().title("バッジ").borders(Borders::ALL);
    let badge_inner = badge_block.inner(*badge_area);
    frame.render_widget(badge_block, *badge_area);
    let badge_content = Text::from(render_badge_section(stats));
    let badge_paragraph = Paragraph::new(badge_content);
    frame.render_widget(badge_paragraph, badge_inner);

    let buddy_block = Block::default()
        .title(format!("バディ (レベル {})", stats.buddy.level))
        .borders(Borders::ALL);
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

    let daily_stats = stats.get_daily_stats(REPORT_DAYS, today, &zone);
    let monthly_block = Block::default()
        .title("180日 (過去180日)")
        .borders(Borders::ALL);
    let monthly_inner = monthly_block.inner(*monthly_area);
    frame.render_widget(monthly_block, *monthly_area);
    let summary = render_evaluation_summary(stats, today, &zone, usize::from(monthly_inner.width));
    let heatmap = create_heatmap_for_date(&daily_stats, today);
    let summary_height = u16::try_from(summary.len()).unwrap_or(u16::MAX);
    let monthly_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(summary_height), Constraint::Min(0)])
        .split(monthly_inner);
    let [summary_area, heatmap_area] = monthly_layout.as_ref() else {
        return;
    };
    frame.render_widget(Paragraph::new(summary), *summary_area);
    frame.render_widget(Paragraph::new(heatmap), *heatmap_area);

    let weekly_stats = stats.get_weekly_stats(WEEKS_TO_SHOW, now);
    let weekly_block = Block::default()
        .title("週次 (過去4週)")
        .borders(Borders::ALL);
    let weekly_inner = weekly_block.inner(*weekly_area);
    frame.render_widget(weekly_block, *weekly_area);
    let chart = create_bar_chart_without_badges(&weekly_stats, usize::from(weekly_inner.width));
    let paragraph = Paragraph::new(chart);
    frame.render_widget(paragraph, weekly_inner);
}

fn create_heatmap_for_date(daily_stats: &HashMap<Date, DailyStats>, today: Date) -> Text<'static> {
    let mut lines = Vec::new();

    let start_offset = i64::try_from(REPORT_DAYS.saturating_sub(1)).unwrap_or(i64::MAX);
    let start_date = today - DateSpan::new().days(start_offset);
    let leading_days = start_date.weekday().to_sunday_zero_offset();
    let grid_start = start_date - DateSpan::new().days(leading_days);
    let week_count = (REPORT_DAYS + usize::from(leading_days.unsigned_abs())).div_ceil(7);

    let week_starts: Vec<Date> = (0..week_count)
        .map(|week| {
            let day_offset = i64::try_from(week.saturating_mul(7)).unwrap_or(i64::MAX);
            grid_start + DateSpan::new().days(day_offset)
        })
        .collect();

    let weekdays = ["日", "月", "火", "水", "木", "金", "土"];

    for (weekday_index, weekday_label) in (0..).zip(weekdays) {
        let mut line_spans = Vec::new();
        line_spans.push(Span::raw(format!("{weekday_label}{HEATMAP_LABEL_SUFFIX}")));

        for week_start in &week_starts {
            let date = *week_start + DateSpan::new().days(weekday_index);
            if date < start_date || date > today {
                line_spans.push(Span::raw(HEATMAP_EMPTY_CELL));
                continue;
            }

            if let Some(stats) = daily_stats.get(&date) {
                let (symbol, style) = get_heatmap_cell_style(stats);
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

    lines.push(Line::from(vec![
        Span::styled(HEATMAP_CELL, Style::default().fg(Color::DarkGray)),
        Span::raw(" なし  "),
        Span::styled(HEATMAP_CELL, Style::default().fg(Color::Red)),
        Span::raw(" 全不合格  "),
        Span::styled(HEATMAP_CELL, Style::default().fg(Color::Yellow)),
        Span::raw(" 混在"),
    ]));
    lines.push(Line::from(vec![
        Span::styled(HEATMAP_CELL, Style::default().fg(Color::LightGreen)),
        Span::raw(" 良  "),
        Span::styled(HEATMAP_CELL, Style::default().fg(Color::Green)),
        Span::raw(" 優  "),
        Span::styled(
            HEATMAP_CELL,
            Style::default().fg(Color::Rgb(0, 255, 0)).bold(),
        ),
        Span::raw(" 全合格  "),
        Span::raw(format!("{HEATMAP_REVIEW_CELL} 要確認")),
    ]));

    Text::from(lines)
}

fn create_bar_chart_without_badges(weekly_stats: &[WeeklyStats], width: usize) -> Text<'static> {
    let mut lines = Vec::new();

    let max_value = weekly_stats
        .iter()
        .map(|week| {
            week.counts
                .correct
                .max(week.counts.incorrect)
                .max(week.counts.review)
        })
        .max()
        .unwrap_or(1);
    let max_bar_width = width.saturating_sub(19);
    for week in weekly_stats {
        let rows = [
            (week.counts.correct, "合格", Color::Green),
            (week.counts.incorrect, "不合格", Color::Red),
            (week.counts.review, "要確認", Color::Yellow),
        ];
        let week_label = format!("第{}週 ", week.week_number);
        for (index, (count, label, color)) in rows.into_iter().enumerate() {
            let prefix = if index == 0 { &week_label } else { "       " };
            let bars = calculate_bar_height(count, max_value, max_bar_width);
            lines.push(Line::from(vec![
                Span::raw(format!("{prefix}{label} {count} ")),
                Span::styled("█".repeat(bars), Style::default().fg(color)),
            ]));
        }
    }
    Text::from(lines)
}

fn get_heatmap_cell_style(stats: &DailyStats) -> (&'static str, Style) {
    let symbol = if stats.review > 0 {
        HEATMAP_REVIEW_CELL
    } else {
        HEATMAP_CELL
    };
    let correct = stats.correct;
    let total = correct + stats.incorrect;
    if total == 0 {
        let color = if stats.review > 0 {
            Color::Yellow
        } else {
            Color::DarkGray
        };
        return (symbol, Style::default().fg(color));
    }

    if correct == 0 {
        return (symbol, Style::default().fg(Color::Red));
    }

    if correct == total {
        return (symbol, Style::default().fg(Color::Rgb(0, 255, 0)).bold());
    }

    let color = if correct.saturating_mul(10) >= total.saturating_mul(8) {
        Color::Green
    } else if correct.saturating_mul(10) >= total.saturating_mul(5) {
        Color::LightGreen
    } else {
        Color::Yellow
    };

    (symbol, Style::default().fg(color))
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
    use ratatui::{backend::TestBackend, buffer::Cell};

    fn date(year: i16, month: i8, day: i8) -> Result<Date, String> {
        Date::new(year, month, day).map_err(|error| error.to_string())
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
    #[expect(
        clippy::panic_in_result_fn,
        reason = "Test assertions report the failed invariant"
    )]
    fn report_displays_both_versions_and_review_on_an_80_by_22_terminal()
    -> Result<(), Box<dyn std::error::Error>> {
        let now = Timestamp::now() - jiff::SignedDuration::from_hours(12);
        let mut stats: TrainingStats = serde_json::from_value(serde_json::json!({
            "results": [{
                "timestamp": now,
                "passed": true,
                "evaluation": {
                    "appropriate": true, "importance": 4, "conciseness": 3, "accuracy": 5,
                    "improvement1": "a", "improvement2": "b", "improvement3": "c",
                    "overall_passed": true
                }
            }]
        }))?;
        let mut scores =
            crate::evaluation::parse_jev_response(&crate::evaluation::tests::response())
                .map_err(|error| std::io::Error::other(format!("{error:?}")))?;
        scores.critical_error = 0.5;
        stats.add_evaluation(crate::models::EvaluationScores {
            total_score: scores.total_score(),
            scores,
            judgment: crate::evaluation::Judgment::Review,
            improvements: ["a".into(), "b".into(), "c".into()],
        });
        let mut terminal = Terminal::new(TestBackend::new(80, 22))?;
        terminal.draw(|frame| render_unified_report(frame, frame.area(), &stats))?;
        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .chunks(80)
            .map(|row| {
                row.iter()
                    .map(Cell::symbol)
                    .collect::<String>()
                    .replace(' ', "")
                    + "\n"
            })
            .collect();
        for expected in [
            "新v20〜4点",
            "旧v11〜5点",
            "忠実性3.5/3.5",
            "簡潔性3.0/3.0",
            "総合/10095.0/95.0",
            "◆",
            "要確認1",
            "第4週",
        ] {
            assert!(
                rendered.contains(expected),
                "missing {expected}\n{rendered}"
            );
        }
        assert_eq!(rendered.matches("要確認0").count(), 3);
        Ok(())
    }

    #[test]
    fn heatmap_uses_weekdays_as_rows_from_sunday_to_saturday() -> Result<(), String> {
        let today = date(2026, 7, 2)?;
        let sunday = date(2026, 6, 28)?;
        let daily_stats = HashMap::from([
            (
                sunday,
                DailyStats {
                    correct: 0,
                    incorrect: 1,
                    review: 0,
                },
            ),
            (
                today,
                DailyStats {
                    correct: 1,
                    incorrect: 0,
                    review: 0,
                },
            ),
        ]);
        let heatmap = create_heatmap_for_date(&daily_stats, today);
        let last_cell_color = |row: usize| {
            heatmap
                .lines
                .get(row)
                .and_then(|line| line.spans.last())
                .and_then(|span| span.style.fg)
        };
        if last_cell_color(0) != Some(Color::Red)
            || last_cell_color(4) != Some(Color::Rgb(0, 255, 0))
        {
            return Err("Sunday and Thursday data should stay on their corresponding rows".into());
        }
        let lines = text_content(heatmap);

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

        if weekday_labels != "日月火水木金土" {
            return Err(format!("unexpected weekday labels: {weekday_labels}"));
        }
        Ok(())
    }

    #[test]
    fn heatmap_distinguishes_review_only_days_from_failures_and_empty_days() {
        let (symbol, style) = get_heatmap_cell_style(&DailyStats {
            review: 1,
            ..DailyStats::default()
        });
        assert_eq!(symbol, HEATMAP_REVIEW_CELL);
        assert_eq!(style.fg, Some(Color::Yellow));
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
                review: 0,
            },
        );

        let rendered = text_content(create_heatmap_for_date(&daily_stats, today)).join("\n");

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
        let lines = text_content(create_heatmap_for_date(&HashMap::new(), today));
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
                .get("日 ".len()..)
                .ok_or_else(|| format!("row was too short: {row}"))?;
            let cell_count = cells.chars().count();
            let expected_cell_count = 26;
            if cell_count != expected_cell_count {
                return Err(format!(
                    "expected {expected_cell_count} week columns, got {cell_count}"
                ));
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
        let lines = text_content(create_heatmap_for_date(&HashMap::new(), today));
        let saturday_row = lines
            .get(6)
            .ok_or_else(|| "heatmap did not render saturday row".to_string())?;
        let friday_row = lines
            .get(5)
            .ok_or_else(|| "heatmap did not render friday row".to_string())?;

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
        Ok(())
    }
}
