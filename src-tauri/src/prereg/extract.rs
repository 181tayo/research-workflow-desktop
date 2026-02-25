use regex::Regex;

use crate::util::text::tokenize_identifiers;

use super::types::{AnalysisModelSpec, DerivedScale, ExclusionRule, PreregSpec};

pub fn fill_from_text(spec: &mut PreregSpec, text: &str) {
    if spec.variables.dv.is_empty() {
        spec.variables.dv =
            extract_list_after_markers(text, &["dv", "dependent variable", "dependent variables"]);
    }
    if spec.variables.iv.is_empty() {
        spec.variables.iv = extract_list_after_markers(
            text,
            &["iv", "independent variable", "independent variables"],
        );
    }
    if spec.variables.controls.is_empty() {
        spec.variables.controls = extract_list_after_markers(text, &["controls", "covariates"]);
    }

    if spec.variables.dv.is_empty() {
        spec.variables.dv = extract_concepts_after_markers(
            text,
            &[
                "dependent variable",
                "dependent variables",
                "outcome variable",
                "outcome variables",
                "primary outcome",
                "primary outcomes",
            ],
        );
    }
    if spec.variables.iv.is_empty() {
        spec.variables.iv = extract_concepts_after_markers(
            text,
            &[
                "independent variable",
                "independent variables",
                "predictor",
                "predictors",
                "treatment",
                "treatment condition",
                "condition",
                "manipulation",
            ],
        );
    }
    if spec.variables.controls.is_empty() {
        spec.variables.controls = extract_concepts_after_markers(
            text,
            &[
                "control variables",
                "controls",
                "covariates",
                "adjustment variables",
            ],
        );
    }

    if spec.variables.dv.is_empty() || spec.variables.iv.is_empty() {
        spec.warnings
            .push("VARIABLES_UNCLEAR_IN_PREREG".to_string());
    }

    let models = extract_model_specs(text);
    if !models.is_empty() {
        spec.main_analyses = models;
        if spec.variables.dv.is_empty() {
            let mut dvs = spec
                .main_analyses
                .iter()
                .map(|m| m.dv.clone())
                .collect::<Vec<String>>();
            dvs.sort();
            dvs.dedup();
            spec.variables.dv = dvs;
        }
        if spec.variables.iv.is_empty() {
            let mut ivs = spec
                .main_analyses
                .iter()
                .flat_map(|m| m.iv.clone())
                .collect::<Vec<String>>();
            ivs.sort();
            ivs.dedup();
            spec.variables.iv = ivs;
        }
        if spec.variables.controls.is_empty() {
            let mut ctrls = spec
                .main_analyses
                .iter()
                .flat_map(|m| m.controls.clone())
                .collect::<Vec<String>>();
            ctrls.sort();
            ctrls.dedup();
            spec.variables.controls = ctrls;
        }
    } else if !spec.variables.dv.is_empty() && !spec.variables.iv.is_empty() {
        spec.main_analyses.push(AnalysisModelSpec {
            id: "main_1".to_string(),
            dv: spec.variables.dv[0].clone(),
            iv: spec.variables.iv.clone(),
            controls: spec.variables.controls.clone(),
            interaction_terms: Vec::new(),
            formula: Some(format!(
                "{} ~ {}",
                spec.variables.dv[0],
                spec.variables.iv.join(" + ")
            )),
        });
    }

    spec.exclusion_rules = extract_exclusions(text);
    spec.derived_scales = extract_scales(text);
    spec.robustness_checks = extract_robustness(text);
    spec.missing_data_plan = extract_missing_data_plan(text);

    if spec.main_analyses.is_empty() {
        spec.warnings.push("NO_MAIN_ANALYSIS_EXTRACTED".to_string());
    }
}

pub fn extract_variable_tokens(text: &str) -> Vec<String> {
    tokenize_identifiers(text)
        .into_iter()
        .filter(|t| plausible_variable_token(t))
        .collect()
}

