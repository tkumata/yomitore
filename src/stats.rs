use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Badge award interval (every N correct answers)
const BADGE_INTERVAL: usize = 5;
/// Maximum consecutive streak for badges (10 badges)
const MAX_CONSECUTIVE_STREAK: usize = 50;
/// Maximum cumulative correct answers for badges (20 badges)
const MAX_CUMULATIVE_MILESTONE: usize = 100;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TrainingResult {
    pub timestamp: DateTime<Local>,
    pub passed: bool,
    #[serde(default)]
    pub evaluation: Option<EvaluationScores>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum BadgeType {
    ConsecutiveStreak(usize),   // 連続正解数 (5, 10, 15, ...)
    CumulativeMilestone(usize), // 累積正解数 (5, 10, 15, ...)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Badge {
    pub badge_type: BadgeType,
    pub earned_at: DateTime<Local>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct TrainingStats {
    pub results: Vec<TrainingResult>,
    #[serde(default)]
    pub badges: Vec<Badge>,
    #[serde(default)]
    pub current_streak: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EvaluationScores {
    pub appropriate: bool,
    pub importance: u8,
    pub conciseness: u8,
    pub accuracy: u8,
    pub improvement1: String,
    pub improvement2: String,
    pub improvement3: String,
    pub overall_passed: bool,
}

#[derive(Clone, Debug)]
pub struct EvaluationScoreStats {
    pub average: f32,
    pub median: f32,
}

#[derive(Clone, Debug)]
pub struct EvaluationSummary {
    pub count: usize,
    pub importance: Option<EvaluationScoreStats>,
    pub conciseness: Option<EvaluationScoreStats>,
    pub accuracy: Option<EvaluationScoreStats>,
}

impl TrainingStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let path = Self::get_stats_file_path()?;
        if !path.exists() {
            return Ok(Self::new());
        }
        let content = fs::read_to_string(&path)?;
        let mut stats: TrainingStats = serde_json::from_str(&content)?;

        // Recalculate current streak from results to handle existing data
        stats.recalculate_streak();

        // Rebuild badges from historical data if needed
        stats.rebuild_badges_from_history();

        Ok(stats)
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::get_stats_file_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        fs::write(&path, content)?;
        Ok(())
    }

    pub fn add_result_with_evaluation(
        &mut self,
        passed: bool,
        evaluation: Option<EvaluationScores>,
    ) {
        self.results.push(TrainingResult {
            timestamp: Local::now(),
            passed,
            evaluation,
        });

        // Update streak and award badges
        if passed {
            self.current_streak += 1;

            // Award consecutive streak badge
            if self.current_streak.is_multiple_of(BADGE_INTERVAL)
                && self.current_streak <= MAX_CONSECUTIVE_STREAK
            {
                let badge = Badge {
                    badge_type: BadgeType::ConsecutiveStreak(self.current_streak),
                    earned_at: Local::now(),
                };
                // Only add if we don't already have this badge
                if !self.badges.iter().any(|b| b.badge_type == badge.badge_type) {
                    self.badges.push(badge);
                }
            }

            // Count total correct answers for cumulative milestone
            let total_correct = self.results.iter().filter(|r| r.passed).count();

            // Award cumulative milestone badge
            if total_correct.is_multiple_of(BADGE_INTERVAL)
                && total_correct <= MAX_CUMULATIVE_MILESTONE
            {
                let badge = Badge {
                    badge_type: BadgeType::CumulativeMilestone(total_correct),
                    earned_at: Local::now(),
                };
                // Only add if we don't already have this badge
                if !self.badges.iter().any(|b| b.badge_type == badge.badge_type) {
                    self.badges.push(badge);
                }
            }
        } else {
            // Reset streak on incorrect answer, but keep earned badges
            self.current_streak = 0;
        }
    }

    fn get_stats_file_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
        let config_dir = dirs::config_dir().ok_or("Could not find config directory")?;
        Ok(config_dir.join("yomitore").join("stats.json"))
    }

    /// Recalculate current streak from the end of results
    fn recalculate_streak(&mut self) {
        self.current_streak = 0;
        // Count consecutive correct answers from the end
        for result in self.results.iter().rev() {
            if result.passed {
                self.current_streak += 1;
            } else {
                break;
            }
        }
    }

    /// Rebuild badges from historical data
    fn rebuild_badges_from_history(&mut self) {
        // Track all streak milestones and cumulative milestones reached
        let mut max_streak: usize = 0;
        let mut current_streak: usize = 0;
        let mut total_correct: usize = 0;

        for result in &self.results {
            if result.passed {
                current_streak += 1;
                total_correct += 1;
                max_streak = max_streak.max(current_streak);

                // Award consecutive streak badges
                if current_streak.is_multiple_of(BADGE_INTERVAL)
                    && current_streak <= MAX_CONSECUTIVE_STREAK
                {
                    let badge = Badge {
                        badge_type: BadgeType::ConsecutiveStreak(current_streak),
                        earned_at: result.timestamp,
                    };
                    if !self.badges.iter().any(|b| b.badge_type == badge.badge_type) {
                        self.badges.push(badge);
                    }
                }

                // Award cumulative milestone badges
                if total_correct.is_multiple_of(BADGE_INTERVAL)
                    && total_correct <= MAX_CUMULATIVE_MILESTONE
                {
                    let badge = Badge {
                        badge_type: BadgeType::CumulativeMilestone(total_correct),
                        earned_at: result.timestamp,
                    };
                    if !self.badges.iter().any(|b| b.badge_type == badge.badge_type) {
                        self.badges.push(badge);
                    }
                }
            } else {
                current_streak = 0;
            }
        }
    }

    /// Get daily aggregated stats for the last N days
    pub fn get_daily_stats(&self, days: usize) -> HashMap<NaiveDate, DailyStats> {
        self.calculate_daily_stats(days, Local::now().date_naive())
    }

    /// Internal logic for daily stats aggregation
    fn calculate_daily_stats(
        &self,
        days: usize,
        today: NaiveDate,
    ) -> HashMap<NaiveDate, DailyStats> {
        let mut daily_map: HashMap<NaiveDate, DailyStats> = HashMap::new();

        // Initialize all dates with empty stats
        for i in 0..days {
            let date = today - chrono::Duration::days(i as i64);
            daily_map.insert(date, DailyStats::default());
        }

        // Aggregate results
        for result in &self.results {
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

    /// Get weekly stats for the last N weeks
    pub fn get_weekly_stats(&self, weeks: usize) -> Vec<WeeklyStats> {
        self.calculate_weekly_stats(weeks, Local::now())
    }

    /// Internal logic for weekly stats aggregation
    fn calculate_weekly_stats(&self, weeks: usize, now: DateTime<Local>) -> Vec<WeeklyStats> {
        let mut weekly_stats = Vec::new();

        for week in 0..weeks {
            let week_start = now - chrono::Duration::weeks((weeks - week - 1) as i64);
            let week_end = week_start + chrono::Duration::weeks(1);

            let mut correct = 0;
            let mut incorrect = 0;

            for result in &self.results {
                if result.timestamp >= week_start && result.timestamp < week_end {
                    if result.passed {
                        correct += 1;
                    } else {
                        incorrect += 1;
                    }
                }
            }

            weekly_stats.push(WeeklyStats {
                week_number: week + 1,
                correct,
                incorrect,
            });
        }

        weekly_stats
    }

    /// Get badges grouped by type for display
    pub fn get_badges_by_type(&self) -> (Vec<&Badge>, Vec<&Badge>) {
        let consecutive: Vec<&Badge> = self
            .badges
            .iter()
            .filter(|b| matches!(b.badge_type, BadgeType::ConsecutiveStreak(_)))
            .collect();

        let cumulative: Vec<&Badge> = self
            .badges
            .iter()
            .filter(|b| matches!(b.badge_type, BadgeType::CumulativeMilestone(_)))
            .collect();

        (consecutive, cumulative)
    }

    pub fn get_recent_evaluation_summary(&self, days: usize) -> EvaluationSummary {
        let today = Local::now().date_naive();
        let start_date = today - chrono::Duration::days((days.saturating_sub(1)) as i64);

        let mut importance_scores = Vec::new();
        let mut conciseness_scores = Vec::new();
        let mut accuracy_scores = Vec::new();

        for result in &self.results {
            if result.timestamp.date_naive() < start_date {
                continue;
            }
            if let Some(evaluation) = &result.evaluation {
                importance_scores.push(evaluation.importance);
                conciseness_scores.push(evaluation.conciseness);
                accuracy_scores.push(evaluation.accuracy);
            }
        }

        let count = importance_scores.len();

        EvaluationSummary {
            count,
            importance: calculate_score_stats(&importance_scores),
            conciseness: calculate_score_stats(&conciseness_scores),
            accuracy: calculate_score_stats(&accuracy_scores),
        }
    }
}

impl Badge {
    /// Get the emoji icon for this badge
    pub fn get_icon(&self) -> &str {
        match &self.badge_type {
            BadgeType::ConsecutiveStreak(_) => "🔥",   // Fire for streak
            BadgeType::CumulativeMilestone(_) => "⭐", // Star for milestone
        }
    }

    /// Get the display text for this badge
    pub fn get_display_text(&self) -> String {
        match &self.badge_type {
            BadgeType::ConsecutiveStreak(n) => format!("{}連", n),
            BadgeType::CumulativeMilestone(n) => format!("累積{}", n),
        }
    }
}

#[derive(Default, Clone, Debug)]
pub struct DailyStats {
    pub correct: usize,
    pub incorrect: usize,
}

impl DailyStats {
    pub fn total(&self) -> usize {
        self.correct + self.incorrect
    }
}

#[derive(Clone, Debug)]
pub struct WeeklyStats {
    pub week_number: usize,
    pub correct: usize,
    pub incorrect: usize,
}

fn calculate_score_stats(scores: &[u8]) -> Option<EvaluationScoreStats> {
    if scores.is_empty() {
        return None;
    }

    let sum: u32 = scores.iter().map(|&value| value as u32).sum();
    let average = sum as f32 / scores.len() as f32;
    let median = calculate_median(scores);

    Some(EvaluationScoreStats { average, median })
}

fn calculate_median(scores: &[u8]) -> f32 {
    let mut sorted: Vec<u8> = scores.to_vec();
    sorted.sort_unstable();

    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 1 {
        sorted[mid] as f32
    } else {
        (sorted[mid - 1] as f32 + sorted[mid] as f32) / 2.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_badge_awarding_consecutive() {
        let mut stats = TrainingStats::new();

        // Add 5 consecutive correct answers
        for _ in 0..5 {
            stats.add_result_with_evaluation(true, None);
        }

        // Should have 1 consecutive streak badge (5連) and 1 cumulative badge (累積5)
        let (consecutive, cumulative) = stats.get_badges_by_type();
        assert_eq!(consecutive.len(), 1);
        assert_eq!(cumulative.len(), 1);

        // Add 5 more consecutive correct answers
        for _ in 0..5 {
            stats.add_result_with_evaluation(true, None);
        }

        // Should have 2 consecutive streak badges (5連, 10連) and 2 cumulative badges (累積5, 累積10)
        let (consecutive, cumulative) = stats.get_badges_by_type();
        assert_eq!(consecutive.len(), 2);
        assert_eq!(cumulative.len(), 2);
    }

    #[test]
    fn test_streak_reset_on_incorrect() {
        let mut stats = TrainingStats::new();

        // Add 5 consecutive correct answers
        for _ in 0..5 {
            stats.add_result_with_evaluation(true, None);
        }

        // Current streak should be 5
        assert_eq!(stats.current_streak, 5);

        // Add incorrect answer
        stats.add_result_with_evaluation(false, None);

        // Streak should reset to 0
        assert_eq!(stats.current_streak, 0);

        // But badges should still be there
        let (consecutive, _) = stats.get_badges_by_type();
        assert_eq!(consecutive.len(), 1); // Still have the 5連 badge
    }

    #[test]
    fn test_rebuild_badges_from_history() {
        let mut stats = TrainingStats::new();

        // Simulate existing data
        for _ in 0..10 {
            stats.add_result_with_evaluation(true, None);
        }

        // Clear badges to simulate old data without badges
        stats.badges.clear();
        stats.current_streak = 0;

        // Rebuild from history
        stats.rebuild_badges_from_history();

        // Should have 2 consecutive streak badges and 2 cumulative badges
        let (consecutive, cumulative) = stats.get_badges_by_type();
        assert_eq!(consecutive.len(), 2); // 5連, 10連
        assert_eq!(cumulative.len(), 2); // 累積5, 累積10
    }

    #[test]
    fn test_calculate_daily_stats() {
        let mut stats = TrainingStats::new();
        let today = Local::now().date_naive();

        // Add today's results
        stats.results.push(TrainingResult {
            timestamp: Local::now(),
            passed: true,
            evaluation: None,
        });
        stats.results.push(TrainingResult {
            timestamp: Local::now(),
            passed: false,
            evaluation: None,
        });

        // Add yesterday's results
        let yesterday = Local::now() - chrono::Duration::days(1);
        stats.results.push(TrainingResult {
            timestamp: yesterday,
            passed: true,
            evaluation: None,
        });

        // Testing the internal method directly
        let daily_stats = stats.calculate_daily_stats(7, today);

        // Verify today
        let today_stats = daily_stats.get(&today).unwrap();
        assert_eq!(today_stats.correct, 1);
        assert_eq!(today_stats.incorrect, 1);

        // Verify yesterday
        let yesterday_date = yesterday.date_naive();
        let yesterday_stats = daily_stats.get(&yesterday_date).unwrap();
        assert_eq!(yesterday_stats.correct, 1);
        assert_eq!(yesterday_stats.incorrect, 0);
    }

    #[test]
    fn test_calculate_weekly_stats() {
        let mut stats = TrainingStats::new();
        let now = Local::now();

        // This week (Week 1 in reverse, so index 0)
        stats.results.push(TrainingResult {
            timestamp: now,
            passed: true,
            evaluation: None,
        });

        // Last week
        let last_week = now - chrono::Duration::days(7);
        stats.results.push(TrainingResult {
            timestamp: last_week,
            passed: false,
            evaluation: None,
        });
        stats.results.push(TrainingResult {
            timestamp: last_week,
            passed: false,
            evaluation: None,
        });

        // Testing the internal method directly
        let weekly_stats = stats.calculate_weekly_stats(4, now);

        // Verify this week (last element)
        let this_week_stats = weekly_stats.last().unwrap();
        assert_eq!(this_week_stats.correct, 1);
        assert_eq!(this_week_stats.incorrect, 0);

        // Verify last week (second to last)
        let last_week_stats = &weekly_stats[weekly_stats.len() - 2];
        assert_eq!(last_week_stats.correct, 0);
        assert_eq!(last_week_stats.incorrect, 2);
    }

    #[test]
    fn test_recent_evaluation_summary() {
        let mut stats = TrainingStats::new();
        let now = Local::now();

        stats.results.push(TrainingResult {
            timestamp: now,
            passed: true,
            evaluation: Some(EvaluationScores {
                appropriate: true,
                importance: 5,
                conciseness: 3,
                accuracy: 4,
                improvement1: "なし".to_string(),
                improvement2: "なし".to_string(),
                improvement3: "なし".to_string(),
                overall_passed: true,
            }),
        });
        stats.results.push(TrainingResult {
            timestamp: now,
            passed: false,
            evaluation: Some(EvaluationScores {
                appropriate: false,
                importance: 3,
                conciseness: 2,
                accuracy: 4,
                improvement1: "不足".to_string(),
                improvement2: "冗長".to_string(),
                improvement3: "不正確".to_string(),
                overall_passed: false,
            }),
        });

        let summary = stats.get_recent_evaluation_summary(30);
        assert_eq!(summary.count, 2);
        assert_eq!(summary.importance.as_ref().unwrap().average, 4.0);
        assert_eq!(summary.importance.as_ref().unwrap().median, 4.0);
        assert_eq!(summary.conciseness.as_ref().unwrap().average, 2.5);
        assert_eq!(summary.conciseness.as_ref().unwrap().median, 2.5);
        assert_eq!(summary.accuracy.as_ref().unwrap().average, 4.0);
        assert_eq!(summary.accuracy.as_ref().unwrap().median, 4.0);
    }
}
