use crate::models::{
    Badge, BadgeType, Buddy, DailyStats, EvaluationScores, EvaluationSummary, TrainingResult,
    WeeklyStats, local_timestamp,
};
use crate::stats_analysis;
use jiff::{SignedDuration, Timestamp, civil::Date, tz::TimeZone};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const BADGE_INTERVAL: usize = 5;
const MAX_CONSECUTIVE_STREAK: usize = 50;
const MAX_CUMULATIVE_MILESTONE: usize = 100;
const BUDDY_EXP_LEVEL2: u32 = 10;
const BUDDY_EXP_DEFAULT: u32 = 5;
const BUDDY_PENALTY_DELAY: SignedDuration = SignedDuration::from_hours(72);

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
    #[serde(default, serialize_with = "local_timestamp::serialize_option")]
    pub last_training_date: Option<Timestamp>,
}

impl TrainingStats {
    pub fn restore_derived_state(&mut self, now: Timestamp) {
        self.recalculate_streak();
        self.check_buddy_penalty(now);
        self.rebuild_badges_from_history();
    }

    fn award_badges_for_progress(
        badges: &mut Vec<Badge>,
        current_streak: usize,
        total_correct: usize,
        earned_at: Timestamp,
    ) {
        if current_streak.is_multiple_of(BADGE_INTERVAL) && current_streak <= MAX_CONSECUTIVE_STREAK
        {
            let badge = Badge {
                badge_type: BadgeType::ConsecutiveStreak(current_streak),
                earned_at,
            };
            if !badges.iter().any(|b| b.badge_type == badge.badge_type) {
                badges.push(badge);
            }
        }

        if total_correct.is_multiple_of(BADGE_INTERVAL) && total_correct <= MAX_CUMULATIVE_MILESTONE
        {
            let badge = Badge {
                badge_type: BadgeType::CumulativeMilestone(total_correct),
                earned_at,
            };
            if !badges.iter().any(|b| b.badge_type == badge.badge_type) {
                badges.push(badge);
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

    fn check_buddy_penalty(&mut self, now: Timestamp) {
        if let Some(last_date) = self.last_training_date
            && now.duration_since(last_date) >= BUDDY_PENALTY_DELAY
        {
            if self.buddy.level > 1 {
                self.buddy.level -= 1;
            }
            self.buddy.exp = 0;
            self.last_training_date = Some(now);
        }
    }

    pub fn add_result_with_evaluation(
        &mut self,
        passed: bool,
        evaluation: Option<EvaluationScores>,
    ) {
        let now = Timestamp::now();
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
            Self::award_badges_for_progress(
                &mut self.badges,
                self.current_streak,
                total_correct,
                now,
            );
        } else {
            self.current_streak = 0;
        }
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

        for result in &self.results {
            if result.passed {
                current_streak += 1;
                total_correct += 1;
                Self::award_badges_for_progress(
                    &mut self.badges,
                    current_streak,
                    total_correct,
                    result.timestamp,
                );
            } else {
                current_streak = 0;
            }
        }
    }

    pub fn get_daily_stats(
        &self,
        days: usize,
        today: Date,
        zone: &TimeZone,
    ) -> HashMap<Date, DailyStats> {
        stats_analysis::calculate_daily_stats(&self.results, days, today, zone)
    }

    pub fn get_weekly_stats(&self, weeks: usize, now: Timestamp) -> Vec<WeeklyStats> {
        stats_analysis::calculate_weekly_stats(&self.results, weeks, now)
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

    pub fn get_recent_evaluation_summary(
        &self,
        days: usize,
        today: Date,
        zone: &TimeZone,
    ) -> EvaluationSummary {
        stats_analysis::get_recent_evaluation_summary(&self.results, days, today, zone)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats_analysis::{
        calculate_daily_stats, calculate_median, calculate_score_stats, calculate_weekly_stats,
    };

    #[test]
    fn loads_legacy_evaluation_without_total_score() {
        let legacy_stats = r#"{
            "results": [{
                "timestamp": "2026-09-24T12:00:00+09:00",
                "passed": true,
                "evaluation": {
                    "appropriate": true,
                    "importance": 4,
                    "conciseness": 3,
                    "accuracy": 5,
                    "improvement1": "改善1",
                    "improvement2": "改善2",
                    "improvement3": "改善3",
                    "overall_passed": true
                }
            }]
        }"#;
        let total_score = serde_json::from_str::<TrainingStats>(legacy_stats)
            .map_err(|error| error.to_string())
            .map(|stats| {
                stats.results.first().map(|result| {
                    result
                        .evaluation
                        .as_ref()
                        .map(|evaluation| evaluation.total_score)
                })
            });

        assert_eq!(total_score, Ok(Some(Some(None))));
    }

    #[test]
    fn legacy_stats_timestamps_keep_their_instants_and_local_offsets() {
        let legacy = r#"{
            "results": [{"timestamp": "2026-09-24T12:00:00.123456789+09:00", "passed": true}],
            "badges": [{"badge_type": {"ConsecutiveStreak": 5},
                        "earned_at": "2026-09-24T03:00:01.120-04:00"}],
            "last_training_date": "2026-09-24T03:00:02.345Z"
        }"#;
        let Ok(stats) = serde_json::from_str::<TrainingStats>(legacy) else {
            unreachable!("invalid legacy stats fixture");
        };
        let timestamps = [
            (
                "/results/0/timestamp",
                stats.results.first().map(|result| result.timestamp),
                "2026-09-24T03:00:00.123456789Z",
            ),
            (
                "/badges/0/earned_at",
                stats.badges.first().map(|badge| badge.earned_at),
                "2026-09-24T07:00:01.120Z",
            ),
            (
                "/last_training_date",
                stats.last_training_date,
                "2026-09-24T03:00:02.345Z",
            ),
        ];
        let Ok(saved) = serde_json::to_value(&stats) else {
            unreachable!("test stats could not be serialized");
        };
        let zone = TimeZone::system();
        for (path, actual, expected) in timestamps {
            let Ok(expected) = expected.parse::<Timestamp>() else {
                unreachable!("invalid expected timestamp");
            };
            assert_eq!(actual, Some(expected), "{path}");
            let formatted = expected
                .display_with_offset(zone.to_offset(expected))
                .to_string();
            let saved_timestamp = saved.pointer(path).and_then(serde_json::Value::as_str);
            assert_eq!(saved_timestamp, Some(formatted.as_str()));
            assert_eq!(formatted.parse::<Timestamp>().ok(), Some(expected));

            let mut invalid = saved.clone();
            let Some(value) = invalid.pointer_mut(path) else {
                unreachable!("missing timestamp at {path}");
            };
            *value = serde_json::Value::String("invalid timestamp".to_owned());
            assert!(serde_json::from_value::<TrainingStats>(invalid).is_err());
        }

        for legacy in [
            r#"{"results": []}"#,
            r#"{"results": [], "last_training_date": null}"#,
        ] {
            let Ok(stats) = serde_json::from_str::<TrainingStats>(legacy) else {
                unreachable!("invalid legacy stats fixture");
            };
            assert!(stats.last_training_date.is_none());
            assert_eq!(
                serde_json::to_value(stats)
                    .ok()
                    .and_then(|value| value.get("last_training_date").cloned()),
                Some(serde_json::Value::Null)
            );
        }
    }

    #[test]
    fn test_badge_awarding_consecutive() {
        let mut stats = TrainingStats::default();

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
        let mut stats = TrainingStats::default();

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
        let mut stats = TrainingStats::default();

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
        let mut stats = TrainingStats::default();
        let zone = TimeZone::fixed(jiff::tz::offset(9));
        let today = jiff::civil::date(2026, 7, 3);
        let Ok(now) = "2026-07-02T15:30:00Z".parse() else {
            unreachable!("invalid timestamp fixture");
        };

        stats.results.push(TrainingResult {
            timestamp: now,
            passed: true,
            evaluation: None,
        });
        stats.results.push(TrainingResult {
            timestamp: now,
            passed: false,
            evaluation: None,
        });

        let Ok(yesterday) = "2026-07-02T14:59:59Z".parse() else {
            unreachable!("invalid timestamp fixture");
        };
        stats.results.push(TrainingResult {
            timestamp: yesterday,
            passed: true,
            evaluation: None,
        });

        let daily_stats = calculate_daily_stats(&stats.results, 7, today, &zone);

        let today_stats = daily_stats.get(&today).cloned().unwrap_or_default();
        assert_eq!(today_stats.correct, 1);
        assert_eq!(today_stats.incorrect, 1);

        let yesterday_date = jiff::civil::date(2026, 7, 2);
        let yesterday_stats = daily_stats
            .get(&yesterday_date)
            .cloned()
            .unwrap_or_default();
        assert_eq!(yesterday_stats.correct, 1);
        assert_eq!(yesterday_stats.incorrect, 0);
    }

    #[test]
    fn test_calculate_weekly_stats() {
        let mut stats = TrainingStats::default();
        let now = Timestamp::now();

        stats.results.push(TrainingResult {
            timestamp: now,
            passed: true,
            evaluation: None,
        });

        let last_week = now - SignedDuration::from_hours(168);
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

        let counts: Vec<_> = weekly_stats
            .iter()
            .map(|week| (week.week_number, week.correct, week.incorrect))
            .collect();
        assert_eq!(counts, [(1, 0, 0), (2, 0, 0), (3, 0, 2), (4, 1, 0)]);
    }

    #[test]
    fn weekly_stats_for_zero_weeks_are_empty() {
        let now = Timestamp::now();
        let results = [TrainingResult {
            timestamp: now,
            passed: true,
            evaluation: None,
        }];
        assert!(calculate_weekly_stats(&results, 0, now).is_empty());
    }

    #[test]
    fn weekly_stats_include_start_and_exclude_end_across_daylight_saving() {
        let (Ok(now), Ok(start), Ok(end)) = (
            "2026-03-11T12:00:00-04:00".parse::<Timestamp>(),
            "2026-03-04T11:00:00-05:00".parse::<Timestamp>(),
            "2026-03-18T12:00:00-04:00".parse::<Timestamp>(),
        ) else {
            unreachable!("invalid timestamp fixtures");
        };
        let results = [
            (start - SignedDuration::from_nanos(1), false),
            (start, true),
            (now - SignedDuration::from_nanos(1), false),
            (now, false),
            (end - SignedDuration::from_nanos(1), true),
            (end, false),
        ]
        .map(|(timestamp, passed)| TrainingResult {
            timestamp,
            passed,
            evaluation: None,
        });

        let counts: Vec<_> = calculate_weekly_stats(&results, 2, now)
            .iter()
            .map(|week| (week.week_number, week.correct, week.incorrect))
            .collect();
        assert_eq!(counts, [(1, 1, 1), (2, 1, 1)]);
    }

    #[test]
    fn test_recent_evaluation_summary() {
        let mut stats = TrainingStats::default();
        let zone = TimeZone::fixed(jiff::tz::offset(9));
        let today = jiff::civil::date(2026, 7, 3);
        let (Ok(now), Ok(earlier)) = (
            "2026-07-02T15:30:00Z".parse(),
            "2026-07-02T14:59:59Z".parse(),
        ) else {
            unreachable!("invalid timestamp fixtures");
        };

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
                total_score: Some(80),
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
                total_score: None,
            }),
        });

        let Some(result) = stats.results.first().cloned() else {
            unreachable!("missing test result");
        };
        stats.results.push(TrainingResult {
            timestamp: earlier,
            ..result.clone()
        });
        stats.results.push(TrainingResult {
            timestamp: now + SignedDuration::from_hours(24),
            ..result
        });
        let summary =
            stats_analysis::get_recent_evaluation_summary(&stats.results, 1, today, &zone);
        assert_eq!(summary.count, 2);
        let importance = summary.importance.as_ref().map(|s| (s.average, s.median));
        assert!(importance.is_some());
        let importance = importance.unwrap_or((0.0, 0.0));
        assert!((importance.0 - 4.0).abs() < f64::EPSILON);
        assert!((importance.1 - 4.0).abs() < f32::EPSILON);

        let conciseness = summary.conciseness.as_ref().map(|s| (s.average, s.median));
        assert!(conciseness.is_some());
        let conciseness = conciseness.unwrap_or((0.0, 0.0));
        assert!((conciseness.0 - 2.5).abs() < f64::EPSILON);
        assert!((conciseness.1 - 2.5).abs() < f32::EPSILON);

        let accuracy = summary.accuracy.as_ref().map(|s| (s.average, s.median));
        assert!(accuracy.is_some());
        let accuracy = accuracy.unwrap_or((0.0, 0.0));
        assert!((accuracy.0 - 4.0).abs() < f64::EPSILON);
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
    fn average_keeps_large_sums_and_counts() {
        for count in [13_108, 65_536] {
            let Some(stats) = calculate_score_stats(&vec![5; count]) else {
                unreachable!("fixture contains scores");
            };
            assert!((stats.average - 5.0).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn test_recalculate_streak_variations() {
        let mut stats = TrainingStats::default();

        stats.recalculate_streak();
        assert_eq!(stats.current_streak, 0);

        for _ in 0..3 {
            stats.results.push(TrainingResult {
                timestamp: Timestamp::now(),
                passed: true,
                evaluation: None,
            });
        }
        stats.recalculate_streak();
        assert_eq!(stats.current_streak, 3);

        stats.results.push(TrainingResult {
            timestamp: Timestamp::now(),
            passed: false,
            evaluation: None,
        });
        stats.results.push(TrainingResult {
            timestamp: Timestamp::now(),
            passed: true,
            evaluation: None,
        });
        stats.recalculate_streak();
        assert_eq!(stats.current_streak, 1);
    }

    #[test]
    fn test_badge_display_text_japanese() {
        let now = Timestamp::now();
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
        let mut stats = TrainingStats::default();
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
    fn test_buddy_penalty_at_72_hours() {
        let (Ok(last), Ok(now)) = (
            "2026-03-06T12:00:00-05:00".parse::<Timestamp>(),
            "2026-03-09T13:00:00-04:00".parse::<Timestamp>(),
        ) else {
            unreachable!("invalid timestamp fixtures");
        };
        let mut stats = TrainingStats::default();
        stats.buddy.level = 2;
        stats.buddy.exp = 3;
        stats.last_training_date = Some(last);

        stats.check_buddy_penalty(now - SignedDuration::from_nanos(1));
        assert_eq!(stats.buddy.level, 2);
        assert_eq!(stats.buddy.exp, 3);
        assert_eq!(stats.last_training_date, Some(last));

        stats.check_buddy_penalty(now);
        assert_eq!(stats.buddy.level, 1);
        assert_eq!(stats.buddy.exp, 0);
        assert_eq!(stats.last_training_date, Some(now));
    }
}
