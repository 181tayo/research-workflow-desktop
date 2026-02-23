use std::collections::HashMap;

use crate::prereg::types::{AnalysisModelSpec, PreregSpec};
use crate::qsf::types::QsfSurveySpec;
use crate::spec::mapping::{map_variable, unresolved_warning};
use crate::util::hash::sha256_hex;

use super::types::{
  AnalysisSpec, DataContractSpec, DerivedVariableSpec, ExclusionSpec, InputRef, InputsSpec,
  MappingResult, ModelSpec, ModelsSpec, OutputsSpec, TemplateBindingsSpec, WarningItem,
};

pub fn build_analysis_spec(
  project_id: &str,
  study_id: &str,
  analysis_id: &str,
  qsf_path: &str,
  prereg_path: &str,
  qsf_bytes: &[u8],
  prereg_bytes: &[u8],
  qsf: &QsfSurveySpec,
  prereg: &PreregSpec,
  template_set: &str,
  style_profile: &str,
) -> AnalysisSpec {
  let mappings = collect_mappings(qsf, prereg);
  let mut warnings = collect_warnings(&mappings, prereg);
  let auto_merge_derived = build_counterbalance_derived_variables(&mappings, qsf);

  let data_contract = DataContractSpec {
    source: "qualtrics_csv".to_string(),
    id_columns: HashMap::from([
      ("response_id".to_string(), "ResponseId".to_string()),
      ("participant_id".to_string(), "participant_id".to_string()),
    ]),
    expected_columns: qsf.expected_columns.clone(),
    label_map: qsf.label_map.clone(),
    exclusions: prereg
      .exclusion_rules
      .iter()
      .map(|e| ExclusionSpec {
        id: e.id.clone(),
        criterion: e.criterion.clone(),
        r_filter: format!("# TODO: apply exclusion: {}", e.criterion),
      })
      .collect(),
    missingness: prereg.missing_data_plan.clone(),
    derived_variables: prereg
      .derived_scales
      .iter()
      .map(|d| DerivedVariableSpec {
        name: d.name.clone(),
        derived_type: d.derived_type.clone(),
        depends_on: d.depends_on.clone(),
        definition: d.definition.clone(),
      })
      .chain(auto_merge_derived.into_iter())
      .collect(),
  };

  let models = ModelsSpec {
    main: map_models(&prereg.main_analyses, &mappings),
    exploratory: map_models(&prereg.exploratory_analyses, &mappings),
    robustness: build_robustness_models(prereg, &mappings),
  };

  if models.main.is_empty() {
    warnings.push(WarningItem {
      code: "NO_MAIN_MODELS".to_string(),
      message: "No main models were extracted from prereg.".to_string(),
      details: serde_json::json!({}),
    });
  }

  let outputs = OutputsSpec {
    tables: vec![
      "descriptives".to_string(),
      "balance_checks".to_string(),
      "model_summary".to_string(),
    ],
    figures: vec![
      "histograms".to_string(),
      "box_by_condition".to_string(),
      "coefplots".to_string(),
    ],
  };

  let template_bindings = TemplateBindingsSpec {
    template_set: template_set.to_string(),
    style_profile: style_profile.to_string(),
    paths: HashMap::from([
      ("data_raw".to_string(), "05_data/raw/data.csv".to_string()),
      ("data_clean".to_string(), "05_data/clean/data_clean.csv".to_string()),
      ("tables_dir".to_string(), "07_outputs/tables".to_string()),
      ("figures_dir".to_string(), "07_outputs/figures".to_string()),
    ]),
    packages: vec![
      "tidyverse".to_string(),
      "janitor".to_string(),
      "broom".to_string(),
      "flextable".to_string(),
      "officer".to_string(),
      "ggpubr".to_string(),
      "modelsummary".to_string(),
    ],
  };

  AnalysisSpec {
    project_id: project_id.to_string(),
    study_id: study_id.to_string(),
    analysis_id: analysis_id.to_string(),
    inputs: InputsSpec {
      qsf: InputRef {
        path: qsf_path.to_string(),
        sha256: sha256_hex(qsf_bytes),
      },
      prereg: InputRef {
        path: prereg_path.to_string(),
        sha256: sha256_hex(prereg_bytes),
      },
    },
    data_contract,
    variable_mappings: mappings,
    models,
    outputs,
    template_bindings,
    warnings,
  }
}

