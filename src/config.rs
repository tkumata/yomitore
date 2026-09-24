use crate::error::AppError;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct ApiKeys {
    pub groq: Option<String>,
    pub jev: Option<String>,
    pub groq_from_env: bool,
    pub groq_saved: bool,
}

fn get_config_path() -> Result<PathBuf, AppError> {
    let config_dir = dirs::config_dir().ok_or(AppError::IoError(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "設定ディレクトリが見つかりません。",
    )))?;
    Ok(config_dir.join("yomitore").join("config.toml"))
}

pub fn load_api_keys() -> Result<ApiKeys, AppError> {
    let path = get_config_path()?;
    let groq_env = groq_api_key_from_env();
    load_api_keys_from_path(&path, groq_env.as_deref())
}

fn load_api_keys_from_path(path: &Path, groq_env: Option<&str>) -> Result<ApiKeys, AppError> {
    let config = read_config(path)?;
    let saved_groq = config
        .get("api_key")
        .and_then(toml::Value::as_str)
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(str::to_owned);
    let saved_jev = config
        .get("jev_api_key")
        .and_then(toml::Value::as_str)
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(str::to_owned);
    Ok(ApiKeys {
        groq: groq_env.map(str::to_owned).or(saved_groq.clone()),
        jev: saved_jev,
        groq_from_env: groq_env.is_some(),
        groq_saved: saved_groq.is_some(),
    })
}

pub fn groq_api_key_from_env() -> Option<String> {
    std::env::var("GROQ_API_KEY")
        .ok()
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty())
}

pub fn load_groq_api_key() -> Result<Option<String>, AppError> {
    if let Some(key) = groq_api_key_from_env() {
        return Ok(Some(key));
    }
    Ok(load_api_keys()?.groq)
}

pub fn save_api_keys(groq: Option<&str>, jev: Option<&str>) -> Result<(), AppError> {
    let path = get_config_path()?;
    save_api_keys_to_path(&path, groq, jev)
}

fn save_api_keys_to_path(
    path: &Path,
    groq: Option<&str>,
    jev: Option<&str>,
) -> Result<(), AppError> {
    let mut config = read_config(path)?;
    if let Some(key) = groq.filter(|key| !key.trim().is_empty()) {
        config.insert("api_key".to_string(), toml::Value::String(key.to_string()));
    }
    if let Some(key) = jev.filter(|key| !key.trim().is_empty()) {
        config.insert(
            "jev_api_key".to_string(),
            toml::Value::String(key.to_string()),
        );
    }
    let contents = toml::to_string(&config)
        .map_err(|_| AppError::IoError(std::io::Error::other("設定の保存に失敗しました。")))?;
    write_config_atomically(path, &contents)
}

