use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreregMetadata {
  pub title: Option<String>,
  pub date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VariableSets {
  pub dv: Vec<String>,
  pub iv: Vec<String>,
  pub controls: Vec<String>,
  pub moderators: Vec<String>,
  pub mediators: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisModelSpec {
  pub id: String,
  pub dv: String,
  pub iv: Vec<String>,
  pub controls: Vec<String>,
  pub interaction_terms: Vec<String>,
  pub formula: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExclusionRule {
  pub id: String,
  pub rule_type: String,
  pub variable: Option<String>,
  pub criterion: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DerivedScale {
  pub name: String,
  pub derived_type: String,
  pub depends_on: Vec<String>,
  pub definition: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreregSpec {
  pub metadata: PreregMetadata,
  pub variables: VariableSets,
  pub main_analyses: Vec<AnalysisModelSpec>,
  pub exploratory_analyses: Vec<AnalysisModelSpec>,
  pub robustness_checks: Vec<String>,
  pub exclusion_rules: Vec<ExclusionRule>,
  pub derived_scales: Vec<DerivedScale>,
  pub missing_data_plan: Option<String>,
  pub sections: HashMap<String, String>,
  pub warnings: Vec<String>,
}

impl Default for PreregSpec {
  fn default() -> Self {
    Self {
      metadata: PreregMetadata {
        title: None,
        date: None,
      },
      variables: VariableSets {
        dv: Vec::new(),
        iv: Vec::new(),
        controls: Vec::new(),
        moderators: Vec::new(),
        mediators: Vec::new(),
      },
      main_analyses: Vec::new(),
      exploratory_analyses: Vec::new(),
      robustness_checks: Vec::new(),
      exclusion_rules: Vec::new(),
      derived_scales: Vec::new(),
      missing_data_plan: None,
      sections: HashMap::new(),
      warnings: Vec::new(),
    }
  }
}
