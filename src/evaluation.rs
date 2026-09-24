use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverallEvaluation {
    Pass,
    Fail,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JevEvaluation {
    pub appropriate: bool,
    pub importance: f64,
    pub conciseness: f64,
    pub accuracy: f64,
    pub overall: OverallEvaluation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseEvaluationError {
    MissingField(&'static str),
    DuplicateField(&'static str),
    InvalidValue(&'static str, String),
}

impl JevEvaluation {
    pub fn displayed_score(score: f64) -> u8 {
        if score < 0.5 {
            1
        } else if score < 1.5 {
            2
        } else if score < 2.5 {
            3
        } else if score < 3.5 {
            4
        } else {
            5
        }
    }

    pub fn total_score(&self) -> u8 {
        let rounded = (100.0 * (self.importance + self.conciseness + self.accuracy) / 12.0).round();
        (0_u8..=100)
            .find(|score| f64::from(*score) >= rounded)
            .unwrap_or(100)
    }
}

pub fn build_improvement_prompt(original_text: &str, summary_text: &str) -> String {
    format!(
        "原文と要約を比較し、改善点を次の形式で3件だけ出してください。空の項目や説明を追加せず、各項目を1行にしてください。\n- 改善点1: ...\n- 改善点2: ...\n- 改善点3: ...\n評価点や合否は書かないでください。\n\n# 原文\n{original_text}\n\n# 要約\n{summary_text}"
    )
}

pub fn parse_improvements(response: &str) -> Result<[String; 3], ParseEvaluationError> {
    let mut first = None;
    let mut second = None;
    let mut third = None;
    for line in response
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let line = line
            .trim_start_matches(&['-', '・', '•', '−', '*'][..])
            .trim();
        let Some((label, value)) = line.split_once(':') else {
            continue;
        };
        let (field, slot) = match label.trim() {
            "改善点1" => ("改善点1", &mut first),
            "改善点2" => ("改善点2", &mut second),
            "改善点3" => ("改善点3", &mut third),
            _ => continue,
        };
        let value = value.trim();
        if value.is_empty() {
            return Err(ParseEvaluationError::InvalidValue(
                field,
                "空欄".to_string(),
            ));
        }
        if slot.is_some() {
            return Err(ParseEvaluationError::DuplicateField(field));
        }
        *slot = Some(value.to_string());
    }
    Ok([
        first.ok_or(ParseEvaluationError::MissingField("改善点1"))?,
        second.ok_or(ParseEvaluationError::MissingField("改善点2"))?,
        third.ok_or(ParseEvaluationError::MissingField("改善点3"))?,
    ])
}

pub fn format_jev_evaluation_display(parsed: &JevEvaluation, improvements: &[String; 3]) -> String {
    let [first, second, third] = improvements;
    format!(
        "- 適切な要約か: {}\n- 重要情報の抽出: {}\n- 簡潔性: {}\n- 正確性: {}\n- 改善点1: {}\n- 改善点2: {}\n- 改善点3: {}\n- 総合得点: {}点\n- 総合評価: {}\n",
        if parsed.appropriate {
            "はい"
        } else {
            "いいえ"
        },
        JevEvaluation::displayed_score(parsed.importance),
        JevEvaluation::displayed_score(parsed.conciseness),
        JevEvaluation::displayed_score(parsed.accuracy),
        first,
        second,
        third,
        parsed.total_score(),
        if parsed.overall == OverallEvaluation::Pass {
            "合格"
        } else {
            "不合格"
        },
    )
}

pub fn parse_jev_response(response: &Value) -> Result<JevEvaluation, ParseEvaluationError> {
    let answers = response
        .get("answers")
        .and_then(Value::as_object)
        .ok_or(ParseEvaluationError::MissingField("answers"))?;
    let answer = |name: &'static str| {
        answers
            .get(name)
            .ok_or(ParseEvaluationError::MissingField(name))
    };

    let noul = answer("appropriate")?;
    if noul.get("type").and_then(Value::as_str) != Some("noul") {
        return Err(invalid("appropriate", "type"));
    }
    let appropriate_value = noul
        .get("noul")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && (0.0..=1.0).contains(value))
        .ok_or(invalid("appropriate", "noul"))?;

    let score = |name: &'static str| -> Result<f64, ParseEvaluationError> {
        let value = answer(name)?;
        if value.get("type").and_then(Value::as_str) != Some("score") {
            return Err(invalid(name, "type"));
        }
        validate_confidence(value, name)?;
        let levels = ["0", "1", "2", "3", "4"];
        if value
            .get("legend")
            .and_then(Value::as_object)
            .is_none_or(|legend| {
                legend.len() != levels.len()
                    || levels
                        .iter()
                        .any(|level| legend.get(*level).and_then(Value::as_str).is_none())
            })
            || !valid_probability_map(value.get("probabilities"), &levels)
        {
            return Err(invalid(name, "probabilities"));
        }
        value
            .get("score")
            .and_then(Value::as_f64)
            .filter(|score| score.is_finite() && (0.0..=4.0).contains(score))
            .ok_or(invalid(name, "score"))
    };

    let overall = answer("overall")?;
    if overall.get("type").and_then(Value::as_str) != Some("choice") {
        return Err(invalid("overall", "type"));
    }
    validate_confidence(overall, "overall")?;
    if !valid_probability_map(overall.get("probabilities"), &["pass", "fail"]) {
        return Err(invalid("overall", "probabilities"));
    }
    let overall = match overall.get("choice").and_then(Value::as_str) {
        Some("pass") => OverallEvaluation::Pass,
        Some("fail") => OverallEvaluation::Fail,
        _ => return Err(invalid("overall", "choice")),
    };

    Ok(JevEvaluation {
        appropriate: appropriate_value >= 0.5,
        importance: score("importance")?,
        conciseness: score("conciseness")?,
        accuracy: score("accuracy")?,
        overall,
    })
}

fn validate_confidence(value: &Value, field: &'static str) -> Result<(), ParseEvaluationError> {
    if value
        .get("confidence")
        .and_then(Value::as_f64)
        .is_some_and(|confidence| confidence.is_finite() && (0.0..=1.0).contains(&confidence))
    {
        Ok(())
    } else {
        Err(invalid(field, "confidence"))
    }
}

fn valid_probability_map(value: Option<&Value>, expected: &[&str]) -> bool {
    let Some(probabilities) = value.and_then(Value::as_object) else {
        return false;
    };
    let total = expected.iter().try_fold(0.0, |sum, key| {
        let probability = probabilities.get(*key)?.as_f64()?;
        (probability.is_finite() && (0.0..=1.0).contains(&probability)).then_some(sum + probability)
    });
    probabilities.len() == expected.len() && total.is_some_and(|sum: f64| (sum - 1.0).abs() < 0.01)
}

fn invalid(field: &'static str, part: &str) -> ParseEvaluationError {
    ParseEvaluationError::InvalidValue(field, part.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn response() -> Value {
        json!({"answers": {
            "appropriate": {"type":"noul", "noul":0.5},
            "importance": {"type":"score", "score":0.5, "legend":{"0":"a","1":"b","2":"c","3":"d","4":"e"}, "probabilities":{"0":0.5,"1":0.5,"2":0.0,"3":0.0,"4":0.0}, "confidence":0.8},
            "conciseness": {"type":"score", "score":2.0, "legend":{"0":"a","1":"b","2":"c","3":"d","4":"e"}, "probabilities":{"0":0.0,"1":0.0,"2":1.0,"3":0.0,"4":0.0}, "confidence":0.9},
            "accuracy": {"type":"score", "score":4.0, "legend":{"0":"a","1":"b","2":"c","3":"d","4":"e"}, "probabilities":{"0":0.0,"1":0.0,"2":0.0,"3":0.0,"4":1.0}, "confidence":1.0},
            "overall": {"type":"choice", "choice":"pass", "probabilities":{"pass":0.8,"fail":0.2}, "confidence":0.8}
        }})
    }

    #[test]
    fn parses_answers_and_calculates_scores() {
        let parsed = parse_jev_response(&response()).map(|parsed| {
            (
                parsed.appropriate,
                JevEvaluation::displayed_score(parsed.importance),
                parsed.total_score(),
                parsed.overall,
            )
        });
        assert_eq!(parsed, Ok((true, 2, 54, OverallEvaluation::Pass)));
    }

    #[test]
    fn total_score_covers_zero_and_hundred_boundaries() {
        let evaluation = |score| JevEvaluation {
            appropriate: true,
            importance: score,
            conciseness: score,
            accuracy: score,
            overall: OverallEvaluation::Pass,
        };

        assert_eq!(evaluation(0.0).total_score(), 0);
        assert_eq!(evaluation(4.0).total_score(), 100);
    }

    #[test]
    fn rejects_missing_and_out_of_range_answers() {
        let mut missing = response();
        let _ = missing
            .get_mut("answers")
            .and_then(Value::as_object_mut)
            .map(|answers| answers.remove("accuracy"));
        assert!(parse_jev_response(&missing).is_err());

        let mut out_of_range = response();
        if let Some(score) = out_of_range
            .get_mut("answers")
            .and_then(|answers| answers.get_mut("accuracy"))
            .and_then(|accuracy| accuracy.get_mut("score"))
        {
            *score = json!(4.1);
        }
        assert!(parse_jev_response(&out_of_range).is_err());
    }

    #[test]
    fn parses_only_three_labeled_improvements() {
        let parsed = parse_improvements("- 改善点1: first\n- 改善点2: second\n- 改善点3: third");
        assert_eq!(
            parsed,
            Ok(["first".into(), "second".into(), "third".into()])
        );
        assert!(parse_improvements("改善点1: a\n改善点3: c").is_err());
        assert!(parse_improvements("改善点1: a\n改善点1: b\n改善点2: c\n改善点3: d").is_err());
        assert_eq!(
            parse_improvements("ignored line\n改善点1: a\nextra: d\n改善点2: b\n改善点3: c"),
            Ok(["a".into(), "b".into(), "c".into()])
        );
    }

    #[test]
    fn accepts_legacy_bullets_and_ignores_unknown_empty_labels() {
        for bullet in ['-', '・', '•', '−', '*'] {
            let response =
                format!("{bullet} 改善点1: a\n{bullet} 改善点2: b\n{bullet} 改善点3: c\nunknown:");
            assert_eq!(
                parse_improvements(&response),
                Ok(["a".into(), "b".into(), "c".into()])
            );
        }
        assert!(matches!(
            parse_improvements("改善点1: \n改善点2: b\n改善点3: c"),
            Err(ParseEvaluationError::InvalidValue("改善点1", _))
        ));
    }
}
