use crate::models::{
    DailyStats, EvaluationScoreStats, EvaluationSummary, TrainingResult, WeeklyStats,
};
use jiff::{SignedDuration, Span, Timestamp, civil::Date, tz::TimeZone};
use std::collections::HashMap;

const HOURS_PER_WEEK: i64 = 168;

pub fn calculate_daily_stats(
    results: &[TrainingResult],
    days: usize,
    today: Date,
    zone: &TimeZone,
) -> HashMap<Date, DailyStats> {
    let mut daily_map = initialize_daily_stats(days, today);

    for result in results {
        let date = zone.to_datetime(result.timestamp).date();
        if let Some(stats) = daily_map.get_mut(&date) {
            if result.passed {
                stats.correct += 1;
            } else {
                stats.incorrect += 1;
            }
        }
    }

    daily_map
}

pub fn calculate_weekly_stats(
    results: &[TrainingResult],
    weeks: usize,
    now: Timestamp,
) -> Vec<WeeklyStats> {
    Vec::from_fn(weeks, |week| {
        let offset = i64::try_from(weeks - week - 1).unwrap_or(i64::MAX);
        let week_start = now - SignedDuration::from_hours(offset.saturating_mul(HOURS_PER_WEEK));
        let week_end = week_start + SignedDuration::from_hours(HOURS_PER_WEEK);
        let (correct, incorrect) = count_results_in_range(results, week_start, week_end);

        WeeklyStats {
            week_number: week + 1,
            correct,
            incorrect,
        }
    })
}

pub fn get_recent_evaluation_summary(
    results: &[TrainingResult],
    days: usize,
    today: Date,
    zone: &TimeZone,
) -> EvaluationSummary {
    let start_date =
        today - Span::new().days(i64::try_from(days.saturating_sub(1)).unwrap_or(i64::MAX));

    let mut importance_scores = Vec::new();
    let mut conciseness_scores = Vec::new();
    let mut accuracy_scores = Vec::new();

    for result in results {
        if days == 0 || !(start_date..=today).contains(&zone.to_datetime(result.timestamp).date()) {
            continue;
        }
        if let Some(evaluation) = &result.evaluation {
            importance_scores.push(evaluation.importance);
            conciseness_scores.push(evaluation.conciseness);
            accuracy_scores.push(evaluation.accuracy);
        }
    }

    EvaluationSummary {
        count: importance_scores.len(),
        importance: calculate_score_stats(&importance_scores),
        conciseness: calculate_score_stats(&conciseness_scores),
        accuracy: calculate_score_stats(&accuracy_scores),
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "f64 represents counts exactly up to 2^53"
)]
pub fn calculate_score_stats(scores: &[u8]) -> Option<EvaluationScoreStats> {
    if scores.is_empty() {
        return None;
    }

    let sum: f64 = scores.iter().map(|&value| f64::from(value)).sum();
    let average = sum / scores.len() as f64;
    let median = calculate_median(scores);

    Some(EvaluationScoreStats { average, median })
}

pub fn calculate_median(scores: &[u8]) -> f32 {
    let mut sorted = scores.to_vec();
    sorted.sort_unstable();

    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 1 {
        f32::from(*sorted.get(mid).unwrap_or(&0))
    } else {
        let left = f32::from(*sorted.get(mid - 1).unwrap_or(&0));
        let right = f32::from(*sorted.get(mid).unwrap_or(&0));
        f32::midpoint(left, right)
    }
}

fn initialize_daily_stats(days: usize, today: Date) -> HashMap<Date, DailyStats> {
    let mut daily_map = HashMap::new();
    for i in 0..days {
        let date = today - Span::new().days(i64::try_from(i).unwrap_or(i64::MAX));
        daily_map.insert(date, DailyStats::default());
    }
    daily_map
}

fn count_results_in_range(
    results: &[TrainingResult],
    start: Timestamp,
    end: Timestamp,
) -> (usize, usize) {
    let mut correct = 0;
    let mut incorrect = 0;

    for result in results {
        if result.timestamp >= start && result.timestamp < end {
            if result.passed {
                correct += 1;
            } else {
                incorrect += 1;
            }
        }
    }

    (correct, incorrect)
}
