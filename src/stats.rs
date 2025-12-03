use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TrainingResult {
    pub timestamp: DateTime<Local>,
    pub passed: bool,
}

#[derive(Serialize, Deserialize, Default)]
pub struct TrainingStats {
    pub results: Vec<TrainingResult>,
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
        let stats: TrainingStats = serde_json::from_str(&content)?;
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

    pub fn add_result(&mut self, passed: bool) {
        self.results.push(TrainingResult {
            timestamp: Local::now(),
            passed,
        });
    }

    fn get_stats_file_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
        let home = dirs::home_dir().ok_or("Could not find home directory")?;
        Ok(home.join(".config").join("yomitore").join("stats.json"))
    }

    /// Get daily aggregated stats for the last N days
    pub fn get_daily_stats(&self, days: usize) -> HashMap<NaiveDate, DailyStats> {
        let mut daily_map: HashMap<NaiveDate, DailyStats> = HashMap::new();
        let today = Local::now().date_naive();

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
        let mut weekly_stats = Vec::new();
        let now = Local::now();

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
