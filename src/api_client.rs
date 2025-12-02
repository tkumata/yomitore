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

pub struct ApiClient {
    client: reqwest::Client,
    api_key: String,
}

impl ApiClient {
    pub fn new(api_key: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
        }
    }

    pub async fn validate_credentials(&self) -> Result<(), AppError> {
        let url = format!("{}{}", API_BASE_URL, MODELS_ENDPOINT);
        let response = self.client
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

    pub async fn generate_text(&self, prompt: &str) -> Result<String, AppError> {
        let url = format!("{}{}", API_BASE_URL, CHAT_COMPLETIONS_ENDPOINT);
        let messages = vec![ChatMessage {
            role: "user",
            content: prompt,
        }];
        let request_body = ChatRequest {
            model: CHAT_MODEL,
            messages,
        };

        let response = self.client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&request_body)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(AppError::ApiError(
                response.error_for_status().unwrap_err(),
            ));
        }

        let chat_response: ChatResponse = response.json().await?;

        if let Some(choice) = chat_response.choices.into_iter().next() {
            // Handle potential null content
            Ok(choice.message.content.unwrap_or_default())
        } else {
            Err(AppError::NoChoicesInResponse)
        }
    }

    pub async fn evaluate_summary(
        &self,
        original_text: &str,
        summary_text: &str,
    ) -> Result<String, AppError> {
        let url = format!("{}{}", API_BASE_URL, CHAT_COMPLETIONS_ENDPOINT);
        let prompt_content = format!(
            "以下の『原文』を『要約文』は適切に要約できていますか？ 「はい」か「いいえ」で端的に答えた上で、簡単な解説を加えてください。\n\n# 原文\n{}\n\n# 要約文\n{}",
            original_text, summary_text
        );
        let messages = vec![ChatMessage {
            role: "user",
            content: &prompt_content,
        }];
        let request_body = ChatRequest {
            model: CHAT_MODEL,
            messages,
        };

        let response = self.client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&request_body)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(AppError::ApiError(
                response.error_for_status().unwrap_err(),
            ));
        }

        let chat_response: ChatResponse = response.json().await?;

        if let Some(choice) = chat_response.choices.into_iter().next() {
            Ok(choice.message.content.unwrap_or_default())
        } else {
            Err(AppError::NoChoicesInResponse)
        }
    }
}
