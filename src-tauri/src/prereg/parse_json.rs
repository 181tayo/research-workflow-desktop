use super::extract::fill_from_text;
use super::types::PreregSpec;

pub fn parse_prereg_json(raw: &str) -> Result<PreregSpec, String> {
  let parsed: serde_json::Value = serde_json::from_str(raw).map_err(|e| format!("Invalid prereg JSON: {e}"))?;
  if let Ok(spec) = serde_json::from_value::<PreregSpec>(parsed.clone()) {
    return Ok(spec);
  }

  let mut spec = PreregSpec::default();
  fill_from_text(&mut spec, &parsed.to_string());
  Ok(spec)
}
