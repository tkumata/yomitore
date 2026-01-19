use crate::error::AppError;
use serde::{Deserialize, Serialize};

// --- Data Structures for API Communication ---

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
}

#[derive(Deserialize, Debug)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize, Debug)]
struct Choice {
    message: ChatResponseMessage,
}

#[derive(Serialize, Deserialize, Debug)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

// NOTE: The 'content' field in the response can sometimes be null.
#[derive(Deserialize, Debug)]
struct ChatResponseMessage {
    content: Option<String>,
}

// --- API Client ---

const API_BASE_URL: &str = "https://api.groq.com/openai/v1";
const CHAT_COMPLETIONS_ENDPOINT: &str = "/chat/completions";
const MODELS_ENDPOINT: &str = "/models";
const CHAT_MODEL: &str = "openai/gpt-oss-120b";
const API_TIMEOUT_SECS: u64 = 60; // API request timeout in seconds

pub struct ApiClient {
    client: reqwest::Client,
    api_key: String,
}

impl ApiClient {
    pub fn new(api_key: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(API_TIMEOUT_SECS))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self { client, api_key }
    }

    pub async fn validate_credentials(&self) -> Result<(), AppError> {
        let url = format!("{}{}", API_BASE_URL, MODELS_ENDPOINT);
        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.api_key)
            .send()
            .await?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(AppError::InvalidApiKey)
        }
    }

    /// Common helper for chat completion requests
    async fn send_chat_request(&self, prompt: &str) -> Result<String, AppError> {
        let url = format!("{}{}", API_BASE_URL, CHAT_COMPLETIONS_ENDPOINT);
        let messages = vec![ChatMessage {
            role: "user",
            content: prompt,
        }];
        let request_body = ChatRequest {
            model: CHAT_MODEL,
            messages,
        };

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&request_body)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(AppError::ApiError(response.error_for_status().unwrap_err()));
        }

        let chat_response: ChatResponse = response.json().await?;

        if let Some(choice) = chat_response.choices.into_iter().next() {
            // Handle potential null content
            Ok(choice.message.content.unwrap_or_default())
        } else {
            Err(AppError::NoChoicesInResponse)
        }
    }

    pub async fn generate_text(&self, prompt: &str) -> Result<String, AppError> {
        self.send_chat_request(prompt).await
    }

    pub async fn evaluate_summary(
        &self,
        original_text: &str,
        summary_text: &str,
    ) -> Result<String, AppError> {
        let prompt_content = format!(
            "以下の「原文」と「要約文」を比較し、要約として適切か評価してください。\n\n# 評価ルール\n- 出力は必ず以下のフォーマットのみ使用すること\n- 数値は 1〜5 の整数のみ\n- 余計な文章や注釈は禁止\n- Markdown 記法は禁止\n\n# 出力フォーマット(厳守)\n- 適切な要約か: はい／いいえ\n- 重要情報の抽出: [1-5]\n- 簡潔性: [1-5]\n- 正確性: [1-5]\n- 改善点1: ...\n- 改善点2: ...\n- 改善点3: ...\n- 総合評価: 合格/不合格\n\n# 採点基準\n- 5: 非常に優れている\n- 3: 可もなく不可もなく\n- 1: 明確な問題がある\n\n# 原文\n{}\n\n# 要約文\n{}\n",
            original_text, summary_text
        );
        self.send_chat_request(&prompt_content).await
    }
}
