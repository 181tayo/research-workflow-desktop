use regex::Regex;
use serde_json::Value;
use strsim::normalized_levenshtein;

use crate::util::text::normalize_token;

use super::normalize::build_spec;
use super::types::{QsfChoice, QsfEmbeddedData, QsfQuestion, QsfSurveySpec};

pub fn parse_qsf_json(raw: &str) -> Result<QsfSurveySpec, String> {
    parse_qsf_json_with_tokens(raw, &[])
}

pub fn parse_qsf_json_with_tokens(
    raw: &str,
    candidate_tokens: &[String],
) -> Result<QsfSurveySpec, String> {
    let root: Value = serde_json::from_str(raw).map_err(|e| format!("Invalid QSF JSON: {e}"))?;
    let survey_name = root
        .pointer("/SurveyEntry/SurveyName")
        .and_then(Value::as_str)
        .unwrap_or("Qualtrics Survey")
        .to_string();

    let elements = root
        .pointer("/SurveyElements")
        .and_then(Value::as_array)
        .ok_or_else(|| "QSF missing SurveyElements array".to_string())?;

    let token_filters = candidate_tokens
        .iter()
        .map(|t| normalize_token(t))
        .filter(|t| !t.is_empty())
        .collect::<Vec<String>>();

    let mut questions: Vec<QsfQuestion> = Vec::new();
    let mut embedded_data_fields: Vec<QsfEmbeddedData> = Vec::new();

    for element in elements {
        match element.get("Element").and_then(Value::as_str).unwrap_or("") {
            "SQ" => {
                if let Some(payload) = element.get("Payload") {
                    if let Some(q) = parse_question(payload, &token_filters) {
                        questions.push(q);
                    }
                }
            }
            "FL" => {
                if let Some(payload) = element.get("Payload") {
                    extract_embedded_data(payload, &mut embedded_data_fields);
                }
            }
            _ => {}
        }
    }

    embedded_data_fields.sort_by(|a, b| a.name.cmp(&b.name));
    embedded_data_fields.dedup_by(|a, b| a.name.eq_ignore_ascii_case(&b.name));

    Ok(build_spec(survey_name, questions, embedded_data_fields))
}

fn parse_question(payload: &Value, token_filters: &[String]) -> Option<QsfQuestion> {
    let qid = payload
        .get("QuestionID")
        .and_then(Value::as_str)
        .unwrap_or("UNKNOWN")
        .to_string();
    let export_tag = payload
        .get("DataExportTag")
        .and_then(Value::as_str)
        .filter(|v| !v.trim().is_empty())
        .unwrap_or(&qid)
        .to_string();
    let question_text = strip_html(
        payload
            .get("QuestionText")
            .and_then(Value::as_str)
            .unwrap_or(""),
    );

    if !token_filters.is_empty() {
        let n_tag = normalize_token(&export_tag);
        let n_text = normalize_token(&question_text);
        let keep = token_filters.iter().any(|token| {
            token_match_score(token, &n_tag) >= 0.55 || token_match_score(token, &n_text) >= 0.55
        });
        if !keep {
            return None;
        }
    }

    let question_type = payload
        .pointer("/QuestionType/Type")
        .and_then(Value::as_str)
        .or_else(|| payload.get("QuestionType").and_then(Value::as_str))
        .unwrap_or("unknown")
        .to_string();

    let mut choices: Vec<QsfChoice> = Vec::new();
    if let Some(choice_obj) = payload.get("Choices").and_then(Value::as_object) {
        for (value, choice) in choice_obj {
            let label = choice
                .get("Display")
                .and_then(Value::as_str)
                .map(strip_html)
                .unwrap_or_else(String::new);
            choices.push(QsfChoice {
                value: value.clone(),
                label,
            });
        }
    }

    Some(QsfQuestion {
        qualtrics_qid: qid,
        export_tag,
        question_text,
        question_type,
        choices,
    })
}

fn extract_embedded_data(node: &Value, out: &mut Vec<QsfEmbeddedData>) {
    if let Some(obj) = node.as_object() {
        if obj.get("Type").and_then(Value::as_str) == Some("EmbeddedData") {
            if let Some(fields) = obj.get("EmbeddedData").and_then(Value::as_array) {
                for field in fields {
                    if let Some(name) = field.get("Field").and_then(Value::as_str) {
                        if name.trim().is_empty() {
                            continue;
                        }
                        let default_value = field
                            .get("Value")
                            .and_then(Value::as_str)
                            .map(|v| v.to_string());
                        out.push(QsfEmbeddedData {
                            name: name.to_string(),
                            default_value,
                        });
                    }
                }
            }
        }

        for value in obj.values() {
            extract_embedded_data(value, out);
        }
    } else if let Some(arr) = node.as_array() {
        for item in arr {
            extract_embedded_data(item, out);
        }
    }
}

fn strip_html(input: &str) -> String {
    let tag_re = Regex::new(r"<[^>]+>").expect("regex");
    let no_tags = tag_re.replace_all(input, " ");
    no_tags
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
        .trim()
        .to_string()
}

fn token_match_score(token: &str, candidate: &str) -> f64 {
    if token.is_empty() || candidate.is_empty() {
        return 0.0;
    }
    let c_token = canonicalize_norm(token);
    let c_candidate = canonicalize_norm(candidate);
    if c_token == c_candidate {
        return 1.0;
    }
    let lev = normalized_levenshtein(&c_token, &c_candidate);
    let overlap = token_overlap(&c_token, &c_candidate);
    let contains = if c_candidate.contains(&c_token) || c_token.contains(&c_candidate) {
        0.1
    } else {
        0.0
    };
    let prefix = token_prefix_boost(&c_token, &c_candidate);
    (0.55 * lev + 0.45 * overlap + contains + prefix).min(1.0)
}

