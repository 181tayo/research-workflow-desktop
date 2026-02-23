use super::extract::fill_from_text;
use super::types::PreregSpec;

pub fn parse_prereg_md(raw: &str) -> PreregSpec {
  let mut spec = PreregSpec::default();
  fill_from_text(&mut spec, raw);
  spec
}
