use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("API リクエストに失敗しました: {0}")]
    ApiError(#[from] reqwest::Error),

    #[error("API レスポンスの解析に失敗しました: {0}")]
    ResponseParseError(#[from] serde_json::Error),

    #[error("API キーが無効です。")]
    InvalidApiKey,

    #[error("HTTP クライアントを初期化できませんでした。")]
    ApiClientInitialization,

    #[error("API が HTTP {0} を返しました。")]
    HttpStatus(reqwest::StatusCode),

    #[error("I/O エラー: {0}")]
    IoError(#[from] std::io::Error),

    #[error("API レスポンスに choices が含まれていません。")]
    NoChoicesInResponse,

    #[error("API レスポンスの本文が空です。")]
    EmptyChatContent,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_messages() {
        assert_eq!(AppError::InvalidApiKey.to_string(), "API キーが無効です。");
        assert_eq!(
            AppError::NoChoicesInResponse.to_string(),
            "API レスポンスに choices が含まれていません。"
        );
    }
}
