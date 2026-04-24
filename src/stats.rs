use crate::models::{
    Badge, BadgeType, Buddy, DailyStats, EvaluationScores, EvaluationSummary, TrainingResult,
    WeeklyStats,
};
use crate::stats_analysis;
use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

const BADGE_INTERVAL: usize = 5;
const MAX_CONSECUTIVE_STREAK: usize = 50;
const MAX_CUMULATIVE_MILESTONE: usize = 100;
const BUDDY_EXP_LEVEL2: u32 = 10;
const BUDDY_EXP_DEFAULT: u32 = 5;
const BUDDY_PENALTY_DAYS: i64 = 3;
const APP_DIR_NAME: &str = "yomitore";
const STATS_FILE_NAME: &str = "stats.json";

pub fn required_exp_for_level(level: u32) -> u32 {
    if level == 2 {
        BUDDY_EXP_LEVEL2
    } else {
        BUDDY_EXP_DEFAULT
    }
}

#[derive(Serialize, Deserialize, Default)]
pub struct TrainingStats {
    pub results: Vec<TrainingResult>,
    #[serde(default)]
    pub badges: Vec<Badge>,
    #[serde(default)]
    pub current_streak: usize,
    #[serde(default, alias = "pet")]
    pub buddy: Buddy,
    #[serde(default)]
    pub last_training_date: Option<DateTime<Local>>,
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

        stats.recalculate_streak();
        stats.check_buddy_penalty();
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

    fn award_badges_for_progress(
        &mut self,
        current_streak: usize,
        total_correct: usize,
        earned_at: DateTime<Local>,
    ) {
        if current_streak.is_multiple_of(BADGE_INTERVAL) && current_streak <= MAX_CONSECUTIVE_STREAK
        {
            let badge = Badge {
                badge_type: BadgeType::ConsecutiveStreak(current_streak),
                earned_at,
            };
            if !self.badges.iter().any(|b| b.badge_type == badge.badge_type) {
                self.badges.push(badge);
            }
        }

        if total_correct.is_multiple_of(BADGE_INTERVAL) && total_correct <= MAX_CUMULATIVE_MILESTONE
        {
            let badge = Badge {
                badge_type: BadgeType::CumulativeMilestone(total_correct),
                earned_at,
            };
            if !self.badges.iter().any(|b| b.badge_type == badge.badge_type) {
                self.badges.push(badge);
            }
        }
    }

    fn add_buddy_exp(&mut self) {
        self.buddy.exp += 1;

        let required_exp = required_exp_for_level(self.buddy.level);

        if self.buddy.exp >= required_exp {
            self.buddy.level += 1;
            self.buddy.exp = 0;
        }
    }

    fn check_buddy_penalty(&mut self) {
        if let Some(last_date) = self.last_training_date {
            let now = Local::now();
            let days_diff = (now - last_date).num_days();

            if days_diff >= BUDDY_PENALTY_DAYS {
                if self.buddy.level > 1 {
                    self.buddy.level -= 1;
                }
                self.buddy.exp = 0;
                self.last_training_date = Some(now);
            }
        }
    }

    pub fn add_result_with_evaluation(
        &mut self,
        passed: bool,
        evaluation: Option<EvaluationScores>,
    ) {
        let now = Local::now();
        self.results.push(TrainingResult {
            timestamp: now,
            passed,
            evaluation,
        });
        self.last_training_date = Some(now);

        if passed {
            self.add_buddy_exp();
            self.current_streak += 1;
            let total_correct = self.results.iter().filter(|r| r.passed).count();
            self.award_badges_for_progress(self.current_streak, total_correct, now);
        } else {
            self.current_streak = 0;
        }
    }

