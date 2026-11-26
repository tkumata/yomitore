use crate::error::AppError;
use crate::evaluation::{build_improvement_prompt, jev_questions};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

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
const JEV_ENDPOINT: &str = "https://api.typesafe.ai/v1/systemone";

pub struct ApiClient {
    client: reqwest::Client,
    api_key: String,
}

impl ApiClient {
    pub fn http_client() -> Result<reqwest::Client, AppError> {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(API_TIMEOUT_SECS))
            .build()
            .map_err(|_| AppError::ApiClientInitialization)
    }

    pub fn new(client: &reqwest::Client, api_key: String) -> Self {
        Self {
            client: client.clone(),
            api_key,
        }
    }

    pub async fn validate_credentials(&self) -> Result<(), AppError> {
        let url = format!("{API_BASE_URL}{MODELS_ENDPOINT}");
        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.api_key)
            .send()
            .await?;

        check_status(response.status())
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

        check_status(response.status())?;

        let chat_response: ChatResponse = response.json().await?;

        chat_response.into_content()
    }

    pub async fn generate_text(&self, prompt: &str) -> Result<String, AppError> {
        self.send_chat_request(prompt).await
    }

    pub async fn evaluate_summary(
        &self,
        original_text: &str,
        summary_text: &str,
    ) -> Result<String, AppError> {
        let prompt_content = build_improvement_prompt(original_text, summary_text);
        self.send_chat_request(&prompt_content).await
    }

    pub async fn evaluate_with_jev(
        &self,
        original_text: &str,
        summary_text: &str,
    ) -> Result<Value, AppError> {
        let body = json!({
            "model": "jev-latest",
            "state": {"original": original_text, "summary": summary_text},
            "questions": jev_questions()
        });
        let response = self
            .client
            .post(JEV_ENDPOINT)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await?;
        check_status(response.status())?;
        Ok(response.json().await?)
    }
}

fn check_status(status: reqwest::StatusCode) -> Result<(), AppError> {
    match status {
        reqwest::StatusCode::UNAUTHORIZED => Err(AppError::InvalidApiKey),
        status if status.is_success() => Ok(()),
        status => Err(AppError::HttpStatus(status)),
    }
}

impl ChatResponse {
    fn into_content(self) -> Result<String, AppError> {
        let choice = self
            .choices
            .into_iter()
            .next()
            .ok_or(AppError::NoChoicesInResponse)?;
        choice
            .message
            .content
            .filter(|content| !content.trim().is_empty())
            .ok_or(AppError::EmptyChatContent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::StatusCode;

    #[test]
    fn rejects_empty_chat_content() {
        for content in [Value::Null, json!(""), json!(" \n\t")] {
            let Ok(response) = serde_json::from_value::<ChatResponse>(
                json!({"choices": [{"message": {"content": content}}]}),
            ) else {
                unreachable!("invalid chat response fixture");
            };
            assert!(matches!(
                response.into_content(),
                Err(AppError::EmptyChatContent)
            ));
        }
    }

    #[test]
    fn maps_unsuccessful_http_statuses_to_errors() {
        assert!(check_status(StatusCode::OK).is_ok());
        assert!(matches!(
            check_status(StatusCode::UNAUTHORIZED),
            Err(AppError::InvalidApiKey)
        ));
        for status in [StatusCode::FOUND, StatusCode::BAD_GATEWAY] {
            assert!(
                matches!(check_status(status), Err(AppError::HttpStatus(actual)) if actual == status)
            );
        }
    }
}
