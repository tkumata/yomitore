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

const BULLET_PREFIXES: [char; 5] = ['-', '・', '•', '−', '*'];

pub fn build_evaluation_prompt(original_text: &str, summary_text: &str) -> String {
    format!(
        r"
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
{original_text}

# 要約文
{summary_text}
"
    )
}

pub fn parse_evaluation(evaluation: &str) -> Result<EvaluationResult, ParseEvaluationError> {
    let mut fields = EvaluationFields::default();

    for line in evaluation.lines() {
        let trimmed = strip_bullet_prefix(line.trim());
        if trimmed.is_empty() {
            continue;
        }

        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };

        fields.assign(key.trim(), value.trim())?;
    }

    fields.build()
}

pub fn format_evaluation_display(parsed: &EvaluationResult) -> String {
    let appropriate = if parsed.appropriate {
        "はい"
    } else {
        "いいえ"
    };
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

#[derive(Default)]
struct EvaluationFields {
    appropriate: Option<bool>,
    importance: Option<u8>,
    conciseness: Option<u8>,
    accuracy: Option<u8>,
    improvement1: Option<String>,
    improvement2: Option<String>,
    improvement3: Option<String>,
    overall: Option<OverallEvaluation>,
}

impl EvaluationFields {
    fn assign(&mut self, key: &str, value: &str) -> Result<(), ParseEvaluationError> {
        match key {
            "適切な要約か" => assign_bool(&mut self.appropriate, "適切な要約か", value),
            "重要情報の抽出" => assign_score(&mut self.importance, "重要情報の抽出", value),
            "簡潔性" => assign_score(&mut self.conciseness, "簡潔性", value),
            "正確性" => assign_score(&mut self.accuracy, "正確性", value),
            "改善点1" => assign_text(&mut self.improvement1, "改善点1", value),
            "改善点2" => assign_text(&mut self.improvement2, "改善点2", value),
            "改善点3" => assign_text(&mut self.improvement3, "改善点3", value),
            "総合評価" => assign_overall(&mut self.overall, "総合評価", value),
            _ => Ok(()),
        }
    }

    fn build(self) -> Result<EvaluationResult, ParseEvaluationError> {
        Ok(EvaluationResult {
            appropriate: self
                .appropriate
                .ok_or(ParseEvaluationError::MissingField("適切な要約か"))?,
            importance: self
                .importance
                .ok_or(ParseEvaluationError::MissingField("重要情報の抽出"))?,
            conciseness: self
                .conciseness
                .ok_or(ParseEvaluationError::MissingField("簡潔性"))?,
            accuracy: self
                .accuracy
                .ok_or(ParseEvaluationError::MissingField("正確性"))?,
            improvement1: self
                .improvement1
                .ok_or(ParseEvaluationError::MissingField("改善点1"))?,
            improvement2: self
                .improvement2
                .ok_or(ParseEvaluationError::MissingField("改善点2"))?,
            improvement3: self
                .improvement3
                .ok_or(ParseEvaluationError::MissingField("改善点3"))?,
            overall: self
                .overall
                .ok_or(ParseEvaluationError::MissingField("総合評価"))?,
        })
    }
}

fn strip_bullet_prefix(line: &str) -> &str {
    let mut trimmed = line;
    if let Some(first) = trimmed.chars().next()
        && BULLET_PREFIXES.contains(&first)
    {
        trimmed = &trimmed[first.len_utf8()..];
    }
    trimmed.trim_start()
}

fn assign_bool(
    slot: &mut Option<bool>,
    field: &'static str,
    value: &str,
) -> Result<(), ParseEvaluationError> {
    ensure_empty(slot.as_ref(), field)?;
    *slot = Some(parse_yes_no(field, value)?);
    Ok(())
}

fn assign_score(
    slot: &mut Option<u8>,
    field: &'static str,
    value: &str,
) -> Result<(), ParseEvaluationError> {
    ensure_empty(slot.as_ref(), field)?;
    *slot = Some(parse_score(field, value)?);
    Ok(())
}

fn assign_text(
    slot: &mut Option<String>,
    field: &'static str,
    value: &str,
) -> Result<(), ParseEvaluationError> {
    ensure_empty(slot.as_ref(), field)?;
    *slot = Some(value.to_string());
    Ok(())
}

fn assign_overall(
    slot: &mut Option<OverallEvaluation>,
    field: &'static str,
    value: &str,
) -> Result<(), ParseEvaluationError> {
    ensure_empty(slot.as_ref(), field)?;
    *slot = Some(parse_overall(field, value)?);
    Ok(())
}

fn ensure_empty<T>(slot: Option<&T>, field: &'static str) -> Result<(), ParseEvaluationError> {
    if slot.is_some() {
        Err(ParseEvaluationError::DuplicateField(field))
    } else {
        Ok(())
    }
}

fn parse_yes_no(field: &'static str, value: &str) -> Result<bool, ParseEvaluationError> {
    if value.starts_with("はい") {
        Ok(true)
    } else if value.starts_with("いいえ") {
        Ok(false)
    } else {
        Err(ParseEvaluationError::InvalidValue(field, value.to_string()))
    }
}

fn parse_overall(
    field: &'static str,
    value: &str,
) -> Result<OverallEvaluation, ParseEvaluationError> {
    if value.starts_with("合格") {
        Ok(OverallEvaluation::Pass)
    } else if value.starts_with("不合格") {
        Ok(OverallEvaluation::Fail)
    } else {
        Err(ParseEvaluationError::InvalidValue(field, value.to_string()))
    }
}

fn parse_score(field: &'static str, value: &str) -> Result<u8, ParseEvaluationError> {
    let digits: String = value
        .trim()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    if digits.is_empty() {
        return Err(ParseEvaluationError::InvalidValue(field, value.to_string()));
    }

    let score = digits
        .parse()
        .map_err(|_| ParseEvaluationError::InvalidValue(field, value.to_string()))?;
    if (1..=5).contains(&score) {
        Ok(score)
    } else {
        Err(ParseEvaluationError::InvalidValue(field, value.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PASS_RESPONSE: &str = r"- 適切な要約か: はい
- 重要情報の抽出: 4
- 簡潔性: 4
- 正確性: 4
- 改善点1: なし
- 改善点2: なし
- 改善点3: なし
- 総合評価: 合格
";

    const FAIL_RESPONSE: &str = r"- 適切な要約か: いいえ
- 重要情報の抽出: 2
- 簡潔性: 2
- 正確性: 2
- 改善点1: 情報不足
- 改善点2: 要約が長すぎる
- 改善点3: 原文の主旨を外れている
- 総合評価: 不合格
";

    const BROKEN_RESPONSE: &str = "not a valid format";

    #[test]
    fn parse_evaluation_accepts_pass_response() {
        let parsed = parse_evaluation(PASS_RESPONSE).unwrap_or(EvaluationResult {
            appropriate: false,
            importance: 0,
            conciseness: 0,
            accuracy: 0,
            improvement1: String::new(),
            improvement2: String::new(),
            improvement3: String::new(),
            overall: OverallEvaluation::Fail,
        });
        assert!(parsed.appropriate);
        assert_eq!(parsed.importance, 4);
        assert_eq!(parsed.conciseness, 4);
        assert_eq!(parsed.accuracy, 4);
        assert_eq!(parsed.improvement1, "なし");
        assert!(matches!(parsed.overall, OverallEvaluation::Pass));
    }

    #[test]
    fn parse_evaluation_accepts_out_of_order_lines() {
        let response = r"評価結果:
- 総合評価: 合格 (OK)
- 改善点3: なし
- 正確性: 5/5
- 改善点1: なし
- 簡潔性: 3
- 重要情報の抽出: 2
- 改善点2: なし
- 適切な要約か: はい
";
        let parsed = parse_evaluation(response).unwrap_or(EvaluationResult {
            appropriate: false,
            importance: 0,
            conciseness: 0,
            accuracy: 0,
            improvement1: String::new(),
            improvement2: String::new(),
            improvement3: String::new(),
            overall: OverallEvaluation::Fail,
        });
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

    #[test]
    fn test_parse_score_variations() {
        assert_eq!(parse_score("f", "5").unwrap_or(0), 5);
        assert_eq!(parse_score("f", " 3 ").unwrap_or(0), 3);
        assert_eq!(parse_score("f", "4/5").unwrap_or(0), 4);
        assert_eq!(parse_score("f", "2 (推薦)").unwrap_or(0), 2);

        assert!(parse_score("f", "0").is_err());
        assert!(parse_score("f", "6").is_err());
        assert!(parse_score("f", "abc").is_err());
        assert!(parse_score("f", "あ").is_err());
    }

    #[test]
    fn test_parse_evaluation_bullet_variations() {
        let bullet_types = vec!["-", "・", "•", "*", "−"];
        for bullet in bullet_types {
            let response = format!(
                "{bullet} 適切な要約か: はい\n{bullet} 重要情報の抽出: 4\n{bullet} 簡潔性: 4\n{bullet} 正確性: 4\n{bullet} 改善点1: なし\n{bullet} 改善点2: なし\n{bullet} 改善点3: なし\n{bullet} 総合評価: 合格"
            );
            assert!(
                parse_evaluation(&response).is_ok(),
                "Failed for bullet: {bullet}"
            );
        }
    }

    #[test]
    fn test_parse_evaluation_missing_fields() {
        let response = r"- 適切な要約か: はい
- 簡潔性: 4
- 正確性: 4
- 改善点1: なし
- 改善点2: なし
- 改善点3: なし
- 総合評価: 合格
";
        let result = parse_evaluation(response);
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(ParseEvaluationError::MissingField("重要情報の抽出"))
        ));
    }

    #[test]
    fn test_parse_evaluation_duplicate_fields() {
        let response = PASS_RESPONSE.to_string() + "- 簡潔性: 5\n";
        let result = parse_evaluation(&response);
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(ParseEvaluationError::DuplicateField("簡潔性"))
        ));
    }

    #[test]
    fn test_format_evaluation_display() {
        let result = EvaluationResult {
            appropriate: true,
            importance: 5,
            conciseness: 3,
            accuracy: 4,
            improvement1: "imp1".to_string(),
            improvement2: "imp2".to_string(),
            improvement3: "imp3".to_string(),
            overall: OverallEvaluation::Pass,
        };
        let formatted = format_evaluation_display(&result);
        assert!(formatted.contains("適切な要約か: はい"));
        assert!(formatted.contains("重要情報の抽出: 5"));
        assert!(formatted.contains("簡潔性: 3"));
        assert!(formatted.contains("正確性: 4"));
        assert!(formatted.contains("改善点1: imp1"));
        assert!(formatted.contains("総合評価: 合格"));
    }

    #[test]
    fn build_evaluation_prompt_contains_inputs() {
        let prompt = build_evaluation_prompt("原文", "要約");
        assert!(prompt.contains("# 原文\n原文"));
        assert!(prompt.contains("# 要約文\n要約"));
    }

    #[test]
    fn fail_response_parses_as_fail() {
        let parsed = parse_evaluation(FAIL_RESPONSE).unwrap_or(EvaluationResult {
            appropriate: true,
            importance: 5,
            conciseness: 5,
            accuracy: 5,
            improvement1: String::from("unexpected"),
            improvement2: String::from("unexpected"),
            improvement3: String::from("unexpected"),
            overall: OverallEvaluation::Pass,
        });
        assert!(matches!(parsed.overall, OverallEvaluation::Fail));
    }
}