pub fn extract_list_after_markers(text: &str, markers: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    let heading_re =
        Regex::new(r"(?im)^\s*(\d+\)|#+\s+|[A-Za-z][A-Za-z \t]{0,60}:)\s*$").expect("regex");
    for marker in markers {
        let pattern = format!(r"(?im){}\s*[:\-]\s*([^\n\.]+)", regex::escape(marker));
        let re = Regex::new(&pattern).expect("regex");
        for cap in re.captures_iter(text) {
            let line = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            for item in line.split(&[',', ';'][..]) {
                let raw = item.trim();
                let explicit_backtick = Regex::new(r"`([A-Za-z][A-Za-z0-9_]*)`").expect("regex");
                for explicit in explicit_backtick.captures_iter(raw) {
                    let token = explicit[1].to_string();
                    if plausible_variable_token(&token) && !out.iter().any(|v| v == &token) {
                        out.push(token);
                    }
                }
                let tokenized = extract_variable_tokens(raw);
                if !tokenized.is_empty() {
                    for token in tokenized {
                        if !out.iter().any(|v| v == &token) {
                            out.push(token);
                        }
                    }
                } else {
                    let single = raw.trim_matches('`').to_string();
                    if plausible_variable_token(&single) && !out.iter().any(|v| v == &single) {
                        out.push(single);
                    }
                }
            }
        }

        // Capture block-style lists under marker headings, e.g.:
        // "Dependent variables" then bullet/list lines.
        let marker_heading =
            Regex::new(&format!(r"(?im)^\s*{}\s*:?\s*$", regex::escape(marker))).expect("regex");
        let lines: Vec<&str> = text.lines().collect();
        let mut i = 0usize;
        while i < lines.len() {
            if marker_heading.is_match(lines[i]) {
                i += 1;
                while i < lines.len() {
                    let raw = lines[i].trim();
                    if raw.is_empty() || heading_re.is_match(raw) {
                        break;
                    }
                    let stripped = raw
                        .trim_start_matches('-')
                        .trim_start_matches('*')
                        .trim_start_matches('•')
                        .trim()
                        .to_string();
                    for token in extract_variable_tokens(&stripped) {
                        if !out.iter().any(|v| v == &token) {
                            out.push(token);
                        }
                    }
                    i += 1;
                }
            } else {
                i += 1;
            }
        }
    }
    out
}

pub fn extract_model_specs(text: &str) -> Vec<AnalysisModelSpec> {
    let formula_re = Regex::new(r"([A-Za-z][A-Za-z0-9_]*)\s*~\s*([^\n\r]+)").expect("regex");
    let regress_re = Regex::new(
    r"(?im)(?:regress|predict|model)\s+([A-Za-z][A-Za-z0-9_ ]{1,80})\s+(?:on|from|using)\s+([A-Za-z][A-Za-z0-9_, +*:\- ]{1,200})"
  ).expect("regex");
    let mut out = Vec::new();
    for (idx, cap) in formula_re.captures_iter(text).enumerate() {
        let dv = cap[1].trim().to_string();
        if !plausible_variable_token(&dv) {
            continue;
        }
        let rhs = cap[2].trim().to_string();
        let (iv, controls, interactions) = parse_rhs_predictors(&rhs);
        if iv.is_empty() {
            continue;
        }
        out.push(AnalysisModelSpec {
            id: format!("main_{}", idx + 1),
            dv,
            iv,
            controls,
            interaction_terms: interactions,
            formula: Some(format!("{} ~ {}", cap[1].trim(), rhs)),
        });
    }

    if out.is_empty() {
        for (idx, cap) in regress_re.captures_iter(text).enumerate() {
            let dv_tokens = extract_variable_tokens(cap[1].trim());
            if dv_tokens.is_empty() {
                continue;
            }
            let dv = dv_tokens[0].clone();
            let rhs = cap[2].trim().to_string();
            let (iv, controls, interactions) = parse_rhs_predictors(&rhs);
            if iv.is_empty() {
                continue;
            }
            out.push(AnalysisModelSpec {
                id: format!("main_{}", idx + 1),
                dv: dv.clone(),
                iv: iv.clone(),
                controls: controls.clone(),
                interaction_terms: interactions.clone(),
                formula: Some(format!(
                    "{} ~ {}",
                    dv,
                    iv.iter()
                        .chain(controls.iter())
                        .cloned()
                        .collect::<Vec<String>>()
                        .join(" + ")
                )),
            });
        }
    }
    out
}

