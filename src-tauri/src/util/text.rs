use regex::Regex;

pub fn normalize_token(value: &str) -> String {
  value
    .to_lowercase()
    .chars()
    .map(|c| if c.is_ascii_alphanumeric() { c } else { ' ' })
    .collect::<String>()
    .split_whitespace()
    .collect::<Vec<&str>>()
    .join("_")
}

pub fn tokenize_identifiers(text: &str) -> Vec<String> {
  let backticks = Regex::new(r"`([A-Za-z][A-Za-z0-9_]*)`").expect("regex");
  let snake = Regex::new(r"\b[A-Za-z][A-Za-z0-9]*_[A-Za-z0-9_]+\b").expect("regex");
  let camel = Regex::new(r"\b[a-z]+[A-Z][A-Za-z0-9]*\b").expect("regex");
  let qid = Regex::new(r"\bQID\d+\b").expect("regex");
  let mut out: Vec<String> = Vec::new();
  for cap in backticks.captures_iter(text) {
    out.push(cap[1].to_string());
  }
  for cap in snake.captures_iter(text) {
    out.push(cap[0].to_string());
  }
  for cap in camel.captures_iter(text) {
    out.push(cap[0].to_string());
  }
  for cap in qid.captures_iter(text) {
    out.push(cap[0].to_string());
  }
  out.sort();
  out.dedup();
  out
}
