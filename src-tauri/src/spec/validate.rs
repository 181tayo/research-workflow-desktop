use serde::Serialize;

pub fn to_json_value<T: Serialize>(value: &T) -> Result<serde_json::Value, String> {
  serde_json::to_value(value).map_err(|e| format!("Serialization failed: {e}"))
}
