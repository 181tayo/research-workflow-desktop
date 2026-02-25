use std::collections::HashMap;

use super::types::{QsfEmbeddedData, QsfQuestion, QsfSurveySpec};

const STANDARD_COLUMNS: &[&str] = &[
    "ResponseId",
    "Finished",
    "Progress",
    "Duration (in seconds)",
    "RecordedDate",
    "StartDate",
    "EndDate",
    "Status",
];

pub fn build_spec(
    survey_name: String,
    questions: Vec<QsfQuestion>,
    embedded_data_fields: Vec<QsfEmbeddedData>,
) -> QsfSurveySpec {
    let mut expected_columns: Vec<String> =
        STANDARD_COLUMNS.iter().map(|v| v.to_string()).collect();
    let mut label_map: HashMap<String, String> = HashMap::new();
    let embedded_data = embedded_data_fields
        .iter()
        .map(|f| f.name.clone())
        .collect::<Vec<String>>();

    for q in &questions {
        if !expected_columns.iter().any(|c| c == &q.export_tag) {
            expected_columns.push(q.export_tag.clone());
        }
        label_map.insert(q.export_tag.clone(), clean_label(&q.question_text));
    }
    for ed in &embedded_data {
        if !expected_columns.iter().any(|c| c == ed) {
            expected_columns.push(ed.clone());
        }
    }

    QsfSurveySpec {
        survey_name,
        questions,
        embedded_data,
        embedded_data_fields,
        expected_columns,
        label_map,
    }
}

fn clean_label(text: &str) -> String {
    let stripped = text.replace('\n', " ").replace('\r', " ");
    let compact = stripped.split_whitespace().collect::<Vec<&str>>().join(" ");
    if compact.len() > 200 {
        format!("{}...", &compact[..197])
    } else {
        compact
    }
}
