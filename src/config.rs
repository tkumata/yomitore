use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Read;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Default)]
struct Config {
    api_key: Option<String>,
}

fn get_config_path() -> Result<PathBuf, AppError> {
    let config_dir = dirs::config_dir().ok_or(AppError::IoError(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "設定ディレクトリが見つかりません。",
    )))?;
    let app_config_dir = config_dir.join("yomitore");
    fs::create_dir_all(&app_config_dir)?;
    Ok(app_config_dir.join("config.toml"))
}

pub fn load_api_key() -> Result<Option<String>, AppError> {
    if let Ok(key) = std::env::var("GROQ_API_KEY") {
        let key = key.trim();
        if !key.is_empty() {
            return Ok(Some(key.to_string()));
        }
    }

    let Ok(config_path) = get_config_path() else {
        return Ok(None);
    };

    if !config_path.exists() {
        return Ok(None);
    }

    let mut file = File::open(config_path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let config: Config = toml::from_str(&contents)
        .map_err(|_| AppError::IoError(std::io::Error::other("設定の解析に失敗しました。")))?;

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
        let toml = toml::to_string(&config).unwrap_or_default();
        assert!(toml.contains("api_key = \"test_key\""));
    }

    #[test]
    fn test_config_deserialization() {
        let toml_str = "api_key = \"secret_key\"";
        let config: Config = toml::from_str(toml_str).unwrap_or_default();
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
        let result = load_api_key().unwrap_or_default();
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
}
