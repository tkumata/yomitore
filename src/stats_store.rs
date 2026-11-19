use crate::{models::EvaluationScores, stats::TrainingStats};
use jiff::Timestamp;
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    mem::DropGuard,
    path::{Path, PathBuf},
};

fn stats_path() -> io::Result<PathBuf> {
    let directory =
        dirs::config_dir().ok_or_else(|| io::Error::other("設定ディレクトリが見つかりません。"))?;
    Ok(directory.join("yomitore").join("stats.json"))
}

pub fn load() -> io::Result<TrainingStats> {
    load_from(&stats_path()?)
}

fn load_from(path: &Path) -> io::Result<TrainingStats> {
    // Check the directory entry first, so a concurrent first save cannot look
    // like a dangling symlink after a read reports NotFound.
    match fs::symlink_metadata(path) {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(TrainingStats::default());
        }
        Err(error) => return Err(error),
    }
    let content = fs::read_to_string(path)?;
    let mut stats: TrainingStats = serde_json::from_str(&content)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    stats.restore_derived_state(Timestamp::now());
    Ok(stats)
}

pub fn record(evaluation: &EvaluationScores) -> io::Result<TrainingStats> {
    record_at(&stats_path()?, evaluation)
}

fn record_at(path: &Path, evaluation: &EvaluationScores) -> io::Result<TrainingStats> {
    let target = match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_symlink() => Some(path.canonicalize()?),
        Ok(_) => None,
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    let path = target.as_deref().unwrap_or(path);
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("成績履歴の保存先が不正です。"))?;
    fs::create_dir_all(parent)?;
    // Keep the lock file in place so every writer locks the same file across replacements.
    let lock = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(path.with_added_extension("lock"))?;
    lock.lock()?;

    let mut stats = load_from(path)?;
    stats.add_result_with_evaluation(evaluation.overall_passed, Some(evaluation.clone()));
    replace(path, &serde_json::to_vec_pretty(&stats)?)?;
    Ok(stats)
}

fn replace(path: &Path, contents: &[u8]) -> io::Result<()> {
    let temporary = path.with_added_extension(format!("{}.tmp", rand::random::<u64>()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(&temporary)?;
    let cleanup = DropGuard::new(&temporary, |path| {
        if let Err(error) = fs::remove_file(path) {
            let _ = writeln!(
                io::stderr(),
                "成績履歴の一時ファイルを削除できませんでした: {error}"
            );
        }
    });
    {
        let mut file = file;
        file.write_all(contents)?;
        file.sync_all()?;
    }
    fs::rename(&temporary, path)?;
    DropGuard::dismiss(cleanup);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> io::Result<DropGuard<PathBuf, impl FnOnce(PathBuf)>> {
        let path = std::env::temp_dir().join(format!("yomitore-stats-{}", rand::random::<u64>()));
        fs::create_dir(&path)?;
        Ok(DropGuard::new(path, |path| {
            let _ = fs::remove_dir_all(path);
        }))
    }

    fn scores() -> EvaluationScores {
        EvaluationScores {
            appropriate: true,
            importance: 5,
            conciseness: 5,
            accuracy: 5,
            improvement1: String::new(),
            improvement2: String::new(),
            improvement3: String::new(),
            overall_passed: true,
            total_score: Some(100),
        }
    }

    #[test]
    #[expect(
        clippy::panic_in_result_fn,
        reason = "Test assertions report the failed invariant"
    )]
    fn preserves_corrupted_history_at_load_and_record() -> io::Result<()> {
        let dir = temp_dir()?;
        let path = dir.join("stats.json");
        fs::write(&path, b"{")?;

        assert!(
            matches!(load_from(&path), Err(error) if error.kind() == io::ErrorKind::InvalidData)
        );
        assert!(
            matches!(record_at(&path, &scores()), Err(error) if error.kind() == io::ErrorKind::InvalidData)
        );
        assert_eq!(fs::read(&path)?, b"{");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn dangling_history_symlink_returns_a_load_error() -> io::Result<()> {
        let dir = temp_dir()?;
        let path = dir.join("stats.json");
        std::os::unix::fs::symlink(dir.join("missing.json"), &path)?;

        match load_from(&path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
            Ok(_) => Err(io::Error::other(
                "リンク切れの履歴が新規履歴として扱われました。",
            )),
        }
    }

    #[test]
    #[expect(
        clippy::panic_in_result_fn,
        reason = "Test assertions report the failed invariant"
    )]
    fn concurrent_records_keep_both_results_and_buddy_progress() -> io::Result<()> {
        let dir = temp_dir()?;
        let path = dir.join("stats.json");
        let start = std::sync::Barrier::new(2);
        std::thread::scope(|scope| {
            let record = || {
                start.wait();
                record_at(&path, &scores()).map(|_| ())
            };
            let first = scope.spawn(record);
            record()?;
            first
                .join()
                .map_err(|_| io::Error::other("成績の保存スレッドが異常終了しました。"))?
        })?;

        let stats = load_from(&path)?;
        assert_eq!(stats.results.len(), 2);
        assert_eq!(stats.current_streak, 2);
        assert_eq!(stats.buddy.exp, 2);
        Ok(())
    }

    #[test]
    #[expect(
        clippy::panic_in_result_fn,
        reason = "Test assertions report the failed invariant"
    )]
    fn failed_replacement_cleans_temporary_file_and_retry_records_once() -> io::Result<()> {
        let dir = temp_dir()?;
        let path = dir.join("stats.json");
        fs::create_dir(&path)?;
        let witness = path.join("keep");
        fs::write(&witness, b"existing")?;

        assert!(replace(&path, b"new").is_err());
        assert_eq!(fs::read(&witness)?, b"existing");
        assert_eq!(fs::read_dir(&*dir)?.count(), 1);

        fs::remove_file(witness)?;
        fs::remove_dir(&path)?;
        let stats = record_at(&path, &scores())?;
        assert_eq!(stats.results.len(), 1);
        assert_eq!(load_from(&path)?.results.len(), 1);
        Ok(())
    }
}