    fn get_stats_file_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
        let config_dir = dirs::config_dir().ok_or("設定ディレクトリが見つかりません。")?;
        Ok(config_dir.join(APP_DIR_NAME).join(STATS_FILE_NAME))
    }

    fn recalculate_streak(&mut self) {
        self.current_streak = self
            .results
            .iter()
            .rev()
            .take_while(|result| result.passed)
            .count();
    }

    fn rebuild_badges_from_history(&mut self) {
        let mut current_streak: usize = 0;
        let mut total_correct: usize = 0;

        let results = self.results.clone();
        for result in results {
            if result.passed {
                current_streak += 1;
                total_correct += 1;
                self.award_badges_for_progress(current_streak, total_correct, result.timestamp);
            } else {
                current_streak = 0;
            }
        }
    }

    pub fn get_daily_stats(&self, days: usize) -> HashMap<NaiveDate, DailyStats> {
        stats_analysis::calculate_daily_stats(&self.results, days, Local::now().date_naive())
    }

    pub fn get_weekly_stats(&self, weeks: usize) -> Vec<WeeklyStats> {
        stats_analysis::calculate_weekly_stats(&self.results, weeks, Local::now())
    }

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
        stats_analysis::get_recent_evaluation_summary(&self.results, days)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats_analysis::{
        calculate_daily_stats, calculate_median, calculate_score_stats, calculate_weekly_stats,
    };

    #[test]
    fn test_badge_awarding_consecutive() {
        let mut stats = TrainingStats::new();

        for _ in 0..5 {
            stats.add_result_with_evaluation(true, None);
        }

        let (consecutive, cumulative) = stats.get_badges_by_type();
        assert_eq!(consecutive.len(), 1);
        assert_eq!(cumulative.len(), 1);

        for _ in 0..5 {
            stats.add_result_with_evaluation(true, None);
        }

        let (consecutive, cumulative) = stats.get_badges_by_type();
        assert_eq!(consecutive.len(), 2);
        assert_eq!(cumulative.len(), 2);
    }

    #[test]
    fn test_streak_reset_on_incorrect() {
        let mut stats = TrainingStats::new();

        for _ in 0..5 {
            stats.add_result_with_evaluation(true, None);
        }

        assert_eq!(stats.current_streak, 5);

        stats.add_result_with_evaluation(false, None);

        assert_eq!(stats.current_streak, 0);

        let (consecutive, _) = stats.get_badges_by_type();
        assert_eq!(consecutive.len(), 1);
    }

    #[test]
    fn test_rebuild_badges_from_history() {
        let mut stats = TrainingStats::new();

        for _ in 0..10 {
            stats.add_result_with_evaluation(true, None);
        }

        stats.badges.clear();
        stats.current_streak = 0;

        stats.rebuild_badges_from_history();

        let (consecutive, cumulative) = stats.get_badges_by_type();
        assert_eq!(consecutive.len(), 2);
        assert_eq!(cumulative.len(), 2);
    }

    #[test]
    fn test_calculate_daily_stats() {
        let mut stats = TrainingStats::new();
        let today = Local::now().date_naive();

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

        let yesterday = Local::now() - chrono::Duration::days(1);
        stats.results.push(TrainingResult {
            timestamp: yesterday,
            passed: true,
            evaluation: None,
        });

        let daily_stats = calculate_daily_stats(&stats.results, 7, today);

        let today_stats = daily_stats.get(&today).cloned().unwrap_or_default();
        assert_eq!(today_stats.correct, 1);
        assert_eq!(today_stats.incorrect, 1);

        let yesterday_date = yesterday.date_naive();
        let yesterday_stats = daily_stats
            .get(&yesterday_date)
            .cloned()
            .unwrap_or_default();
        assert_eq!(yesterday_stats.correct, 1);
        assert_eq!(yesterday_stats.incorrect, 0);
    }

    #[test]
    fn test_calculate_weekly_stats() {
        let mut stats = TrainingStats::new();
        let now = Local::now();

        stats.results.push(TrainingResult {
            timestamp: now,
            passed: true,
            evaluation: None,
        });

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

        let weekly_stats = calculate_weekly_stats(&stats.results, 4, now);

        let this_week_stats = weekly_stats.last().cloned().unwrap_or(WeeklyStats {
            week_number: 0,
            correct: 0,
            incorrect: 0,
        });
        assert_eq!(this_week_stats.correct, 1);
        assert_eq!(this_week_stats.incorrect, 0);

        let last_week_stats = weekly_stats
            .get(weekly_stats.len().saturating_sub(2))
            .cloned()
            .unwrap_or(WeeklyStats {
                week_number: 0,
                correct: 0,
                incorrect: 0,
            });
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
        let importance = summary.importance.as_ref().map(|s| (s.average, s.median));
        assert!(importance.is_some());
        let importance = importance.unwrap_or((0.0, 0.0));
        assert!((importance.0 - 4.0).abs() < f32::EPSILON);
        assert!((importance.1 - 4.0).abs() < f32::EPSILON);

        let conciseness = summary.conciseness.as_ref().map(|s| (s.average, s.median));
        assert!(conciseness.is_some());
        let conciseness = conciseness.unwrap_or((0.0, 0.0));
        assert!((conciseness.0 - 2.5).abs() < f32::EPSILON);
        assert!((conciseness.1 - 2.5).abs() < f32::EPSILON);

        let accuracy = summary.accuracy.as_ref().map(|s| (s.average, s.median));
        assert!(accuracy.is_some());
        let accuracy = accuracy.unwrap_or((0.0, 0.0));
        assert!((accuracy.0 - 4.0).abs() < f32::EPSILON);
        assert!((accuracy.1 - 4.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_calculate_median_edge_cases() {
        assert!((calculate_median(&[5]) - 5.0).abs() < f32::EPSILON);
        assert!((calculate_median(&[1, 5]) - 3.0).abs() < f32::EPSILON);
        assert!((calculate_median(&[10, 2, 5]) - 5.0).abs() < f32::EPSILON);
        assert!((calculate_median(&[1, 2, 3, 4]) - 2.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_calculate_score_stats_handles_empty() {
        assert!(calculate_score_stats(&[]).is_none());
    }

    #[test]
    fn test_recalculate_streak_variations() {
        let mut stats = TrainingStats::new();

        stats.recalculate_streak();
        assert_eq!(stats.current_streak, 0);

        for _ in 0..3 {
            stats.results.push(TrainingResult {
                timestamp: Local::now(),
                passed: true,
                evaluation: None,
            });
        }
        stats.recalculate_streak();
        assert_eq!(stats.current_streak, 3);

        stats.results.push(TrainingResult {
            timestamp: Local::now(),
            passed: false,
            evaluation: None,
        });
        stats.results.push(TrainingResult {
            timestamp: Local::now(),
            passed: true,
            evaluation: None,
        });
        stats.recalculate_streak();
        assert_eq!(stats.current_streak, 1);
    }

    #[test]
    fn test_badge_display_text_japanese() {
        let now = Local::now();
        let b1 = Badge {
            badge_type: BadgeType::ConsecutiveStreak(5),
            earned_at: now,
        };
        let b2 = Badge {
            badge_type: BadgeType::CumulativeMilestone(10),
            earned_at: now,
        };
        assert_eq!(b1.get_display_text(), "5連");
        assert_eq!(b2.get_display_text(), "累積10");
    }

    #[test]
    fn test_buddy_growth() {
        let mut stats = TrainingStats::new();
        assert_eq!(stats.buddy.level, 1);
        assert_eq!(stats.buddy.exp, 0);

        for _ in 0..5 {
            stats.add_result_with_evaluation(true, None);
        }
        assert_eq!(stats.buddy.level, 2);
        assert_eq!(stats.buddy.exp, 0);

        for _ in 0..9 {
            stats.add_result_with_evaluation(true, None);
        }
        assert_eq!(stats.buddy.level, 2);
        assert_eq!(stats.buddy.exp, 9);

        stats.add_result_with_evaluation(true, None);
        assert_eq!(stats.buddy.level, 3);
        assert_eq!(stats.buddy.exp, 0);

        for _ in 0..4 {
            stats.add_result_with_evaluation(true, None);
        }
        assert_eq!(stats.buddy.level, 3);
        assert_eq!(stats.buddy.exp, 4);

        stats.add_result_with_evaluation(false, None);
        assert_eq!(stats.buddy.exp, 4);
    }

    #[test]
    fn test_buddy_penalty() {
        let mut stats = TrainingStats::new();
        stats.buddy.level = 2;
        stats.buddy.exp = 3;
        stats.last_training_date = Some(Local::now() - chrono::Duration::days(3));

        stats.check_buddy_penalty();

        assert_eq!(stats.buddy.level, 1);
        assert_eq!(stats.buddy.exp, 0);
        assert!(
            stats
                .last_training_date
                .is_some_and(|date| date > Local::now() - chrono::Duration::minutes(1))
        );
    }
}