fn collect_mappings(qsf: &QsfSurveySpec, prereg: &PreregSpec) -> Vec<MappingResult> {
  let mut vars = Vec::new();
  vars.extend(prereg.variables.dv.clone());
  vars.extend(prereg.variables.iv.clone());
  vars.extend(prereg.variables.controls.clone());
  vars.sort();
  vars.dedup();
  vars.into_iter().map(|v| map_variable(&v, qsf)).collect()
}

fn collect_warnings(mappings: &[MappingResult], prereg: &PreregSpec) -> Vec<WarningItem> {
  let mut warnings: Vec<WarningItem> = mappings.iter().filter_map(unresolved_warning).collect();
  warnings.extend(prereg.warnings.iter().map(|w| WarningItem {
    code: w.clone(),
    message: w.clone(),
    details: serde_json::json!({}),
  }));
  warnings
}

fn resolved_or_todo(var: &str, mappings: &[MappingResult], unresolved: &mut Vec<String>) -> String {
  if let Some(m) = mappings.iter().find(|m| m.prereg_var.eq_ignore_ascii_case(var)) {
    if let Some(col) = &m.resolved_to {
      return col.clone();
    }
  }
  unresolved.push(var.to_string());
  format!("TODO_{}", sanitize_identifier(var))
}

fn sanitize_identifier(value: &str) -> String {
  let mut out = String::new();
  for ch in value.chars() {
    if ch.is_ascii_alphanumeric() {
      out.push(ch.to_ascii_lowercase());
    } else if !out.ends_with('_') {
      out.push('_');
    }
  }
  let trimmed = out.trim_matches('_').to_string();
  if trimmed.is_empty() {
    "unresolved_var".to_string()
  } else {
    trimmed
  }
}

fn map_models(models: &[AnalysisModelSpec], mappings: &[MappingResult]) -> Vec<ModelSpec> {
  models
    .iter()
    .map(|m| {
      let mut unresolved = Vec::new();
      let dv = resolved_or_todo(&m.dv, mappings, &mut unresolved);
      let iv = m
        .iv
        .iter()
        .map(|v| resolved_or_todo(v, mappings, &mut unresolved))
        .collect::<Vec<String>>();
      let controls = m
        .controls
        .iter()
        .map(|v| resolved_or_todo(v, mappings, &mut unresolved))
        .collect::<Vec<String>>();
      let rhs = iv
        .iter()
        .chain(controls.iter())
        .cloned()
        .collect::<Vec<String>>()
        .join(" + ");
      ModelSpec {
        id: m.id.clone(),
        family: "gaussian".to_string(),
        dv: dv.clone(),
        iv,
        controls,
        interactions: m.interaction_terms.clone(),
        formula: format!("{} ~ {}", dv, rhs),
        unresolved_variables: unresolved,
      }
    })
    .collect()
}

fn build_robustness_models(prereg: &PreregSpec, mappings: &[MappingResult]) -> Vec<ModelSpec> {
  let mut out = map_models(&prereg.exploratory_analyses, mappings);
  if prereg
    .robustness_checks
    .iter()
    .any(|v| v == "with_without_controls")
  {
    for main in map_models(&prereg.main_analyses, mappings) {
      out.push(ModelSpec {
        id: format!("{}_with_controls", main.id),
        family: main.family.clone(),
        dv: main.dv.clone(),
        iv: main.iv.clone(),
        controls: main.controls.clone(),
        interactions: main.interactions.clone(),
        formula: main.formula.clone(),
        unresolved_variables: main.unresolved_variables.clone(),
      });
      out.push(ModelSpec {
        id: format!("{}_without_controls", main.id),
        family: main.family.clone(),
        dv: main.dv.clone(),
        iv: main.iv.clone(),
        controls: Vec::new(),
        interactions: main.interactions.clone(),
        formula: format!("{} ~ {}", main.dv, main.iv.join(" + ")),
        unresolved_variables: main.unresolved_variables.clone(),
      });
    }
  }
  out
}

