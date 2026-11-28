use crate::models::{
    DailyStats, EvaluationScoreStats, EvaluationSummary, RecordedOutcome, TrainingResult,
    WeeklyStats, WeightedOutcome,
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
            stats.record(result.outcome());
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

        WeeklyStats {
            week_number: week + 1,
            counts: count_results_in_range(results, week_start, week_end),
        }
    })
}

pub fn get_recent_evaluation_summaries(
    results: &[TrainingResult],
    days: usize,
    today: Date,
    zone: &TimeZone,
) -> (
    EvaluationSummary<3>,
    EvaluationSummary<6>,
    EvaluationSummary<4>,
) {
    let start_date =
        today - Span::new().days(i64::try_from(days.saturating_sub(1)).unwrap_or(i64::MAX));

    let mut legacy_scores = std::array::from_fn(|_| Vec::new());
    let mut summary_scores = std::array::from_fn(|_| Vec::new());
    let mut reading_scores = std::array::from_fn(|_| Vec::new());
    let mut legacy_count = 0;
    let mut summary_count = 0;
    let mut reading_count = 0;

    for result in results {
        if days == 0 || !(start_date..=today).contains(&zone.to_datetime(result.timestamp).date()) {
            continue;
        }
        match &result.outcome {
            RecordedOutcome::Legacy(legacy) => {
                if let Some(evaluation) = &legacy.evaluation {
                    let scores = [
                        evaluation.importance,
                        evaluation.conciseness,
                        evaluation.accuracy,
                    ];
                    for (column, score) in legacy_scores.iter_mut().zip(scores) {
                        column.push(f64::from(score));
                    }
                    legacy_count += 1;
                }
            }
            RecordedOutcome::Weighted(WeightedOutcome::V2 { evaluation }) => {
                let scores = &evaluation.scores;
                let values = [
                    scores.faithfulness,
                    scores.coverage,
                    scores.relevance,
                    scores.conciseness,
                    scores.clarity,
                    f64::from(evaluation.total_score),
                ];
                for (column, score) in summary_scores.iter_mut().zip(values) {
                    column.push(score);
                }
                summary_count += 1;
            }
            RecordedOutcome::Weighted(WeightedOutcome::V3 { evaluation }) => {
                let values = evaluation
                    .scores
                    .scores()
                    .into_iter()
                    .chain([f64::from(evaluation.total_score)]);
                for (column, score) in reading_scores.iter_mut().zip(values) {
                    column.push(score);
                }
                reading_count += 1;
            }
        }
    }

    (
        EvaluationSummary {
            count: legacy_count,
            scores: legacy_scores.map(calculate_score_stats),
        },
        EvaluationSummary {
            count: summary_count,
            scores: summary_scores.map(calculate_score_stats),
        },
        EvaluationSummary {
            count: reading_count,
            scores: reading_scores.map(calculate_score_stats),
        },
    )
}

#[expect(
    clippy::cast_precision_loss,
    reason = "f64 represents counts exactly up to 2^53"
)]
pub fn calculate_score_stats(mut scores: Vec<f64>) -> Option<EvaluationScoreStats> {
    if scores.is_empty() {
        return None;
    }

    let sum: f64 = scores.iter().sum();
    let average = sum / scores.len() as f64;
    let median = calculate_median(&mut scores);

    Some(EvaluationScoreStats { average, median })
}

pub fn calculate_median(scores: &mut [f64]) -> f64 {
    scores.sort_unstable_by(f64::total_cmp);
    let mid = scores.len() / 2;
    match (
        scores.get(mid),
        mid.checked_sub(1).and_then(|left| scores.get(left)),
    ) {
        (Some(&right), Some(&left)) if scores.len().is_multiple_of(2) => f64::midpoint(left, right),
        (Some(&middle), _) => middle,
        _ => 0.0,
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
) -> DailyStats {
    let mut counts = DailyStats::default();
    for result in results {
        if result.timestamp >= start && result.timestamp < end {
            counts.record(result.outcome());
        }
    }
    counts
}
