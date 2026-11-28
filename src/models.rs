use crate::evaluation::{JevEvaluation, Judgment, Outcome, deserialize_bounded};
use jiff::{Timestamp, tz::TimeZone};
use serde::{Deserialize, Serialize};

pub mod local_timestamp {
    use super::{Serialize, TimeZone, Timestamp};
    use serde::Serializer;
    use std::fmt::Display;

    fn display(timestamp: Timestamp) -> impl Display {
        timestamp.display_with_offset(TimeZone::system().to_offset(timestamp))
    }

    pub fn serialize<S: Serializer>(
        timestamp: &Timestamp,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&display(*timestamp))
    }

    #[expect(clippy::ref_option, reason = "Serde passes a reference to the field")]
    pub fn serialize_option<S: Serializer>(
        timestamp: &Option<Timestamp>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        timestamp
            .map(|value| display(value).to_string())
            .serialize(serializer)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TrainingResult {
    #[serde(serialize_with = "local_timestamp::serialize")]
    pub timestamp: Timestamp,
    #[serde(flatten)]
    pub outcome: RecordedOutcome,
}

impl TrainingResult {
    pub fn outcome(&self) -> Outcome {
        match &self.outcome {
            RecordedOutcome::Weighted(WeightedOutcome::V3 { evaluation }) => {
                evaluation.judgment.outcome()
            }
            RecordedOutcome::Weighted(WeightedOutcome::V2 { evaluation }) => {
                evaluation.judgment.outcome()
            }
            RecordedOutcome::Legacy(legacy) if legacy.passed => Outcome::Pass,
            RecordedOutcome::Legacy(_) => Outcome::Fail,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum RecordedOutcome {
    Weighted(WeightedOutcome),
    Legacy(LegacyOutcome),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "evaluation_version")]
pub enum WeightedOutcome {
    #[serde(rename = "v2")]
    V2 {
        #[serde(deserialize_with = "deserialize_weighted_evaluation")]
        evaluation: EvaluationScores<SummaryScores>,
    },
    #[serde(rename = "v3")]
    V3 {
        #[serde(deserialize_with = "deserialize_weighted_evaluation")]
        evaluation: EvaluationScores,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct LegacyOutcome {
    pub passed: bool,
    #[serde(default)]
    pub evaluation: Option<LegacyEvaluationScores>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum BadgeType {
    ConsecutiveStreak(usize),   // 連続正解数 (5, 10, 15, ...)
    CumulativeMilestone(usize), // 累積正解数 (5, 10, 15, ...)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Badge {
    pub badge_type: BadgeType,
    #[serde(serialize_with = "local_timestamp::serialize")]
    pub earned_at: Timestamp,
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
            BadgeType::ConsecutiveStreak(n) => format!("{n}連"),
            BadgeType::CumulativeMilestone(n) => format!("累積{n}"),
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
pub struct EvaluationScores<S = JevEvaluation> {
    pub scores: S,
    pub total_score: u8,
    pub judgment: Judgment,
    pub improvements: [String; 3],
}

fn deserialize_weighted_evaluation<'de, D: serde::Deserializer<'de>, S: Deserialize<'de>>(
    deserializer: D,
) -> Result<EvaluationScores<S>, D::Error> {
    let evaluation = EvaluationScores::deserialize(deserializer)?;
    if evaluation.total_score > 100
        || evaluation
            .improvements
            .iter()
            .any(|text| text.trim().is_empty())
    {
        return Err(serde::de::Error::custom("invalid weighted evaluation"));
    }
    Ok(evaluation)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SummaryScores {
    #[serde(deserialize_with = "deserialize_bounded::<_, 4>")]
    pub faithfulness: f64,
    #[serde(deserialize_with = "deserialize_bounded::<_, 4>")]
    pub coverage: f64,
    #[serde(deserialize_with = "deserialize_bounded::<_, 4>")]
    pub relevance: f64,
    #[serde(deserialize_with = "deserialize_bounded::<_, 4>")]
    pub conciseness: f64,
    #[serde(deserialize_with = "deserialize_bounded::<_, 4>")]
    pub clarity: f64,
    #[serde(deserialize_with = "deserialize_bounded::<_, 1>")]
    pub critical_error: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LegacyEvaluationScores {
    pub appropriate: bool,
    pub importance: u8,
    pub conciseness: u8,
    pub accuracy: u8,
    pub improvement1: String,
    pub improvement2: String,
    pub improvement3: String,
    pub overall_passed: bool,
    #[serde(default)]
    pub total_score: Option<u8>,
}

#[derive(Clone, Debug)]
pub struct EvaluationScoreStats {
    pub average: f64,
    pub median: f64,
}

#[derive(Clone, Debug)]
pub struct EvaluationSummary<const N: usize> {
    pub count: usize,
    pub scores: [Option<EvaluationScoreStats>; N],
}

#[derive(Default, Clone, Debug)]
pub struct DailyStats {
    pub correct: usize,
    pub incorrect: usize,
    pub review: usize,
}

impl DailyStats {
    pub fn record(&mut self, outcome: Outcome) {
        match outcome {
            Outcome::Pass => self.correct += 1,
            Outcome::Fail => self.incorrect += 1,
            Outcome::Review => self.review += 1,
        }
    }
}

#[derive(Clone, Debug)]
pub struct WeeklyStats {
    pub week_number: usize,
    pub counts: DailyStats,
}
