use regex::Regex;
use std::io::Read;
use zip::ZipArchive;

use super::extract::fill_from_text;
use super::types::PreregSpec;

pub fn parse_prereg_docx(path: &str) -> Result<PreregSpec, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("Unable to open DOCX: {e}"))?;
    let mut zip = ZipArchive::new(file).map_err(|e| format!("Invalid DOCX zip: {e}"))?;
    let mut xml = String::new();
    zip.by_name("word/document.xml")
        .map_err(|e| format!("DOCX missing word/document.xml: {e}"))?
        .read_to_string(&mut xml)
        .map_err(|e| format!("Unable to read DOCX XML: {e}"))?;

    let text = xml
        .replace("</w:p>", "\n")
        .replace("</w:tr>", "\n")
        .replace("</w:tc>", " ");
    let tag_re = Regex::new(r"<[^>]+>").expect("regex");
    let plain = tag_re.replace_all(&text, " ").to_string();

    build_structured_spec(&plain)
}

pub fn build_structured_spec(plain_text: &str) -> Result<PreregSpec, String> {
    let mut spec = PreregSpec::default();
    let section_re = Regex::new(r"(?m)^\s*(\d+)\)\s+(.+)$").expect("regex");
    let mut boundaries: Vec<(usize, String)> = Vec::new();
    for cap in section_re.captures_iter(plain_text) {
        if let Some(m) = cap.get(0) {
            boundaries.push((m.start(), cap[0].trim().to_string()));
        }
    }

    if boundaries.is_empty() {
        spec.warnings.push("DOCX_SECTIONS_NOT_DETECTED".to_string());
        fill_from_text(&mut spec, plain_text);
        return Ok(spec);
    }

    for i in 0..boundaries.len() {
        let (start, heading) = &boundaries[i];
        let end = if i + 1 < boundaries.len() {
            boundaries[i + 1].0
        } else {
            plain_text.len()
        };
        let body = plain_text[*start..end].trim().to_string();
        spec.sections.insert(heading.clone(), body.clone());
    }

    let full_text = spec
        .sections
        .values()
        .cloned()
        .collect::<Vec<String>>()
        .join("\n\n");
    fill_from_text(&mut spec, &full_text);
    Ok(spec)
}

#[cfg(test)]
mod tests {
    use super::build_structured_spec;

    #[test]
    fn extracts_sections_and_main_analysis_from_text() {
        let txt = "1) Variables\nDV: outcome_y\nIV: treat_x\n2) Analysis\noutcome_y ~ treat_x + age\n3) Exclusions\nexclude duration < 60";
        let spec = build_structured_spec(txt).expect("spec");
        assert!(!spec.sections.is_empty());
        assert!(!spec.main_analyses.is_empty());
        assert!(!spec.exclusion_rules.is_empty());
    }
}
