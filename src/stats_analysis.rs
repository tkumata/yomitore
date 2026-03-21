use crate::models::{
    DailyStats, EvaluationScoreStats, EvaluationSummary, TrainingResult, WeeklyStats,
};
use chrono::{DateTime, Local, NaiveDate};
use std::collections::HashMap;

pub fn calculate_daily_stats(
    results: &[TrainingResult],
    days: usize,
    today: NaiveDate,
) -> HashMap<NaiveDate, DailyStats> {
    let mut daily_map = initialize_daily_stats(days, today);

    for result in results {
        let date = result.timestamp.date_naive();
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
    now: DateTime<Local>,
) -> Vec<WeeklyStats> {
    let mut weekly_stats = Vec::with_capacity(weeks);

    for week in 0..weeks {
        let week_start = now - chrono::Duration::weeks((weeks - week - 1) as i64);
        let week_end = week_start + chrono::Duration::weeks(1);
        let (correct, incorrect) = count_results_in_range(results, week_start, week_end);

        weekly_stats.push(WeeklyStats {
            week_number: week + 1,
            correct,
            incorrect,
        });
    }

    weekly_stats
}

pub fn get_recent_evaluation_summary(results: &[TrainingResult], days: usize) -> EvaluationSummary {
    let today = Local::now().date_naive();
    let start_date = today - chrono::Duration::days((days.saturating_sub(1)) as i64);

    let mut importance_scores = Vec::new();
    let mut conciseness_scores = Vec::new();
    let mut accuracy_scores = Vec::new();

    for result in results {
        if result.timestamp.date_naive() < start_date {
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

pub fn calculate_score_stats(scores: &[u8]) -> Option<EvaluationScoreStats> {
    if scores.is_empty() {
        return None;
    }

    let sum: u32 = scores.iter().map(|&value| value as u32).sum();
    let average = sum as f32 / scores.len() as f32;
    let median = calculate_median(scores);

    Some(EvaluationScoreStats { average, median })
}

pub fn calculate_median(scores: &[u8]) -> f32 {
    let mut sorted = scores.to_vec();
    sorted.sort_unstable();

    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 1 {
        sorted[mid] as f32
    } else {
        (sorted[mid - 1] as f32 + sorted[mid] as f32) / 2.0
    }
}

fn initialize_daily_stats(days: usize, today: NaiveDate) -> HashMap<NaiveDate, DailyStats> {
    let mut daily_map = HashMap::new();
    for i in 0..days {
        let date = today - chrono::Duration::days(i as i64);
        daily_map.insert(date, DailyStats::default());
    }
    daily_map
}

fn count_results_in_range(
    results: &[TrainingResult],
    start: DateTime<Local>,
    end: DateTime<Local>,
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