fn read_config(path: &Path) -> Result<toml::Table, AppError> {
    match fs::read_to_string(path) {
        Ok(contents) => toml::from_str(&contents)
            .map_err(|_| AppError::IoError(std::io::Error::other("設定の解析に失敗しました。"))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(toml::Table::new()),
        Err(error) => Err(AppError::IoError(error)),
    }
}

fn write_config_atomically(path: &Path, contents: &str) -> Result<(), AppError> {
    let parent = path
        .parent()
        .ok_or(AppError::IoError(std::io::Error::other(
            "設定先が不正です。",
        )))?;
    fs::create_dir_all(parent)?;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary =
        path.with_file_name(format!("config.toml.{}.{}.tmp", std::process::id(), nonce));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let write_result = (|| -> Result<(), AppError> {
        let mut file = options.open(&temporary)?;
        file.write_all(contents.as_bytes())?;
        file.sync_all()?;
        Ok(())
    })();
    if let Err(error) = write_result {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }

    #[cfg(unix)]
    let commit_result = (|| -> Result<(), AppError> {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600))?;
        fs::rename(&temporary, path)?;
        Ok(())
    })();
    #[cfg(not(unix))]
    let commit_result = {
        if !path.exists() {
            fs::rename(&temporary, path).map_err(AppError::from)
        } else {
            let backup =
                path.with_file_name(format!("config.toml.{}.{}.bak", std::process::id(), nonce));
            match fs::rename(path, &backup) {
                Err(error) => Err(AppError::from(error)),
                Ok(()) => match fs::rename(&temporary, path) {
                    Ok(()) => fs::remove_file(backup).map_err(AppError::from),
                    Err(error) => {
                        if fs::rename(&backup, path).is_err() {
                            Err(AppError::IoError(std::io::Error::other(
                                "設定の置換に失敗し、旧設定はバックアップに退避されています。",
                            )))
                        } else {
                            Err(AppError::from(error))
                        }
                    }
                },
            }
        }
    };
    if commit_result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    commit_result
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TemporaryDirectory(PathBuf);

    impl TemporaryDirectory {
        fn new() -> std::io::Result<Self> {
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "yomitore-config-test-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir(&path)?;
            Ok(Self(path))
        }

        fn config_path(&self) -> PathBuf {
            self.0.join("config.toml")
        }
    }

    impl Drop for TemporaryDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[derive(Debug, PartialEq, Eq)]
    struct SaveObservation {
        first_groq: Option<String>,
        first_jev: Option<String>,
        other_value: Option<i64>,
        overwritten_groq: Option<String>,
        retained_jev: Option<String>,
        mode: Option<u32>,
    }

    fn observe_save_and_overwrite() -> Result<SaveObservation, String> {
        let temp = TemporaryDirectory::new().map_err(|error| error.to_string())?;
        let path = temp.config_path();
        fs::write(&path, "other_value = 42\n").map_err(|error| error.to_string())?;

        save_api_keys_to_path(&path, Some("groq-1"), Some("jev-1"))
            .map_err(|error| error.to_string())?;
        let first = read_config(&path).map_err(|error| error.to_string())?;

        save_api_keys_to_path(&path, Some("groq-2"), None).map_err(|error| error.to_string())?;
        let overwritten = read_config(&path).map_err(|error| error.to_string())?;

        let mode = {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;

                Some(
                    fs::metadata(&path)
                        .map_err(|error| error.to_string())?
                        .permissions()
                        .mode()
                        & 0o777,
                )
            }
            #[cfg(not(unix))]
            {
                None
            }
        };
        Ok(SaveObservation {
            first_groq: first
                .get("api_key")
                .and_then(toml::Value::as_str)
                .map(str::to_owned),
            first_jev: first
                .get("jev_api_key")
                .and_then(toml::Value::as_str)
                .map(str::to_owned),
            other_value: first.get("other_value").and_then(toml::Value::as_integer),
            overwritten_groq: overwritten
                .get("api_key")
                .and_then(toml::Value::as_str)
                .map(str::to_owned),
            retained_jev: overwritten
                .get("jev_api_key")
                .and_then(toml::Value::as_str)
                .map(str::to_owned),
            mode,
        })
    }

    #[test]
    fn saves_and_overwrites_keys_while_preserving_existing_settings() {
        assert_eq!(
            observe_save_and_overwrite(),
            Ok(SaveObservation {
                first_groq: Some("groq-1".to_string()),
                first_jev: Some("jev-1".to_string()),
                other_value: Some(42),
                overwritten_groq: Some("groq-2".to_string()),
                retained_jev: Some("jev-1".to_string()),
                mode: if cfg!(unix) { Some(0o600) } else { None },
            })
        );
    }

    fn observe_failed_save() -> Result<(bool, String), String> {
        let temp = TemporaryDirectory::new().map_err(|error| error.to_string())?;
        let path = temp.config_path();
        let original = "api_key = [invalid toml";
        fs::write(&path, original).map_err(|error| error.to_string())?;
        let failed = save_api_keys_to_path(&path, Some("new-key"), None).is_err();
        let unchanged = fs::read_to_string(path).map_err(|error| error.to_string())?;
        Ok((failed, unchanged))
    }

    #[test]
    fn failed_save_keeps_existing_config_contents() {
        assert_eq!(
            observe_failed_save(),
            Ok((true, "api_key = [invalid toml".to_string()))
        );
    }

    #[test]
    fn blank_saved_keys_are_unset_in_loaded_state() {
        assert_eq!(observe_blank_saved_keys(), Ok((true, true, true, true)));
    }

    fn observe_blank_saved_keys() -> Result<(bool, bool, bool, bool), String> {
        let temp = TemporaryDirectory::new().map_err(|error| error.to_string())?;
        let path = temp.config_path();
        fs::write(&path, "api_key = \"  \"\njev_api_key = \"\t\"\n")
            .map_err(|error| error.to_string())?;

        let keys = load_api_keys_from_path(&path, None).map_err(|error| error.to_string())?;
        Ok((
            keys.groq.is_none(),
            keys.jev.is_none(),
            !keys.groq_saved,
            !keys.groq_from_env,
        ))
    }
}
