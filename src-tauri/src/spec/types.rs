use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::llm::types::{LlmModelLock, ModelProvenance};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputRef {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputsSpec {
    pub qsf: InputRef,
    pub prereg: InputRef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExclusionSpec {
    pub id: String,
    pub criterion: String,
    pub r_filter: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DerivedVariableSpec {
    pub name: String,
    pub derived_type: String,
    pub depends_on: Vec<String>,
    pub definition: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataContractSpec {
    pub source: String,
    pub id_columns: HashMap<String, String>,
    pub expected_columns: Vec<String>,
    pub label_map: HashMap<String, String>,
    pub exclusions: Vec<ExclusionSpec>,
    pub missingness: Option<String>,
    pub derived_variables: Vec<DerivedVariableSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSpec {
    pub id: String,
    pub family: String,
    pub dv: String,
    pub iv: Vec<String>,
    pub controls: Vec<String>,
    pub interactions: Vec<String>,
    pub formula: String,
    pub unresolved_variables: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelsSpec {
    pub main: Vec<ModelSpec>,
    pub exploratory: Vec<ModelSpec>,
    pub robustness: Vec<ModelSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputsSpec {
    pub tables: Vec<String>,
    pub figures: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateBindingsSpec {
    pub template_set: String,
    pub style_profile: String,
    pub paths: HashMap<String, String>,
    pub packages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MappingCandidate {
    pub key: String,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MappingResult {
    pub prereg_var: String,
    pub resolved_to: Option<String>,
    pub candidates: Vec<MappingCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WarningItem {
    pub code: String,
    pub message: String,
    pub details: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisSpec {
    pub project_id: String,
    pub study_id: String,
    pub analysis_id: String,
    pub inputs: InputsSpec,
    pub data_contract: DataContractSpec,
    pub variable_mappings: Vec<MappingResult>,
    pub models: ModelsSpec,
    pub outputs: OutputsSpec,
    pub template_bindings: TemplateBindingsSpec,
    #[serde(default)]
    pub model_provenance: Option<ModelProvenance>,
    #[serde(default)]
    pub model_lock: Option<LlmModelLock>,
    pub warnings: Vec<WarningItem>,
}