fn build_counterbalance_derived_variables(
  mappings: &[MappingResult],
  qsf: &QsfSurveySpec,
) -> Vec<DerivedVariableSpec> {
  let expected = qsf
    .expected_columns
    .iter()
    .map(|v| v.to_lowercase())
    .collect::<Vec<String>>();
  let mut out = Vec::new();
  for m in mappings {
    let Some(resolved) = &m.resolved_to else {
      continue;
    };
    // This indicates map_variable auto-resolved to prereg var rather than a raw column.
    if !resolved.eq_ignore_ascii_case(&m.prereg_var) {
      continue;
    }
    if expected.iter().any(|col| col.eq_ignore_ascii_case(resolved)) {
      continue;
    }
    let sources = candidate_pair_sources(&m.candidates, &m.prereg_var);
    if sources.len() < 2 {
      continue;
    }
    let definition = format!(
      "dplyr::coalesce({})",
      sources
        .iter()
        .map(|s| format!("`{}`", s))
        .collect::<Vec<String>>()
        .join(", ")
    );
    out.push(DerivedVariableSpec {
      name: resolved.clone(),
      derived_type: "counterbalance_merge".to_string(),
      depends_on: sources,
      definition,
    });
  }
  out
}

fn candidate_pair_sources(candidates: &[crate::spec::types::MappingCandidate], prereg_var: &str) -> Vec<String> {
  let prereg_norm = crate::util::text::normalize_token(prereg_var);
  let mut filtered = candidates
    .iter()
    .filter(|c| c.score >= 0.70)
    .map(|c| c.key.clone())
    .collect::<Vec<String>>();
  filtered.sort();
  filtered.dedup();
  for i in 0..filtered.len() {
    for j in (i + 1)..filtered.len() {
      let a = &filtered[i];
      let b = &filtered[j];
      let a_norm = crate::util::text::normalize_token(a);
      let b_norm = crate::util::text::normalize_token(b);
      let a_base = strip_order_suffix(&a_norm);
      let b_base = strip_order_suffix(&b_norm);
      if a_base.is_empty() || a_base != b_base {
        continue;
      }
      if a_base == prereg_norm || a_base.contains(&prereg_norm) || prereg_norm.contains(&a_base) {
        return vec![a.clone(), b.clone()];
      }
    }
  }
  Vec::new()
}

fn strip_order_suffix(value: &str) -> String {
  let re = regex::Regex::new(r"(?i)(?:_)?[ab]\d+$").expect("regex");
  re.replace(value, "").to_string()
}

#[cfg(test)]
mod tests {
  use super::build_analysis_spec;
  use crate::prereg::types::{AnalysisModelSpec, PreregSpec};
  use crate::qsf::types::{QsfQuestion, QsfSurveySpec};
  use std::collections::HashMap;

  #[test]
  fn builds_spec_and_emits_unresolved_warning() {
    let qsf = QsfSurveySpec {
      survey_name: "Survey".to_string(),
      questions: vec![QsfQuestion {
        qualtrics_qid: "QID1".to_string(),
        export_tag: "known_x".to_string(),
        question_text: "Known".to_string(),
        question_type: "MC".to_string(),
        choices: vec![],
      }],
      embedded_data: vec![],
      embedded_data_fields: vec![],
      expected_columns: vec!["known_x".to_string()],
      label_map: HashMap::new(),
    };
    let mut prereg = PreregSpec::default();
    prereg.variables.dv = vec!["missing_y".to_string()];
    prereg.variables.iv = vec!["known_x".to_string()];
    prereg.main_analyses.push(AnalysisModelSpec {
      id: "m1".to_string(),
      dv: "missing_y".to_string(),
      iv: vec!["known_x".to_string()],
      controls: vec![],
      interaction_terms: vec![],
      formula: Some("missing_y ~ known_x".to_string()),
    });
    let spec = build_analysis_spec(
      "p",
      "s",
      "a",
      "qsf",
      "prereg",
      b"q",
      b"p",
      &qsf,
      &prereg,
      "apa_v1",
      "apa_flextable_ggpubr",
    );
    assert!(!spec.models.main.is_empty());
    assert!(spec.warnings.iter().any(|w| w.code == "UNRESOLVED_VARIABLE"));
  }
}
