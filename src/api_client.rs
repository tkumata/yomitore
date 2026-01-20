use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::{future::Future, pin::Pin};

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
            r#"
以下の「原文」と「要約文」を比較し、要約として適切か評価してください。

# 評価ルール
- 出力は必ず以下の「出力フォーマット」のみ使用すること
- 数値は 1〜5 の整数のみ
- 余計な文章や注釈は禁止
- Markdown 記法は禁止

# 出力フォーマット(厳守)
- 適切な要約か: はい/いいえ
- 重要情報の抽出: [1-5]
- 簡潔性: [1-5]
- 正確性: [1-5]
- 改善点1: ...
- 改善点2: ...
- 改善点3: ...
- 総合評価: 合格/不合格

# 採点基準
- 5: 非常に優れている
- 3: 可もなく不可もなく
- 1: 明確な問題がある

# 原文
{}

# 要約文
{}
"#,
            original_text, summary_text
        );

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverallEvaluation {
    Pass,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluationResult {
    pub appropriate: bool,
    pub importance: u8,
    pub conciseness: u8,
    pub accuracy: u8,
    pub improvement1: String,
    pub improvement2: String,
    pub improvement3: String,
    pub overall: OverallEvaluation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseEvaluationError {
    DuplicateField(&'static str),
    MissingField(&'static str),
    InvalidValue(&'static str, String),
}

fn parse_score(field: &'static str, value: &str) -> Result<u8, ParseEvaluationError> {
    let trimmed = value.trim();
    let digits: String = trimmed
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect();
    if digits.is_empty() {
        return Err(ParseEvaluationError::InvalidValue(
            field,
            value.to_string(),
        ));
    }
    let score: u8 = digits
        .parse()
        .map_err(|_| ParseEvaluationError::InvalidValue(field, value.to_string()))?;
    if !(1..=5).contains(&score) {
        return Err(ParseEvaluationError::InvalidValue(field, value.to_string()));
    }
    Ok(score)
}

pub fn parse_evaluation(evaluation: &str) -> Result<EvaluationResult, ParseEvaluationError> {
    let mut appropriate = None;
    let mut importance = None;
    let mut conciseness = None;
    let mut accuracy = None;
    let mut improvement1 = None;
    let mut improvement2 = None;
    let mut improvement3 = None;
    let mut overall = None;

    for line in evaluation.lines() {
        let mut trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some(stripped) = trimmed.strip_prefix('-') {
            trimmed = stripped.trim_start();
        } else if let Some(stripped) = trimmed.strip_prefix('・') {
            trimmed = stripped.trim_start();
        } else if let Some(stripped) = trimmed.strip_prefix('•') {
            trimmed = stripped.trim_start();
        } else if let Some(stripped) = trimmed.strip_prefix('−') {
            trimmed = stripped.trim_start();
        } else if let Some(stripped) = trimmed.strip_prefix('*') {
            trimmed = stripped.trim_start();
        }

        let (key, value) = match trimmed.split_once(':') {
            Some((key, value)) => (key, value),
            None => continue,
        };
        let key = key.trim();
        let value = value.trim();

        match key {
            "適切な要約か" => {
                if appropriate.is_some() {
                    return Err(ParseEvaluationError::DuplicateField("適切な要約か"));
                }
                let parsed = if value.starts_with("はい") {
                    true
                } else if value.starts_with("いいえ") {
                    false
                } else {
                    return Err(ParseEvaluationError::InvalidValue(
                        "適切な要約か",
                        value.to_string(),
                    ));
                };
                appropriate = Some(parsed);
            }
            "重要情報の抽出" => {
                if importance.is_some() {
                    return Err(ParseEvaluationError::DuplicateField("重要情報の抽出"));
                }
                importance = Some(parse_score("重要情報の抽出", value)?);
            }
            "簡潔性" => {
                if conciseness.is_some() {
                    return Err(ParseEvaluationError::DuplicateField("簡潔性"));
                }
                conciseness = Some(parse_score("簡潔性", value)?);
            }
            "正確性" => {
                if accuracy.is_some() {
                    return Err(ParseEvaluationError::DuplicateField("正確性"));
                }
                accuracy = Some(parse_score("正確性", value)?);
            }
            "改善点1" => {
                if improvement1.is_some() {
                    return Err(ParseEvaluationError::DuplicateField("改善点1"));
                }
                improvement1 = Some(value.to_string());
            }
            "改善点2" => {
                if improvement2.is_some() {
                    return Err(ParseEvaluationError::DuplicateField("改善点2"));
                }
                improvement2 = Some(value.to_string());
            }
            "改善点3" => {
                if improvement3.is_some() {
                    return Err(ParseEvaluationError::DuplicateField("改善点3"));
                }
                improvement3 = Some(value.to_string());
            }
            "総合評価" => {
                if overall.is_some() {
                    return Err(ParseEvaluationError::DuplicateField("総合評価"));
                }
                let parsed = if value.starts_with("合格") {
                    OverallEvaluation::Pass
                } else if value.starts_with("不合格") {
                    OverallEvaluation::Fail
                } else {
                    return Err(ParseEvaluationError::InvalidValue(
                        "総合評価",
                        value.to_string(),
                    ));
                };
                overall = Some(parsed);
            }
            _ => continue,
        }
    }

    Ok(EvaluationResult {
        appropriate: appropriate.ok_or(ParseEvaluationError::MissingField("適切な要約か"))?,
        importance: importance.ok_or(ParseEvaluationError::MissingField("重要情報の抽出"))?,
        conciseness: conciseness.ok_or(ParseEvaluationError::MissingField("簡潔性"))?,
        accuracy: accuracy.ok_or(ParseEvaluationError::MissingField("正確性"))?,
        improvement1: improvement1.ok_or(ParseEvaluationError::MissingField("改善点1"))?,
        improvement2: improvement2.ok_or(ParseEvaluationError::MissingField("改善点2"))?,
        improvement3: improvement3.ok_or(ParseEvaluationError::MissingField("改善点3"))?,
        overall: overall.ok_or(ParseEvaluationError::MissingField("総合評価"))?,
    })
}

pub fn format_evaluation_display(parsed: &EvaluationResult) -> String {
    let appropriate = if parsed.appropriate { "はい" } else { "いいえ" };
    let overall = match parsed.overall {
        OverallEvaluation::Pass => "合格",
        OverallEvaluation::Fail => "不合格",
    };

    format!(
        "- 適切な要約か: {}\n- 重要情報の抽出: {}\n- 簡潔性: {}\n- 正確性: {}\n- 改善点1: {}\n- 改善点2: {}\n- 改善点3: {}\n- 総合評価: {}\n",
        appropriate,
        parsed.importance,
        parsed.conciseness,
        parsed.accuracy,
        parsed.improvement1,
        parsed.improvement2,
        parsed.improvement3,
        overall
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const PASS_RESPONSE: &str = r#"- 適切な要約か: はい
- 重要情報の抽出: 4
- 簡潔性: 4
- 正確性: 4
- 改善点1: なし
- 改善点2: なし
- 改善点3: なし
- 総合評価: 合格
"#;

    const FAIL_RESPONSE: &str = r#"- 適切な要約か: いいえ
- 重要情報の抽出: 2
- 簡潔性: 2
- 正確性: 2
- 改善点1: 情報不足
- 改善点2: 要約が長すぎる
- 改善点3: 原文の主旨を外れている
- 総合評価: 不合格
"#;

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
        let client = TestApiClient::new(PASS_RESPONSE);
        let evaluation = client
            .evaluate_summary("original".to_string(), "summary".to_string())
            .await
            .expect("evaluation response");
        let parsed = parse_evaluation(&evaluation).expect("parse evaluation");
        assert!(matches!(parsed.overall, OverallEvaluation::Pass));
    }

    #[tokio::test]
    async fn evaluation_fails_for_valid_fail_response() {
        let client = TestApiClient::new(FAIL_RESPONSE);
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

    #[test]
    fn parse_evaluation_accepts_pass_response() {
        let parsed = parse_evaluation(PASS_RESPONSE).expect("parse evaluation");
        assert!(parsed.appropriate);
        assert_eq!(parsed.importance, 4);
        assert_eq!(parsed.conciseness, 4);
        assert_eq!(parsed.accuracy, 4);
        assert_eq!(parsed.improvement1, "なし");
        assert!(matches!(parsed.overall, OverallEvaluation::Pass));
    }

    #[test]
    fn parse_evaluation_accepts_out_of_order_lines() {
        let response = r#"評価結果:
- 総合評価: 合格 (OK)
- 改善点3: なし
- 正確性: 5/5
- 改善点1: なし
- 簡潔性: 3
- 重要情報の抽出: 2
- 改善点2: なし
- 適切な要約か: はい
"#;
        let parsed = parse_evaluation(response).expect("parse evaluation");
        assert_eq!(parsed.importance, 2);
        assert_eq!(parsed.conciseness, 3);
        assert_eq!(parsed.accuracy, 5);
        assert!(matches!(parsed.overall, OverallEvaluation::Pass));
    }

    #[test]
    fn parse_evaluation_rejects_broken_response() {
        assert!(parse_evaluation(BROKEN_RESPONSE).is_err());
    }

    #[test]
    fn parse_evaluation_rejects_out_of_range_score() {
        let response = PASS_RESPONSE.replace("重要情報の抽出: 4", "重要情報の抽出: 6");
        assert!(parse_evaluation(&response).is_err());
    }
}
