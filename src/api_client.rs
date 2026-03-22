use crate::error::AppError;
use crate::evaluation::build_evaluation_prompt;
use serde::{Deserialize, Serialize};
use std::{future::Future, pin::Pin};

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

type ApiFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait ApiClientLike: Send + Sync {
    fn generate_text(&self, prompt: String) -> ApiFuture<'_, Result<String, AppError>>;
    fn evaluate_summary(
        &self,
        original_text: String,
        summary_text: String,
    ) -> ApiFuture<'_, Result<String, AppError>>;
}

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

impl ApiClientLike for ApiClient {
    fn generate_text(&self, prompt: String) -> ApiFuture<'_, Result<String, AppError>> {
        Box::pin(async move { ApiClient::generate_text(self, &prompt).await })
    }

    fn evaluate_summary(
        &self,
        original_text: String,
        summary_text: String,
    ) -> ApiFuture<'_, Result<String, AppError>> {
        Box::pin(
            async move { ApiClient::evaluate_summary(self, &original_text, &summary_text).await },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evaluation::{OverallEvaluation, parse_evaluation};

    const BROKEN_RESPONSE: &str = "not a valid format";

    struct TestApiClient {
        evaluation: String,
    }

    impl TestApiClient {
        fn new(evaluation: &str) -> Self {
            Self {
                evaluation: evaluation.to_string(),
            }
        }
    }

    impl ApiClientLike for TestApiClient {
        fn generate_text(&self, _prompt: String) -> ApiFuture<'_, Result<String, AppError>> {
            Box::pin(async { Ok("dummy text".to_string()) })
        }

        fn evaluate_summary(
            &self,
            _original_text: String,
            _summary_text: String,
        ) -> ApiFuture<'_, Result<String, AppError>> {
            let evaluation = self.evaluation.clone();
            Box::pin(async move { Ok(evaluation) })
        }
    }

    #[tokio::test]
    async fn evaluation_passes_for_valid_pass_response() {
        let client = TestApiClient::new(
            "- 適切な要約か: はい\n- 重要情報の抽出: 4\n- 簡潔性: 4\n- 正確性: 4\n- 改善点1: なし\n- 改善点2: なし\n- 改善点3: なし\n- 総合評価: 合格\n",
        );
        let evaluation = client
            .evaluate_summary("original".to_string(), "summary".to_string())
            .await
            .expect("evaluation response");
        let parsed = parse_evaluation(&evaluation).expect("parse evaluation");
        assert!(matches!(parsed.overall, OverallEvaluation::Pass));
    }

    #[tokio::test]
    async fn evaluation_fails_for_valid_fail_response() {
        let client = TestApiClient::new(
            "- 適切な要約か: いいえ\n- 重要情報の抽出: 2\n- 簡潔性: 2\n- 正確性: 2\n- 改善点1: 情報不足\n- 改善点2: 要約が長すぎる\n- 改善点3: 原文の主旨を外れている\n- 総合評価: 不合格\n",
        );
        let evaluation = client
            .evaluate_summary("original".to_string(), "summary".to_string())
            .await
            .expect("evaluation response");
        let parsed = parse_evaluation(&evaluation).expect("parse evaluation");
        assert!(matches!(parsed.overall, OverallEvaluation::Fail));
    }

    #[tokio::test]
    async fn evaluation_fails_for_broken_response() {
        let client = TestApiClient::new(BROKEN_RESPONSE);
        let evaluation = client
            .evaluate_summary("original".to_string(), "summary".to_string())
            .await
            .expect("evaluation response");
        assert!(parse_evaluation(&evaluation).is_err());
    }
}
