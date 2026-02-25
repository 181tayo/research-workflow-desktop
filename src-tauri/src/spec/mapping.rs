use strsim::normalized_levenshtein;

use crate::qsf::types::QsfSurveySpec;
use crate::util::text::normalize_token;

use super::types::{MappingCandidate, MappingResult, WarningItem};

const RESOLVE_THRESHOLD: f64 = 0.95;
const CANDIDATE_MIN_SCORE: f64 = 0.75;

pub fn map_variable(prereg_var: &str, qsf: &QsfSurveySpec) -> MappingResult {
    let all_candidates = build_candidates(prereg_var, qsf);
    let mut resolved = all_candidates
        .iter()
        .find(|c| c.score >= RESOLVE_THRESHOLD)
        .map(|c| c.key.clone());
    if resolved.is_none() && has_counterbalanced_pair(prereg_var, &all_candidates) {
        // Auto-resolve to a derived variable keyed by prereg variable name.
        resolved = Some(prereg_var.to_string());
    }
    let mut candidates = all_candidates
        .iter()
        .filter(|c| c.score >= CANDIDATE_MIN_SCORE)
        .cloned()
        .collect::<Vec<MappingCandidate>>();
    if candidates.is_empty() {
        if let Some(best) = all_candidates.first() {
            candidates.push(best.clone());
        }
    }

    MappingResult {
        prereg_var: prereg_var.to_string(),
        resolved_to: resolved,
        candidates: candidates.into_iter().take(5).collect(),
    }
}

pub fn unresolved_warning(mapping: &MappingResult) -> Option<WarningItem> {
    if mapping.resolved_to.is_some() {
        return None;
    }
    Some(WarningItem {
        code: "UNRESOLVED_VARIABLE".to_string(),
        message: format!(
            "Unable to map prereg variable '{}' to QSF column.",
            mapping.prereg_var
        ),
        details: serde_json::json!({
          "preregVar": mapping.prereg_var,
          "candidates": mapping.candidates,
        }),
    })
}

fn build_candidates(prereg_var: &str, qsf: &QsfSurveySpec) -> Vec<MappingCandidate> {
    let n_prereg = normalize_token(prereg_var);

    // Score each stable output column (export_tag / embedded data), using aliases
    // (QID + question text) only for matching, never as returned keys.
    let mut out: Vec<MappingCandidate> = Vec::new();
    for q in &qsf.questions {
        let aliases = vec![
            q.export_tag.clone(),
            q.qualtrics_qid.clone(),
            q.question_text.clone(),
        ];
        let score = best_alias_score(prereg_var, &n_prereg, &aliases);
        out.push(MappingCandidate {
            key: q.export_tag.clone(),
            score,
        });
    }
    for ed in &qsf.embedded_data {
        let aliases = vec![ed.clone()];
        let score = best_alias_score(prereg_var, &n_prereg, &aliases);
        out.push(MappingCandidate {
            key: ed.clone(),
            score,
        });
    }

    // Keep highest score per key.
    let mut deduped: Vec<MappingCandidate> = Vec::new();
    for c in out {
        if let Some(existing) = deduped.iter_mut().find(|x| x.key == c.key) {
            if c.score > existing.score {
                existing.score = c.score;
            }
        } else {
            deduped.push(c);
        }
    }

    deduped.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    deduped
}

fn best_alias_score(prereg_var: &str, n_prereg: &str, aliases: &[String]) -> f64 {
    let c_prereg = canonicalize_norm(n_prereg);
    let mut best = 0.0_f64;
    for alias in aliases {
        let score = if alias.eq_ignore_ascii_case(prereg_var) {
            1.0
        } else {
            let n_alias = normalize_token(alias);
            let c_alias = canonicalize_norm(&n_alias);
            if c_alias == c_prereg {
                0.99
            } else {
                let lev = normalized_levenshtein(&c_alias, &c_prereg);
                let overlap = token_overlap(&c_alias, &c_prereg);
                let contains_boost = if c_alias.contains(&c_prereg) || c_prereg.contains(&c_alias) {
                    0.1
                } else {
                    0.0
                };
                let prefix_boost = token_prefix_boost(&c_alias, &c_prereg);
                (0.55 * lev + 0.45 * overlap + contains_boost + prefix_boost).min(1.0)
            }
        };
        if score > best {
            best = score;
        }
    }
    best
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

fn has_counterbalanced_pair(prereg_var: &str, candidates: &[MappingCandidate]) -> bool {
    if candidates.len() < 2 {
        return false;
    }
    let prereg_norm = normalize_token(prereg_var);
    let top = candidates
        .iter()
        .filter(|c| c.score >= CANDIDATE_MIN_SCORE)
        .take(4)
        .collect::<Vec<&MappingCandidate>>();
    if top.len() < 2 {
        return false;
    }

    for i in 0..top.len() {
        for j in (i + 1)..top.len() {
            let a = top[i];
            let b = top[j];
            if (a.score - b.score).abs() > 0.08 {
                continue;
            }
            let a_norm = normalize_token(&a.key);
            let b_norm = normalize_token(&b.key);
            let a_base = strip_order_suffix(&a_norm);
            let b_base = strip_order_suffix(&b_norm);
            if a_base.is_empty() || b_base.is_empty() || a_base != b_base {
                continue;
            }
            if a_base == prereg_norm
                || a_base.contains(&prereg_norm)
                || prereg_norm.contains(&a_base)
            {
                return true;
            }
        }
    }
    false
}

fn strip_order_suffix(value: &str) -> String {
    let re = regex::Regex::new(r"(?i)(?:_)?[ab]\d+$").expect("regex");
    re.replace(value, "").to_string()
}

#[cfg(test)]
mod tests {
    use super::map_variable;
    use crate::qsf::types::{QsfChoice, QsfEmbeddedData, QsfQuestion, QsfSurveySpec};
    use std::collections::HashMap;

    #[test]
    fn maps_condition_to_label_candidate() {
        let qsf = QsfSurveySpec {
            survey_name: "S".to_string(),
            questions: vec![QsfQuestion {
                qualtrics_qid: "QID1".to_string(),
                export_tag: "income_label".to_string(),
                question_text: "Income condition".to_string(),
                question_type: "MC".to_string(),
                choices: vec![QsfChoice {
                    value: "1".to_string(),
                    label: "Low".to_string(),
                }],
            }],
            embedded_data: vec![],
            embedded_data_fields: vec![QsfEmbeddedData {
                name: "participant_id".to_string(),
                default_value: None,
            }],
            expected_columns: vec!["income_label".to_string(), "participant_id".to_string()],
            label_map: HashMap::new(),
        };
        let result = map_variable("income_condition", &qsf);
        assert!(result.candidates.iter().any(|c| c.key == "income_label"));
    }
}
