use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("API request failed: {0}")]
    ApiError(#[from] reqwest::Error),

    #[error("Failed to parse API response: {0}")]
    ResponseParseError(#[from] serde_json::Error),

    #[error("Invalid API Key.")]
    InvalidApiKey,

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("API response contained no choices.")]
    NoChoicesInResponse,
}
