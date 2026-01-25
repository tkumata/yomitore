use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Buddy {
    pub level: u32,
    pub exp: u32,
}

impl Default for Buddy {
    fn default() -> Self {
        Self { level: 1, exp: 0 }
    }
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
