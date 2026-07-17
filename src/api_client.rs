use crate::error::AppError;
use crate::evaluation::build_evaluation_prompt;
use serde::{Deserialize, Serialize};

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

#[derive(Deserialize, Debug)]
struct ChatResponseMessage {
    content: Option<String>,
}

const API_BASE_URL: &str = "https://api.groq.com/openai/v1";
const CHAT_COMPLETIONS_ENDPOINT: &str = "/chat/completions";
const MODELS_ENDPOINT: &str = "/models";
const CHAT_MODEL: &str = "openai/gpt-oss-120b";
const API_TIMEOUT_SECS: u64 = 60;

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
        let url = format!("{API_BASE_URL}{MODELS_ENDPOINT}");
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

    async fn send_chat_request(&self, prompt: &str) -> Result<String, AppError> {
        let url = format!("{API_BASE_URL}{CHAT_COMPLETIONS_ENDPOINT}");
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
            let Err(err) = response.error_for_status() else {
                unreachable!("response status was already checked as unsuccessful");
            };
            return Err(AppError::ApiError(err));
        }

        let chat_response: ChatResponse = response.json().await?;

        if let Some(choice) = chat_response.choices.into_iter().next() {
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
        let prompt_content = build_evaluation_prompt(original_text, summary_text);
        self.send_chat_request(&prompt_content).await
    }
}