pub fn extract_exclusions(text: &str) -> Vec<ExclusionRule> {
    let re = Regex::new(r"(?im)(exclude|remove|drop)\s+([^\n\.]+)").expect("regex");
    let mut out = Vec::new();
    for (idx, cap) in re.captures_iter(text).enumerate() {
        out.push(ExclusionRule {
            id: format!("exclusion_{}", idx + 1),
            rule_type: "filter".to_string(),
            variable: None,
            criterion: cap[2].trim().to_string(),
        });
    }
    out
}

pub fn extract_scales(text: &str) -> Vec<DerivedScale> {
    let re = Regex::new(r"(?im)(\d+)-item\s+([A-Za-z][A-Za-z0-9_]*)").expect("regex");
    let text_re =
    Regex::new(r"(?im)([A-Za-z][A-Za-z0-9 \-]{3,80})\s*\((four|five|six|seven|eight|nine|ten|\d+)\s+items?\)")
      .expect("regex");
    let mut out = Vec::new();
    for cap in re.captures_iter(text) {
        let name = cap[2].to_string();
        out.push(DerivedScale {
            name: format!("{}_scale", name),
            derived_type: "scale".to_string(),
            depends_on: Vec::new(),
            definition: format!("rowMeans(cbind(/* items for {} */), na.rm = TRUE)", name),
        });
    }
    for cap in text_re.captures_iter(text) {
        let raw_name = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let name = normalize_concept_phrase(raw_name);
        if name.is_empty()
            || out
                .iter()
                .any(|s: &DerivedScale| s.name == format!("{}_scale", name))
        {
            continue;
        }
        out.push(DerivedScale {
            name: format!("{}_scale", name),
            derived_type: "scale".to_string(),
            depends_on: Vec::new(),
            definition: format!("rowMeans(cbind(/* items for {} */), na.rm = TRUE)", name),
        });
    }
    out
}

pub fn extract_robustness(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let lc = text.to_lowercase();
    if lc.contains("with and without controls") {
        out.push("with_without_controls".to_string());
    }
    if (lc.contains("without any control variables")
        && lc.contains("controlling for participant demographics"))
        || (lc.contains("without control variables") && lc.contains("controlling for"))
    {
        out.push("with_without_controls".to_string());
    }
    if lc.contains("robust") || lc.contains("sensitivity") {
        out.push("sensitivity_checks".to_string());
    }
    out
}

fn extract_missing_data_plan(text: &str) -> Option<String> {
    let re = Regex::new(r"(?im)(missing data|missingness)\s*[:\-]\s*([^\n]+)").expect("regex");
    re.captures(text)
        .and_then(|cap| cap.get(2).map(|m| m.as_str().trim().to_string()))
}

fn extract_concepts_after_markers(text: &str, markers: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let heading_re =
        Regex::new(r"(?im)^\s*(\d+\)|#+\s+|[A-Za-z][A-Za-z \t]{0,60}:)\s*$").expect("regex");
    let lines: Vec<&str> = text.lines().collect();
    for marker in markers {
        let inline = Regex::new(&format!(
            r"(?im){}\s*[:\-]\s*([^\n]+)",
            regex::escape(marker)
        ))
        .expect("regex");
        for cap in inline.captures_iter(text) {
            if let Some(m) = cap.get(1) {
                for item in split_candidates(m.as_str()) {
                    let normalized = normalize_concept_phrase(&item);
                    if !normalized.is_empty()
                        && !out.iter().any(|v| v.eq_ignore_ascii_case(&normalized))
                    {
                        out.push(normalized);
                    }
                }
            }
        }

        let marker_heading =
            Regex::new(&format!(r"(?im)^\s*{}\s*:?\s*$", regex::escape(marker))).expect("regex");
        let mut i = 0usize;
        while i < lines.len() {
            if marker_heading.is_match(lines[i]) {
                i += 1;
                while i < lines.len() {
                    let raw = lines[i].trim();
                    if raw.is_empty() || heading_re.is_match(raw) {
                        break;
                    }
                    let stripped = raw
                        .trim_start_matches('-')
                        .trim_start_matches('*')
                        .trim_start_matches('•')
                        .trim();
                    for item in split_candidates(stripped) {
                        let normalized = normalize_concept_phrase(&item);
                        if !normalized.is_empty()
                            && !out.iter().any(|v| v.eq_ignore_ascii_case(&normalized))
                        {
                            out.push(normalized);
                        }
                    }
                    i += 1;
                }
            } else {
                i += 1;
            }
        }
    }
    out
}

