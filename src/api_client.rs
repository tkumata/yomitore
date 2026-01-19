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
            "以下の『要約文』は『原文』を適切に要約しているか「はい」か「いいえ」で端的に答えた上で以下の観点で評価せよ。\n\n- 重要情報の抽出(5段階): 主要なポイントを捉えているか\n\n- 簡潔性(5段階): 冗長な表現がないか\n\n- 正確性(5段階): 事実の誤認や歪曲がないか\n\n- 具体的な改善点: 3つ挙げてください。\n\n- 総合評価: 合格/不合格\n\n表示部は CLI ターミナルなので markdown 形式の出力は禁止とする。\n\n# 原文\n{}\n\n# 要約文\n{}\n以下の『要約文』は『原文』を適切に要約しているか「はい」か「いいえ」で端的に答えた上で以下の観点で評価せよ。\n\n- 重要情報の抽出(5段階): 主要なポイントを捉えているか\n\n- 簡潔性(5段階): 冗長な表現がないか\n\n- 正確性(5段階): 事実の誤認や歪曲がないか\n\n- 具体的な改善点: 3つ挙げてください。\n\n- 総合評価: 合格/不合格\n\n表示部は CLI ターミナルなので markdown 形式の出力は禁止とする。\n\n# 原文\n{}\n\n# 要約文\n{}\n",
            original_text, summary_text, original_text, summary_text
        );
        self.send_chat_request(&prompt_content).await
    }
}
