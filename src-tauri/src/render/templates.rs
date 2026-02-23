use std::path::{Path, PathBuf};

use tera::{Context, Tera};

use crate::render::helpers::write_string;
use crate::spec::types::AnalysisSpec;

const ORDERED_PARTIALS: &[&str] = &[
  "00_header.Rmd.tera",
  "01_packages.R.tera",
  "02_import_clean.R.tera",
  "03_main_models.R.tera",
  "04_robustness.R.tera",
  "05_exploratory.R.tera",
  "06_tables_figures.R.tera",
  "99_appendix.R.tera",
];

pub fn render_from_spec(spec: &AnalysisSpec, template_root: &Path, out_rmd: &Path, out_r: &Path) -> Result<(), String> {
  let pattern = format!(
    "{}/analysis/{}/**/*",
    template_root.display(),
    spec.template_bindings.template_set
  );
  let tera = Tera::new(&pattern).map_err(|e| format!("Template load failed: {e}"))?;

  let mut ctx = Context::new();
  ctx.insert("spec", spec);

  let mut rendered = String::new();
  for partial in ORDERED_PARTIALS {
    let template_name = tera
      .get_template_names()
      .find(|name| name.ends_with(partial))
      .ok_or_else(|| format!("Template '{}' not found in loaded template set.", partial))?
      .to_string();
    let chunk = tera
      .render(&template_name, &ctx)
      .map_err(|e| format!("Render failed for {template_name}: {e}"))?;
    rendered.push_str(&chunk);
    rendered.push_str("\n\n");
  }

  write_string(out_rmd, &rendered)?;

  let mut r_helper = String::new();
  r_helper.push_str("# Auto-generated helper script\n");
  r_helper.push_str("rmarkdown::render('analysis/analysis.Rmd')\n");
  write_string(out_r, &r_helper)?;

  Ok(())
}

pub fn template_root_from_cwd() -> Result<PathBuf, String> {
  let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
  let local = cwd.join("templates");
  if local.exists() {
    return Ok(local);
  }
  let parent = cwd
    .parent()
    .map(|p| p.join("templates"))
    .unwrap_or_else(|| cwd.join("templates"));
  Ok(parent)
}

#[cfg(test)]
mod tests {
  use super::render_from_spec;
  use crate::spec::types::{
    AnalysisSpec, DataContractSpec, InputRef, InputsSpec, ModelsSpec, OutputsSpec, TemplateBindingsSpec
  };
  use std::collections::HashMap;
  use std::path::PathBuf;
  use uuid::Uuid;

  #[test]
  fn renders_rmd_with_style_sources() {
    let spec = AnalysisSpec {
      project_id: "p".to_string(),
      study_id: "s".to_string(),
      analysis_id: "a".to_string(),
      inputs: InputsSpec {
        qsf: InputRef { path: "q".to_string(), sha256: "x".to_string() },
        prereg: InputRef { path: "p".to_string(), sha256: "y".to_string() },
      },
      data_contract: DataContractSpec {
        source: "qualtrics_csv".to_string(),
        id_columns: HashMap::new(),
        expected_columns: vec![],
        label_map: HashMap::new(),
        exclusions: vec![],
        missingness: None,
        derived_variables: vec![],
      },
      variable_mappings: vec![],
      models: ModelsSpec { main: vec![], exploratory: vec![], robustness: vec![] },
      outputs: OutputsSpec { tables: vec![], figures: vec![] },
      template_bindings: TemplateBindingsSpec {
        template_set: "apa_v1".to_string(),
        style_profile: "apa_flextable_ggpubr".to_string(),
        paths: HashMap::from([
          ("data_raw".to_string(), "x.csv".to_string()),
          ("data_clean".to_string(), "y.csv".to_string()),
          ("tables_dir".to_string(), "tables".to_string()),
          ("figures_dir".to_string(), "figures".to_string()),
        ]),
        packages: vec!["tidyverse".to_string()],
      },
      warnings: vec![],
    };

    let root = std::env::current_dir().expect("cwd");
    let tmp = std::env::temp_dir().join(format!("render-test-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&tmp).expect("tmp");
    let out_rmd: PathBuf = tmp.join("analysis.Rmd");
    let out_r: PathBuf = tmp.join("analysis.R");
    let template_root = if root.join("templates").exists() {
      root.join("templates")
    } else {
      root.parent().expect("parent").join("templates")
    };
    render_from_spec(&spec, &template_root, &out_rmd, &out_r).expect("render");
    let rendered = std::fs::read_to_string(&out_rmd).expect("read");
    assert!(rendered.contains("source(\"styles/apa_flextable_ggpubr/style.R\")"));
    let _ = std::fs::remove_dir_all(tmp);
  }
}