fn split_candidates(raw: &str) -> Vec<String> {
    raw.split(&[',', ';', '\n'][..])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn parse_rhs_predictors(rhs: &str) -> (Vec<String>, Vec<String>, Vec<String>) {
    let coef_re = Regex::new(r"(?i)\b(?:b|beta)\d*\b").expect("regex");
    let cleaned_rhs = coef_re.replace_all(rhs, "").to_string();
    let mut iv: Vec<String> = Vec::new();
    let mut controls: Vec<String> = Vec::new();
    let mut interactions: Vec<String> = Vec::new();

    for term in cleaned_rhs.split('+') {
        let clean = term
            .trim()
            .trim_matches('`')
            .trim_matches(|c: char| c == '*' || c == ':' || c == '=')
            .to_string();
        if clean.is_empty() || clean == "0" || clean == "1" {
            continue;
        }

        let interaction_split = Regex::new(r"(?i)\s*(?:x|\*|:)\s*").expect("regex");
        let parts = interaction_split
            .split(&clean)
            .map(normalize_concept_phrase)
            .filter(|p| !p.is_empty())
            .collect::<Vec<String>>();
        if parts.len() >= 2 {
            let interaction = parts.join(":");
            if !interactions.iter().any(|i| i == &interaction) {
                interactions.push(interaction);
            }
            for part in parts {
                if !iv.iter().any(|v| v == &part) {
                    iv.push(part);
                }
            }
            continue;
        }

        let single = normalize_concept_phrase(&clean);
        if single.is_empty() {
            continue;
        }
        let lower = single.to_lowercase();
        if lower.contains("control") || lower.contains("covariat") || lower.contains("demograph") {
            if !controls.iter().any(|v| v == &single) {
                controls.push(single);
            }
        } else if !iv.iter().any(|v| v == &single) {
            iv.push(single);
        }
    }

    (iv, controls, interactions)
}

fn normalize_concept_phrase(raw: &str) -> String {
    let explicit = Regex::new(r"`([A-Za-z][A-Za-z0-9_]*)`").expect("regex");
    if let Some(cap) = explicit.captures(raw) {
        return cap[1].to_string();
    }

    let lowered = raw
        .replace(['(', ')', '[', ']', '"', '\''], " ")
        .replace('/', " ")
        .replace('-', " ");
    let tokens = lowered
        .split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_ascii_alphanumeric())
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
        .filter(|w| !is_concept_stopword(w))
        .collect::<Vec<String>>();
    if tokens.is_empty() {
        return String::new();
    }
    if tokens.len() == 1 && !is_allowed_single_token(&tokens[0]) {
        return String::new();
    }
    tokens.join("_")
}

