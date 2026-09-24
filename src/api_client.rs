use crate::error::AppError;
use crate::evaluation::build_improvement_prompt;
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

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(AppError::InvalidApiKey);
        }
        response.error_for_status()?;
        Ok(())
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

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(AppError::InvalidApiKey);
        }
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
            "questions": {
                "appropriate": {
                    "type": "noul",
                    "instructions": "原文の主旨を保ち、重要事項を伝え、要約として成立していますか？",
                    "criteria": {
                        "true": "主旨と主要情報を保ち、要約として成立する",
                        "false": "主旨の損失や重大な欠落があり、要約として不適切"
                    }
                },
                "importance": {
                    "type": "score",
                    "instructions": "原文の主要事実・主張をどれだけ取りこぼさず抽出していますか？",
                    "criteria": [
                        "主要情報の多くを欠落", "重要な欠落が多い", "一部不足するが主旨は伝わる",
                        "ほぼ必要な情報を含む", "主要事実・主張を漏れなく含む"
                    ]
                },
                "conciseness": {
                    "type": "score",
                    "instructions": "重要情報を残して冗長な繰り返しや不要な詳細を省けていますか？",
                    "criteria": [
                        "冗長で要約になっていない", "不要な記述が多く重要情報も埋もれる",
                        "一部冗長だが主旨は伝わる", "ほぼ簡潔で不要な記述は少ない",
                        "重要情報を保ち無駄なく簡潔"
                    ]
                },
                "accuracy": {
                    "type": "score",
                    "instructions": "要約の記述が原文と矛盾せず、事実の追加や意味の改変がありませんか？",
                    "criteria": [
                        "重大な矛盾や捏造がある", "重要な誤りや意味の改変がある",
                        "一部不正確だが主旨は概ね正しい", "軽微な不正確さのみ",
                        "矛盾・追加・意味の改変がない"
                    ]
                },
                "overall": {
                    "type": "choice",
                    "instructions": "要約全体は合格ですか？",
                    "criteria": {
                        "pass": "原文の主旨と主要情報を保ち、重大な不正確さがなく要約として読める",
                        "fail": "それ以外"
                    }
                }
            }
        });
        let response = self
            .client
            .post(JEV_ENDPOINT)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(AppError::InvalidApiKey);
        }
        let response = response.error_for_status()?;
        Ok(response.json().await?)
    }
}