fn token_overlap(a: &str, b: &str) -> f64 {
    let a_set = a
        .split('_')
        .filter(|v| !v.is_empty())
        .collect::<std::collections::BTreeSet<&str>>();
    let b_set = b
        .split('_')
        .filter(|v| !v.is_empty())
        .collect::<std::collections::BTreeSet<&str>>();
    if a_set.is_empty() || b_set.is_empty() {
        return 0.0;
    }
    let inter = a_set.intersection(&b_set).count() as f64;
    let union = a_set.union(&b_set).count() as f64;
    inter / union
}

fn token_prefix_boost(a: &str, b: &str) -> f64 {
    let a_tokens = a
        .split('_')
        .filter(|v| !v.is_empty())
        .collect::<Vec<&str>>();
    let b_tokens = b
        .split('_')
        .filter(|v| !v.is_empty())
        .collect::<Vec<&str>>();
    for at in &a_tokens {
        for bt in &b_tokens {
            if at.len() >= 3 && bt.len() >= 3 && (at.starts_with(bt) || bt.starts_with(at)) {
                return 0.15;
            }
        }
    }
    0.0
}

fn canonicalize_norm(norm: &str) -> String {
    norm.split('_')
        .filter(|t| !t.is_empty())
        .map(canonical_token)
        .collect::<Vec<&str>>()
        .join("_")
}

fn canonical_token(token: &str) -> &str {
    match token {
        "cond" | "condition" | "group" | "assignment" | "arm" | "label" | "lbl" => "condition",
        "info" | "information" => "information",
        "ctrl" | "control" | "covariate" | "covariates" => "control",
        "demo" | "demographic" | "demographics" => "demographic",
        "treat" | "treatment" | "predictor" | "iv" => "predictor",
        "dv" | "outcome" => "outcome",
        _ => token,
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_qsf_json, parse_qsf_json_with_tokens};

    #[test]
    fn parses_sq_and_fl_only_with_embedded_data_defaults() {
        let raw = r#"{
      "SurveyEntry": {"SurveyName": "T"},
      "SurveyElements": [
        {"Element":"SQ","Payload":{"QuestionID":"QID1","DataExportTag":"dv_main","QuestionText":"<b>DV?</b>","QuestionType":{"Type":"MC"},"Choices":{"1":{"Display":"<i>A</i>"}}}},
        {"Element":"FL","Payload":{"Flow":[{"Type":"EmbeddedData","EmbeddedData":[{"Field":"participant_id","Value":""},{"Field":"condition","Value":"treat"}]}]}},
        {"Element":"BL","Payload":{"Flow":[{"Type":"EmbeddedData","EmbeddedData":[{"Field":"ignored"}]}]}}
      ]
    }"#;
        let spec = parse_qsf_json(raw).expect("parse qsf");
        assert_eq!(spec.questions.len(), 1);
        assert_eq!(spec.questions[0].question_text, "DV?");
        assert!(spec.expected_columns.iter().any(|c| c == "dv_main"));
        assert!(spec.expected_columns.iter().any(|c| c == "participant_id"));
        assert!(spec
            .embedded_data_fields
            .iter()
            .any(|f| f.name == "condition" && f.default_value.as_deref() == Some("treat")));
        assert!(!spec.embedded_data.iter().any(|e| e == "ignored"));
    }

    #[test]
    fn targeted_mode_keeps_matching_questions_only() {
        let raw = r#"{
      "SurveyEntry": {"SurveyName": "T"},
      "SurveyElements": [
        {"Element":"SQ","Payload":{"QuestionID":"QID1","DataExportTag":"advice_choice","QuestionText":"Advice choice","QuestionType":{"Type":"MC"}}},
        {"Element":"SQ","Payload":{"QuestionID":"QID2","DataExportTag":"age","QuestionText":"Your age","QuestionType":{"Type":"TE"}}}
      ]
    }"#;
        let tokens = vec!["advice".to_string()];
        let spec = parse_qsf_json_with_tokens(raw, &tokens).expect("parse qsf targeted");
        assert_eq!(spec.questions.len(), 1);
        assert_eq!(spec.questions[0].export_tag, "advice_choice");
    }

    #[test]
    fn targeted_mode_keeps_condition_label_and_info_aliases() {
        let raw = r#"{
      "SurveyEntry": {"SurveyName": "T"},
      "SurveyElements": [
        {"Element":"SQ","Payload":{"QuestionID":"QID1","DataExportTag":"income_label","QuestionText":"Income label","QuestionType":{"Type":"MC"}}},
        {"Element":"SQ","Payload":{"QuestionID":"QID2","DataExportTag":"info","QuestionText":"Information condition","QuestionType":{"Type":"MC"}}},
        {"Element":"SQ","Payload":{"QuestionID":"QID3","DataExportTag":"unrelated_var","QuestionText":"Other","QuestionType":{"Type":"TE"}}}
      ]
    }"#;
        let tokens = vec![
            "income_condition".to_string(),
            "information_condition".to_string(),
        ];
        let spec = parse_qsf_json_with_tokens(raw, &tokens).expect("parse qsf targeted");
        let tags = spec
            .questions
            .iter()
            .map(|q| q.export_tag.clone())
            .collect::<Vec<String>>();
        assert!(tags.iter().any(|t| t == "income_label"));
        assert!(tags.iter().any(|t| t == "info"));
        assert!(!tags.iter().any(|t| t == "unrelated_var"));
    }
}
