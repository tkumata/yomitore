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
    let config_dir = dirs::config_dir().ok_or(AppError::IoError(
        std::io::Error::new(std::io::ErrorKind::NotFound, "Config directory not found"),
    ))?;
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

    let mut file = OpenOptions::new().write(true).create(true).truncate(true).open(&config_path)?;
    
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
