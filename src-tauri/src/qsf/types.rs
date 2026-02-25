use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QsfChoice {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QsfEmbeddedData {
    pub name: String,
    pub default_value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QsfQuestion {
    pub qualtrics_qid: String,
    pub export_tag: String,
    pub question_text: String,
    pub question_type: String,
    pub choices: Vec<QsfChoice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QsfSurveySpec {
    pub survey_name: String,
    pub questions: Vec<QsfQuestion>,
    pub embedded_data: Vec<String>,
    pub embedded_data_fields: Vec<QsfEmbeddedData>,
    pub expected_columns: Vec<String>,
    pub label_map: HashMap<String, String>,
}