fn is_concept_stopword(word: &str) -> bool {
    let stop = [
        "a",
        "an",
        "the",
        "of",
        "for",
        "to",
        "in",
        "on",
        "at",
        "by",
        "and",
        "or",
        "is",
        "are",
        "be",
        "being",
        "been",
        "that",
        "this",
        "these",
        "those",
        "our",
        "their",
        "participant",
        "participants",
        "self",
        "reported",
        "measure",
        "measured",
        "item",
        "items",
        "question",
        "questions",
        "respond",
        "response",
        "responses",
        "asked",
        "ask",
        "student",
        "students",
        "advisee",
        "advisors",
        "advisor",
        "company",
        "offer",
        "offers",
        "option",
        "options",
        "using",
        "will",
        "would",
        "should",
        "can",
        "could",
        "anything",
        "after",
        "before",
        "during",
        "between",
        "then",
        "than",
        "where",
        "which",
        "what",
        "when",
        "with",
        "without",
        "include",
        "excluding",
        "exclude",
        "remove",
        "drop",
        "control",
        "controls",
        "covariate",
        "covariates",
        "outcome",
        "outcomes",
        "analysis",
        "variable",
        "variables",
    ];
    stop.iter().any(|v| v == &word)
}

fn is_allowed_single_token(word: &str) -> bool {
    let allowed = [
        "age",
        "gender",
        "race",
        "education",
        "income",
        "condition",
        "demographics",
        "bot_score",
        "duration",
    ];
    allowed.iter().any(|v| v == &word)
}

fn plausible_variable_token(token: &str) -> bool {
    let value = token.trim().trim_matches('`');
    if value.is_empty() {
        return false;
    }
    let lower = value.to_lowercase();
    if is_concept_stopword(&lower) {
        return false;
    }
    if Regex::new(r"^QID\d+$").expect("regex").is_match(value) {
        return true;
    }
    if value.contains('_') {
        return true;
    }
    if Regex::new(r"^[a-z]+[A-Z][A-Za-z0-9]*$")
        .expect("regex")
        .is_match(value)
    {
        return true;
    }
    Regex::new(r"^[A-Za-z][A-Za-z0-9]{2,}$")
        .expect("regex")
        .is_match(value)
        && value.len() <= 64
}

#[cfg(test)]
mod tests {
    use super::fill_from_text;
    use crate::prereg::types::PreregSpec;

    #[test]
    fn extracts_models_from_prereg_prose_with_coefficient_style_formula() {
        let txt = r#"
5) Specify exactly which analyses you will conduct to examine the main question/hypothesis.
Our primary analysis of interest is an OLS regression predicting our two advice-sharing variables.
(1) advice_choice ~ B0 + B1 x income condition + B2 x information condition + B3 x income condition x information condition
(2) advice_continuous ~ B0 + B1 x income condition + B2 x information condition + B3 x income condition x information condition

8) Anything else
As a robustness check, we will run our regressions both without any control variables and controlling for participant demographics.
"#;
        let mut spec = PreregSpec::default();
        fill_from_text(&mut spec, txt);
        assert!(spec.main_analyses.len() >= 2);
        assert!(spec.main_analyses.iter().any(|m| m.dv == "advice_choice"));
        assert!(spec
            .main_analyses
            .iter()
            .all(|m| m.iv.contains(&"income_condition".to_string())));
        assert!(spec
            .main_analyses
            .iter()
            .all(|m| m.iv.contains(&"information_condition".to_string())));
        assert!(spec
            .robustness_checks
            .contains(&"with_without_controls".to_string()));
        assert!(!spec
            .warnings
            .iter()
            .any(|w| w == "NO_MAIN_ANALYSIS_EXTRACTED"));
    }

    #[test]
    fn does_not_promote_generic_words_to_variables() {
        let txt = r#"
3) Describe the key dependent variable(s) specifying how they will be measured.
Participants will be asked to advise the student.

5) Specify exactly which analyses you will conduct to examine the main question/hypothesis.
(1) advice_choice ~ B0 + B1 x income condition + B2 x information condition + B3 x income condition x information condition
"#;
        let mut spec = PreregSpec::default();
        fill_from_text(&mut spec, txt);
        let mut vars = spec.variables.dv.clone();
        vars.extend(spec.variables.iv.clone());
        vars.extend(spec.variables.controls.clone());
        assert!(!vars.iter().any(|v| v == "asked"));
        assert!(!vars.iter().any(|v| v == "student"));
        assert!(vars.iter().any(|v| v == "advice_choice"));
    }
}
