use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Default)]
struct Config {
    api_key: Option<String>,
}

fn get_config_path() -> Result<PathBuf, AppError> {
    let config_dir = dirs::config_dir().ok_or(AppError::IoError(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "Config directory not found",
    )))?;
    let app_config_dir = config_dir.join("yomitore");
    fs::create_dir_all(&app_config_dir)?;
    Ok(app_config_dir.join("config.toml"))
}

pub fn save_api_key(api_key: &str) -> Result<(), AppError> {
    let config_path = get_config_path()?;
    let config = Config {
        api_key: Some(api_key.to_string()),
    };
    let toml_string = toml::to_string(&config)
        .map_err(|_| AppError::IoError(std::io::Error::other("Failed to serialize config")))?;

    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&config_path)?;

    // Set file permissions to 600 on Unix-like systems
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = file.metadata()?.permissions();
        perms.set_mode(0o600);
        file.set_permissions(perms)?;
    }

    file.write_all(toml_string.as_bytes())?;
    Ok(())
}

pub fn load_api_key() -> Result<Option<String>, AppError> {
    // 1. 環境変数 GROQ_API_KEY を最優先に従う
    if let Ok(key) = std::env::var("GROQ_API_KEY") {
        let key = key.trim();
        if !key.is_empty() {
            return Ok(Some(key.to_string()));
        }
    }

    // 2. 設定ファイルから読み込む
    let config_path = match get_config_path() {
        Ok(path) => path,
        Err(_) => return Ok(None),
    };

    if !config_path.exists() {
        return Ok(None);
    }

    let mut file = File::open(config_path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let config: Config = toml::from_str(&contents)
        .map_err(|_| AppError::IoError(std::io::Error::other("Failed to parse config")))?;

    Ok(config.api_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_serialization() {
        let config = Config {
            api_key: Some("test_key".to_string()),
        };
        let toml = toml::to_string(&config).unwrap();
        assert!(toml.contains("api_key = \"test_key\""));
    }

    #[test]
    fn test_config_deserialization() {
        let toml_str = "api_key = \"secret_key\"";
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.api_key, Some("secret_key".to_string()));
    }

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert!(config.api_key.is_none());
    }

    #[test]
    fn test_api_key_loading_priority() {
        use std::env;
        let env_var_name = "GROQ_API_KEY";
        let original_env = env::var(env_var_name).ok();

        // Ensure env var is set
        unsafe {
            env::set_var(env_var_name, "env_key");
        }
        let result = load_api_key().unwrap();
        assert_eq!(result, Some("env_key".to_string()));

        // Restore env var
        unsafe {
            if let Some(val) = original_env {
                env::set_var(env_var_name, val);
            } else {
                env::remove_var(env_var_name);
            }
        }
    }

    #[test]
    #[cfg(unix)]
    fn test_save_api_key_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        // We can't easily mock get_config_path, but we can test the serialization and permission logic
        let api_key = "test_perm_key";
        let config = Config {
            api_key: Some(api_key.to_string()),
        };
        let toml_string = toml::to_string(&config).unwrap();

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&config_path)
            .unwrap();

        let mut perms = file.metadata().unwrap().permissions();
        perms.set_mode(0o600);
        file.set_permissions(perms).unwrap();
        file.write_all(toml_string.as_bytes()).unwrap();

        let metadata = fs::metadata(&config_path).unwrap();
        assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
    }
}
