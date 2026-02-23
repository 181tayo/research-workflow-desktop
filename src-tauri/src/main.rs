#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod prereg;
mod qsf;
mod render;
mod spec;
mod util;

use chrono::Utc;
use pathdiff::diff_paths;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::AppHandle;
use uuid::Uuid;

use commands::analysis::{
  generate_analysis_spec, parse_prereg, parse_qsf, render_analysis_from_spec, resolve_mappings,
  save_analysis_spec
};
use commands::assets::{list_build_assets, list_prereg_assets};

const PROJECT_FOLDERS: &[&str] = &["studies", "paper", "templates"];
const STUDY_FOLDERS: &[&str] = &[
  "00_admin",
  "01_design",
  "02_build",
  "03_pilots",
  "04_prereg",
  "05_data",
  "06_analysis",
  "07_outputs",
  "08_osf_release"
];
const ANALYSIS_FOLDER: &str = "06_analysis";
const STYLE_KIT_DIR: &str = "R/style";
const STYLE_PACKAGE_NAME: &str = "researchworkflowstyle";
const STYLE_PACKAGE_DIR: &str = "R/researchworkflowstyle";
const ANALYSIS_CONFIG_PATH: &str = "config/analysis_defaults.json";

const DEFAULT_ANALYSIS_CONFIG_JSON: &str = r#"{
  "version": 1,
  "styleKit": {
    "mode": "project",
    "path": "R/style"
  },
  "stylePackage": {
    "name": "researchworkflowstyle",
    "path": "R/researchworkflowstyle"
  },
  "modules": {
    "plots": true,
    "tables": true
  },
  "plots": {
    "base_family": "Times New Roman",
    "base_size": 12,
    "dpi": 300,
    "ggpubr_palette": "jco"
  },
  "tables": {
    "font_family": "Times New Roman",
    "font_size": 12,
    "header_bold": true,
    "autofit": true
  }
}"#;

const THEME_PLOTS_R: &str = r#"# R/style/theme_plots.R

suppressPackageStartupMessages({
  library(ggplot2)
  library(ggpubr)
  library(rlang)
})

theme_apa <- function(base_size = 12, base_family = "Times New Roman") {
  ggplot2::theme_classic(base_size = base_size, base_family = base_family) +
    ggplot2::theme(
      plot.title = element_text(face = "bold", size = base_size + 2, hjust = 0),
      plot.subtitle = element_text(size = base_size, hjust = 0),
      plot.caption = element_text(size = base_size - 2, hjust = 1),
      axis.title = element_text(size = base_size),
      axis.text  = element_text(size = base_size - 1),
      legend.title = element_text(size = base_size - 1),
      legend.text = element_text(size = base_size - 1),
      panel.grid.major.y = element_line(linewidth = 0.25, color = "grey85"),
      panel.grid.minor = element_blank(),
      axis.line = element_line(linewidth = 0.5, color = "black"),
      plot.margin = margin(10, 10, 10, 10)
    )
}

set_apa_plot_defaults <- function(base_size = 12, base_family = "Times New Roman") {
  ggplot2::theme_set(theme_apa(base_size = base_size, base_family = base_family))
  invisible(TRUE)
}

apa_scatter <- function(df, x, y, add_lm = FALSE, se = TRUE, ...) {
  xq <- enquo(x); yq <- enquo(y)
  p <- ggplot(df, aes(x = !!xq, y = !!yq)) +
    geom_point(...) +
    theme_apa()
  if (isTRUE(add_lm)) {
    p <- p + geom_smooth(method = "lm", se = se, linewidth = 0.6)
  }
  p
}

apa_hist <- function(df, x, bins = 30, ...) {
  xq <- enquo(x)
  ggplot(df, aes(x = !!xq)) +
    geom_histogram(bins = bins, ...) +
    theme_apa()
}

apa_box <- function(df, x, y, ...) {
  xq <- enquo(x); yq <- enquo(y)
  ggplot(df, aes(x = !!xq, y = !!yq)) +
    geom_boxplot(...) +
    theme_apa()
}

theme_study_plot <- function(base_family = "Times New Roman") {
  ggplot2::theme(
    text = ggplot2::element_text(family = base_family),
    axis.text.x = ggplot2::element_text(size = 11),
    axis.text.y = ggplot2::element_text(size = 14),
    axis.title = ggplot2::element_text(size = 12, face = "bold"),
    axis.title.x = ggplot2::element_text(size = 16, face = "bold"),
    axis.title.y = ggplot2::element_text(size = 16, face = "bold"),
    legend.text = ggplot2::element_text(size = 12),
    legend.title = ggplot2::element_text(size = 12),
    plot.title = ggplot2::element_text(size = 18, face = "bold", hjust = 0.5),
    strip.background = ggplot2::element_rect(fill = "white"),
    strip.text = ggplot2::element_text(size = 12, face = "bold"),
    legend.background = ggplot2::element_rect(fill = "white", color = "white"),
    panel.background = ggplot2::element_rect(fill = "white", color = "white"),
    plot.background = ggplot2::element_rect(fill = "white", color = "white")
  )
}

style_box_plot <- function(
  data,
  x_col,
  y_col,
  fill_col = x_col,
  color_col = x_col,
  facet_col = NULL,
  palette = NULL,
  add_jitter = TRUE,
  y_breaks = NULL,
  y_labels = waiver(),
  hline_at = NULL,
  xlab = "",
  ylab = "",
  title = ""
) {
  p <- ggpubr::ggboxplot(
    data = data,
    x = x_col,
    y = y_col,
    fill = fill_col,
    color = color_col,
    facet.by = facet_col,
    add = if (isTRUE(add_jitter)) "jitter" else "none",
    add.params = list(alpha = 0.3, size = 0.9),
    palette = palette,
    xlab = xlab,
    ylab = ylab,
    title = title
  )
  if (!is.null(y_breaks)) {
    p <- p + ggplot2::scale_y_continuous(breaks = y_breaks, labels = y_labels)
  }
  if (!is.null(hline_at)) {
    p <- p + ggplot2::geom_hline(yintercept = hline_at, linetype = "dashed", color = "black", linewidth = 0.8)
  }
  p + theme_study_plot()
}

style_bar_plot <- function(
  data,
  x_col,
  y_col,
  fill_col = x_col,
  color_col = x_col,
  facet_col = NULL,
  palette = NULL,
  add = "mean_se",
  y_breaks = NULL,
  compare_groups = NULL,
  compare_method = "t.test",
  compare_label = "p.signif",
  compare_label_y = NULL,
  xlab = "",
  ylab = "",
  title = ""
) {
  p <- ggpubr::ggbarplot(
    data = data,
    x = x_col,
    y = y_col,
    fill = fill_col,
    color = color_col,
    facet.by = facet_col,
    add = add,
    palette = palette,
    xlab = xlab,
    ylab = ylab,
    title = title
  )
  if (!is.null(y_breaks)) {
    p <- p + ggplot2::scale_y_continuous(breaks = y_breaks)
  }
  if (!is.null(compare_groups)) {
    p <- p + ggpubr::stat_compare_means(
      comparisons = compare_groups,
      method = compare_method,
      label = compare_label,
      label.y = compare_label_y
    )
  }
  p + theme_study_plot()
}
"#;

const TABLES_FLEXTABLE_R: &str = r#"# R/style/tables_flextable.R

suppressPackageStartupMessages({
  library(flextable)
})

ft_apa <- function(x,
                   font_family = "Times New Roman",
                   font_size = 12,
                   header_bold = TRUE,
                   autofit = TRUE) {
  ft <- flextable::flextable(x)
  ft <- flextable::font(ft, fontname = font_family, part = "all")
  ft <- flextable::fontsize(ft, size = font_size, part = "all")
  ft <- flextable::align(ft, align = "center", part = "header")
  ft <- flextable::align(ft, align = "center", part = "body")
  ft <- flextable::border_remove(ft)
  ft <- flextable::hline_top(ft, border = officer::fp_border(width = 1))
  ft <- flextable::hline(ft, i = 1, border = officer::fp_border(width = 1), part = "header")
  ft <- flextable::hline_bottom(ft, border = officer::fp_border(width = 1))
  if (isTRUE(header_bold)) {
    ft <- flextable::bold(ft, part = "header")
  }
  if (isTRUE(autofit)) {
    ft <- flextable::autofit(ft)
  }
  ft
}

ft_apa_descriptives <- function(df, digits = 2) {
  # Basic descriptive summary for numeric columns
  num <- df[, vapply(df, is.numeric, logical(1)), drop = FALSE]
  if (ncol(num) == 0) stop("No numeric columns found for descriptives.")
  out <- data.frame(
    Variable = names(num),
    N = vapply(num, function(x) sum(!is.na(x)), numeric(1)),
    Mean = vapply(num, function(x) mean(x, na.rm = TRUE), numeric(1)),
    SD = vapply(num, function(x) stats::sd(x, na.rm = TRUE), numeric(1)),
    Min = vapply(num, function(x) min(x, na.rm = TRUE), numeric(1)),
    Max = vapply(num, function(x) max(x, na.rm = TRUE), numeric(1)),
    check.names = FALSE
  )
  out$Mean <- round(out$Mean, digits)
  out$SD   <- round(out$SD, digits)
  out$Min  <- round(out$Min, digits)
  out$Max  <- round(out$Max, digits)

  ft_apa(out)
}

ft_apa_regression <- function(model, ...) {
  stop("ft_apa_regression() is a placeholder. Consider using broom + dplyr to create a data.frame, then pass to ft_apa().")
}

style_model_table <- function(
  models,
  output_path = NULL,
  estimate = "{estimate}{stars}",
  statistic = "({std.error})",
  stars = c("*" = .05, "**" = .01, "***" = .001),
  ...
) {
  if (!requireNamespace("modelsummary", quietly = TRUE)) {
    stop("Package `modelsummary` is required for style_model_table().")
  }
  tbl <- modelsummary::modelsummary(
    models,
    estimate = estimate,
    statistic = statistic,
    stars = stars,
    ...
  )
  if (!is.null(output_path)) {
    modelsummary::modelsummary(
      models,
      estimate = estimate,
      statistic = statistic,
      stars = stars,
      output = output_path,
      ...
    )
  }
  tbl
}
"#;

const STYLE_INIT_R: &str = r#"# R/style/style_init.R

suppressPackageStartupMessages({
  library(here)
})

init_project_style <- function(config_path = here::here("config/analysis_defaults.json")) {
  cfg <- list(
    plots = list(base_family = "Times New Roman", base_size = 12),
    tables = list(font_family = "Times New Roman", font_size = 12, header_bold = TRUE, autofit = TRUE)
  )

  if (file.exists(config_path)) {
    if (requireNamespace("jsonlite", quietly = TRUE)) {
      user_cfg <- jsonlite::fromJSON(config_path, simplifyVector = TRUE)
      # shallow merge
      if (!is.null(user_cfg$plots)) cfg$plots <- modifyList(cfg$plots, user_cfg$plots)
      if (!is.null(user_cfg$tables)) cfg$tables <- modifyList(cfg$tables, user_cfg$tables)
    }
  }

  # Apply plot defaults if available
  if (exists("set_apa_plot_defaults", mode = "function")) {
    set_apa_plot_defaults(base_size = cfg$plots$base_size, base_family = cfg$plots$base_family)
  }

  invisible(cfg)
}
"#;

const STYLE_README_MD: &str = r#"# Project Style Kit

This folder contains shared, project-level styling helpers used by generated analysis templates.

- `theme_plots.R`: APA-ish plot theme and helper plot wrappers.
- `tables_flextable.R`: APA-ish table formatting helpers with `flextable`.
- `style_init.R`: Initializes style defaults from `config/analysis_defaults.json`.

Customize these files once to affect all future analyses that source them.
"#;

const STYLE_PACKAGE_DESCRIPTION: &str = r#"Package: researchworkflowstyle
Type: Package
Title: Shared Figure and Table Style Helpers
Version: 0.1.0
Authors@R: person("Research", "Team", email = "noreply@example.com", role = c("aut", "cre"))
Description: Shared plotting and table helpers for project analysis templates.
License: MIT + file LICENSE
Encoding: UTF-8
LazyData: true
RoxygenNote: 7.3.2
Depends:
    R (>= 4.1.0)
Imports:
    flextable,
    ggpubr,
    ggplot2,
    here,
    rlang,
    officer
Suggests:
    dplyr,
    gganimate,
    jsonlite,
    modelsummary,
    stringr
"#;

const STYLE_PACKAGE_NAMESPACE: &str = r#"export(theme_apa)
export(set_apa_plot_defaults)
export(apa_scatter)
export(apa_hist)
export(apa_box)
export(theme_study_plot)
export(style_box_plot)
export(style_bar_plot)
export(ft_apa)
export(ft_apa_descriptives)
export(ft_apa_regression)
export(style_model_table)
export(init_project_style)
"#;

const STYLE_PACKAGE_LICENSE: &str = r#"MIT License

Copyright (c) 2026
"#;

const STYLE_PACKAGE_PLOTS_R: &str = r#"# R/researchworkflowstyle/R/plots.R

theme_apa <- function(base_size = 12, base_family = "Times New Roman") {
  ggplot2::theme_classic(base_size = base_size, base_family = base_family) +
    ggplot2::theme(
      plot.title = ggplot2::element_text(face = "bold", size = base_size + 2, hjust = 0),
      plot.subtitle = ggplot2::element_text(size = base_size, hjust = 0),
      plot.caption = ggplot2::element_text(size = base_size - 2, hjust = 1),
      axis.title = ggplot2::element_text(size = base_size),
      axis.text  = ggplot2::element_text(size = base_size - 1),
      legend.title = ggplot2::element_text(size = base_size - 1),
      legend.text = ggplot2::element_text(size = base_size - 1),
      panel.grid.major.y = ggplot2::element_line(linewidth = 0.25, color = "grey85"),
      panel.grid.minor = ggplot2::element_blank(),
      axis.line = ggplot2::element_line(linewidth = 0.5, color = "black"),
      plot.margin = ggplot2::margin(10, 10, 10, 10)
    )
}

set_apa_plot_defaults <- function(base_size = 12, base_family = "Times New Roman") {
  ggplot2::theme_set(theme_apa(base_size = base_size, base_family = base_family))
  invisible(TRUE)
}

apa_scatter <- function(df, x, y, add_lm = FALSE, se = TRUE, ...) {
  xq <- rlang::enquo(x)
  yq <- rlang::enquo(y)
  p <- ggplot2::ggplot(df, ggplot2::aes(x = !!xq, y = !!yq)) +
    ggplot2::geom_point(...) +
    theme_apa()
  if (isTRUE(add_lm)) {
    p <- p + ggplot2::geom_smooth(method = "lm", se = se, linewidth = 0.6)
  }
  p
}

apa_hist <- function(df, x, bins = 30, ...) {
  xq <- rlang::enquo(x)
  ggplot2::ggplot(df, ggplot2::aes(x = !!xq)) +
    ggplot2::geom_histogram(bins = bins, ...) +
    theme_apa()
}

apa_box <- function(df, x, y, ...) {
  xq <- rlang::enquo(x)
  yq <- rlang::enquo(y)
  ggplot2::ggplot(df, ggplot2::aes(x = !!xq, y = !!yq)) +
    ggplot2::geom_boxplot(...) +
    theme_apa()
}

theme_study_plot <- function(base_family = "Times New Roman") {
  ggplot2::theme(
    text = ggplot2::element_text(family = base_family),
    axis.text.x = ggplot2::element_text(size = 11),
    axis.text.y = ggplot2::element_text(size = 14),
    axis.title = ggplot2::element_text(size = 12, face = "bold"),
    axis.title.x = ggplot2::element_text(size = 16, face = "bold"),
    axis.title.y = ggplot2::element_text(size = 16, face = "bold"),
    legend.text = ggplot2::element_text(size = 12),
    legend.title = ggplot2::element_text(size = 12),
    plot.title = ggplot2::element_text(size = 18, face = "bold", hjust = 0.5),
    strip.background = ggplot2::element_rect(fill = "white"),
    strip.text = ggplot2::element_text(size = 12, face = "bold"),
    legend.background = ggplot2::element_rect(fill = "white", color = "white"),
    panel.background = ggplot2::element_rect(fill = "white", color = "white"),
    plot.background = ggplot2::element_rect(fill = "white", color = "white")
  )
}

style_box_plot <- function(
  data,
  x_col,
  y_col,
  fill_col = x_col,
  color_col = x_col,
  facet_col = NULL,
  palette = NULL,
  add_jitter = TRUE,
  y_breaks = NULL,
  y_labels = waiver(),
  hline_at = NULL,
  xlab = "",
  ylab = "",
  title = ""
) {
  p <- ggpubr::ggboxplot(
    data = data,
    x = x_col,
    y = y_col,
    fill = fill_col,
    color = color_col,
    facet.by = facet_col,
    add = if (isTRUE(add_jitter)) "jitter" else "none",
    add.params = list(alpha = 0.3, size = 0.9),
    palette = palette,
    xlab = xlab,
    ylab = ylab,
    title = title
  )
  if (!is.null(y_breaks)) {
    p <- p + ggplot2::scale_y_continuous(breaks = y_breaks, labels = y_labels)
  }
  if (!is.null(hline_at)) {
    p <- p + ggplot2::geom_hline(yintercept = hline_at, linetype = "dashed", color = "black", linewidth = 0.8)
  }
  p + theme_study_plot()
}

style_bar_plot <- function(
  data,
  x_col,
  y_col,
  fill_col = x_col,
  color_col = x_col,
  facet_col = NULL,
  palette = NULL,
  add = "mean_se",
  y_breaks = NULL,
  compare_groups = NULL,
  compare_method = "t.test",
  compare_label = "p.signif",
  compare_label_y = NULL,
  xlab = "",
  ylab = "",
  title = ""
) {
  p <- ggpubr::ggbarplot(
    data = data,
    x = x_col,
    y = y_col,
    fill = fill_col,
    color = color_col,
    facet.by = facet_col,
    add = add,
    palette = palette,
    xlab = xlab,
    ylab = ylab,
    title = title
  )
  if (!is.null(y_breaks)) {
    p <- p + ggplot2::scale_y_continuous(breaks = y_breaks)
  }
  if (!is.null(compare_groups)) {
    p <- p + ggpubr::stat_compare_means(
      comparisons = compare_groups,
      method = compare_method,
      label = compare_label,
      label.y = compare_label_y
    )
  }
  p + theme_study_plot()
}
"#;

const STYLE_PACKAGE_TABLES_R: &str = r#"# R/researchworkflowstyle/R/tables.R

ft_apa <- function(
  x,
  font_family = "Times New Roman",
  font_size = 12,
  header_bold = TRUE,
  autofit = TRUE,
  digits = NULL
) {
  if (is.data.frame(x) && !is.null(digits)) {
    num_cols <- vapply(x, is.numeric, logical(1))
    x[num_cols] <- lapply(x[num_cols], round, digits = digits)
  }
  ft <- flextable::flextable(x)
  ft <- flextable::font(ft, fontname = font_family, part = "all")
  ft <- flextable::fontsize(ft, size = font_size, part = "all")
  ft <- flextable::align(ft, align = "center", part = "header")
  ft <- flextable::align(ft, align = "center", part = "body")
  ft <- flextable::border_remove(ft)
  ft <- flextable::hline_top(ft, border = officer::fp_border(width = 1))
  ft <- flextable::hline(ft, i = 1, border = officer::fp_border(width = 1), part = "header")
  ft <- flextable::hline_bottom(ft, border = officer::fp_border(width = 1))
  if (isTRUE(header_bold)) {
    ft <- flextable::bold(ft, part = "header")
  }
  if (isTRUE(autofit)) {
    ft <- flextable::autofit(ft)
  }
  ft
}

ft_apa_descriptives <- function(df, digits = 2) {
  num <- df[, vapply(df, is.numeric, logical(1)), drop = FALSE]
  if (ncol(num) == 0) {
    stop("No numeric columns found for descriptives.")
  }
  out <- data.frame(
    Variable = names(num),
    N = vapply(num, function(x) sum(!is.na(x)), numeric(1)),
    Mean = vapply(num, function(x) mean(x, na.rm = TRUE), numeric(1)),
    SD = vapply(num, function(x) stats::sd(x, na.rm = TRUE), numeric(1)),
    Min = vapply(num, function(x) min(x, na.rm = TRUE), numeric(1)),
    Max = vapply(num, function(x) max(x, na.rm = TRUE), numeric(1)),
    check.names = FALSE
  )

  out$Mean <- round(out$Mean, digits)
  out$SD <- round(out$SD, digits)
  out$Min <- round(out$Min, digits)
  out$Max <- round(out$Max, digits)

  ft_apa(out)
}

ft_apa_regression <- function(model, ...) {
  stop("ft_apa_regression() is a placeholder. Build a data.frame then pass to ft_apa().")
}

style_model_table <- function(
  models,
  output_path = NULL,
  digits = 3,
  estimate = "{estimate}{stars}",
  statistic = "({std.error})",
  stars = c("*" = .05, "**" = .01, "***" = .001),
  ...
) {
  if (!requireNamespace("modelsummary", quietly = TRUE)) {
    stop("Package `modelsummary` is required for style_model_table().")
  }
  tbl <- modelsummary::modelsummary(
    models,
    fmt = digits,
    estimate = estimate,
    statistic = statistic,
    stars = stars,
    ...
  )
  if (!is.null(output_path)) {
    modelsummary::modelsummary(
      models,
      fmt = digits,
      estimate = estimate,
      statistic = statistic,
      stars = stars,
      output = output_path,
      ...
    )
  }
  tbl
}
"#;

const STYLE_PACKAGE_INIT_R: &str = r#"# R/researchworkflowstyle/R/init.R

init_project_style <- function(config_path = here::here("config/analysis_defaults.json")) {
  cfg <- list(
    plots = list(base_family = "Times New Roman", base_size = 12),
    tables = list(font_family = "Times New Roman", font_size = 12, header_bold = TRUE, autofit = TRUE)
  )

  if (file.exists(config_path) && requireNamespace("jsonlite", quietly = TRUE)) {
    user_cfg <- jsonlite::fromJSON(config_path, simplifyVector = TRUE)
    if (!is.null(user_cfg$plots)) {
      cfg$plots <- utils::modifyList(cfg$plots, user_cfg$plots)
    }
    if (!is.null(user_cfg$tables)) {
      cfg$tables <- utils::modifyList(cfg$tables, user_cfg$tables)
    }
  }

  set_apa_plot_defaults(
    base_size = cfg$plots$base_size,
    base_family = cfg$plots$base_family
  )

  invisible(cfg)
}
"#;

const STYLE_PACKAGE_README_MD: &str = r#"# researchworkflowstyle

Local project package for shared figure and table styling helpers used by generated analysis templates.

Usage in analysis scripts:
- Prefer `researchworkflowstyle::init_project_style()`.
- Use `researchworkflowstyle::theme_apa()` and `researchworkflowstyle::ft_apa()` directly.
- Use `researchworkflowstyle::style_box_plot()` and `researchworkflowstyle::style_bar_plot()` for consistent figure styling.
- Use `researchworkflowstyle::style_model_table()` for consistent regression table output.
"#;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Project {
  id: String,
  name: String,
  #[serde(alias = "root_path")]
  root_path: String,
  #[serde(alias = "created_at")]
  created_at: String,
  #[serde(default)]
  #[serde(alias = "updated_at")]
  updated_at: String,
  #[serde(default)]
  #[serde(alias = "google_drive_url")]
  google_drive_url: Option<String>,
  #[serde(default)]
  #[serde(alias = "analysis_package_defaults")]
  analysis_package_defaults: Option<AnalysisPackages>,
  #[serde(default)]
  studies: Vec<Study>
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ProjectsStore {
  projects: Vec<Project>
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Study {
  id: String,
  title: String,
  #[serde(alias = "created_at")]
  created_at: String,
  #[serde(default)]
  #[serde(alias = "folder_path")]
  folder_path: String,
  #[serde(default)]
  files: Vec<FileRef>
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FileRef {
  pub path: String,
  pub name: String,
  pub kind: String
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DbStudy {
  id: String,
  #[serde(alias = "project_id")]
  project_id: String,
  #[serde(alias = "internal_name")]
  internal_name: String,
  #[serde(alias = "paper_label")]
  paper_label: Option<String>,
  status: String,
  #[serde(alias = "folder_path")]
  folder_path: String,
  #[serde(alias = "created_at")]
  created_at: String
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Artifact {
  id: String,
  #[serde(alias = "study_id")]
  study_id: String,
  kind: String,
  value: String,
  label: Option<String>,
  #[serde(alias = "created_at")]
  created_at: String
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct StudyDetail {
  study: DbStudy,
  artifacts: Vec<Artifact>
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct RootDirInfo {
  exists: bool,
  is_git_repo: bool
}

fn app_root(app: &AppHandle) -> Result<PathBuf, String> {
  let base = tauri::api::path::app_data_dir(&app.config())
    .ok_or_else(|| "Unable to resolve app data dir".to_string())?;
  let root = base.join("research-workflow");
  fs::create_dir_all(&root).map_err(|err| err.to_string())?;
  Ok(root)
}

fn db_path(app: &AppHandle) -> Result<PathBuf, String> {
  let root = app_root(app)?;
  Ok(root.join("db.sqlite3"))
}

fn projects_path(app: &AppHandle) -> Result<PathBuf, String> {
  let root = app_root(app)?;
  Ok(root.join("projects.json"))
}

fn connection(app: &AppHandle) -> Result<Connection, String> {
  let path = db_path(app)?;
  Connection::open(path).map_err(|err| err.to_string())
}

fn init_schema(conn: &Connection) -> Result<(), String> {
  conn.execute_batch(
    "CREATE TABLE IF NOT EXISTS projects (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        root_path TEXT NOT NULL,
        created_at TEXT NOT NULL
      );
      CREATE TABLE IF NOT EXISTS studies (
        id TEXT PRIMARY KEY,
        project_id TEXT NOT NULL,
        internal_name TEXT NOT NULL,
        paper_label TEXT,
        status TEXT NOT NULL,
        folder_path TEXT NOT NULL,
        created_at TEXT NOT NULL,
        FOREIGN KEY(project_id) REFERENCES projects(id)
      );
      CREATE INDEX IF NOT EXISTS idx_studies_project ON studies(project_id);
      CREATE TABLE IF NOT EXISTS artifacts (
        id TEXT PRIMARY KEY,
        study_id TEXT NOT NULL,
        kind TEXT NOT NULL,
        value TEXT NOT NULL,
        label TEXT,
        created_at TEXT NOT NULL,
        FOREIGN KEY(study_id) REFERENCES studies(id)
      );
      CREATE INDEX IF NOT EXISTS idx_artifacts_study ON artifacts(study_id);"
  )
  .map_err(|err| err.to_string())?;
  Ok(())
}

fn now_string() -> String {
  Utc::now().to_rfc3339()
}

fn is_valid_study_folder(value: &str) -> bool {
  let mut chars = value.chars();
  if chars.next() != Some('S') || chars.next() != Some('-') {
    return false;
  }
  let rest: Vec<char> = chars.collect();
  if rest.len() != 6 {
    return false;
  }
  rest.iter().all(|ch| ch.is_ascii_alphanumeric())
}

fn generate_study_code() -> String {
  let raw = Uuid::new_v4().simple().to_string().to_uppercase();
  format!("S-{}", &raw[..6])
}

fn read_projects_store(app: &AppHandle) -> Result<ProjectsStore, String> {
  let path = projects_path(app)?;
  if !path.exists() {
    return Ok(ProjectsStore { projects: Vec::new() });
  }
  let raw = fs::read_to_string(&path).map_err(|err| err.to_string())?;
  if raw.trim().is_empty() {
    return Ok(ProjectsStore { projects: Vec::new() });
  }
  let mut store: ProjectsStore =
    serde_json::from_str(&raw).map_err(|err| err.to_string())?;
  for project in &mut store.projects {
    if project.updated_at.is_empty() {
      project.updated_at = project.created_at.clone();
    }
  }
  Ok(store)
}

fn write_projects_store(app: &AppHandle, store: &ProjectsStore) -> Result<(), String> {
  let path = projects_path(app)?;
  let payload = serde_json::to_string_pretty(store).map_err(|err| err.to_string())?;
  fs::write(path, payload).map_err(|err| err.to_string())?;
  Ok(())
}

fn migrate_sqlite_projects(app: &AppHandle) -> Result<(), String> {
  let db = db_path(app)?;
  if !db.exists() {
    return Ok(());
  }

  let conn = Connection::open(db).map_err(|err| err.to_string())?;
  let table_exists: i64 = conn
    .query_row(
      "SELECT COUNT(1) FROM sqlite_master WHERE type='table' AND name='projects'",
      [],
      |row| row.get(0)
    )
    .map_err(|err| err.to_string())?;
  if table_exists == 0 {
    return Ok(());
  }

  let mut stmt = conn
    .prepare("SELECT id, name, root_path, created_at FROM projects")
    .map_err(|err| err.to_string())?;
  let rows = stmt
    .query_map([], |row| {
      Ok(Project {
        id: row.get(0)?,
        name: row.get(1)?,
        root_path: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(3)?,
        google_drive_url: None,
        analysis_package_defaults: None,
        studies: Vec::new()
      })
    })
    .map_err(|err| err.to_string())?;

  let mut sqlite_projects = Vec::new();
  for row in rows {
    sqlite_projects.push(row.map_err(|err| err.to_string())?);
  }
  if sqlite_projects.is_empty() {
    return Ok(());
  }

  let mut store = read_projects_store(app)?;
  let mut added = 0;
  for project in sqlite_projects {
    if !store.projects.iter().any(|p| p.id == project.id) {
      store.projects.push(project);
      added += 1;
    }
  }
  if added > 0 {
    write_projects_store(app, &store)?;
    println!("migration: imported {} project(s) from sqlite", added);
  } else {
    println!("migration: no new projects to import from sqlite");
  }

  Ok(())
}

fn ensure_folders(root: &Path, folders: &[&str]) -> Result<(), String> {
  for folder in folders {
    fs::create_dir_all(root.join(folder)).map_err(|err| err.to_string())?;
  }
  Ok(())
}

fn resolve_study_root(project: &Project, study: &Study) -> PathBuf {
  if study.folder_path.trim().is_empty() {
    PathBuf::from(project.root_path.clone())
      .join("studies")
      .join(&study.id)
  } else {
    PathBuf::from(study.folder_path.clone())
  }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AnalysisPackages {
  cleaning: Vec<String>,
  plot: Vec<String>,
  table: Vec<String>,
  analysis: Vec<String>
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelLayout {
  name: String,
  model_type: String,
  outcome_var: String,
  #[serde(default)]
  treatment_var: Option<String>,
  layout: String,
  #[serde(default)]
  interaction_var: Option<String>,
  #[serde(default)]
  covariates: Option<String>,
  #[serde(default)]
  id_var: Option<String>,
  #[serde(default)]
  time_var: Option<String>,
  #[serde(default)]
  figures: Vec<String>,
  #[serde(default)]
  include_in_main_table: bool
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AnalysisTemplateOptions {
  analysis_file_name: Option<String>,
  data_source_paths: Option<Vec<String>>,
  dataset_path_hint: Option<String>,
  outcome_var_hint: Option<String>,
  treatment_var_hint: Option<String>,
  id_var_hint: Option<String>,
  time_var_hint: Option<String>,
  group_var_hint: Option<String>,
  descriptives: Vec<String>,
  plots: Vec<String>,
  balance_checks: Vec<String>,
  models: Vec<String>,
  diagnostics: Vec<String>,
  tables: Vec<String>,
  robustness: Vec<String>,
  #[serde(default)]
  model_layouts: Vec<ModelLayout>,
  exploratory: bool,
  export_artifacts: bool
}

fn add_package(packages: &mut Vec<String>, value: &str) {
  if !packages.iter().any(|item| item == value) {
    packages.push(value.to_string());
  }
}

fn selected(values: &[String], key: &str) -> bool {
  values.iter().any(|value| value == key)
}

fn collect_model_types(options: &AnalysisTemplateOptions) -> Vec<String> {
  let mut out: Vec<String> = Vec::new();
  for layout in &options.model_layouts {
    if !layout.model_type.trim().is_empty() && !out.iter().any(|m| m == &layout.model_type) {
      out.push(layout.model_type.clone());
    }
  }
  out
}

fn selected_model(options: &AnalysisTemplateOptions, key: &str) -> bool {
  collect_model_types(options).iter().any(|value| value == key)
}

fn model_outcomes(options: &AnalysisTemplateOptions, fallback: &str) -> Vec<String> {
  let mut out: Vec<String> = Vec::new();
  for layout in &options.model_layouts {
    let value = layout.outcome_var.trim();
    if !value.is_empty() && !out.iter().any(|item| item == value) {
      out.push(value.to_string());
    }
  }
  if out.is_empty() {
    out.push(fallback.to_string());
  }
  out
}

fn primary_treatment_from_models(options: &AnalysisTemplateOptions, fallback: &str) -> String {
  for layout in &options.model_layouts {
    if let Some(value) = &layout.treatment_var {
      let trimmed = value.trim();
      if !trimmed.is_empty() {
        return trimmed.to_string();
      }
    }
  }
  fallback.to_string()
}

fn primary_group_from_models(options: &AnalysisTemplateOptions, fallback: &str) -> String {
  for layout in &options.model_layouts {
    if let Some(value) = &layout.interaction_var {
      let trimmed = value.trim();
      if !trimmed.is_empty() {
        return trimmed.to_string();
      }
    }
  }
  fallback.to_string()
}

fn safe_token(value: &str, fallback: &str) -> String {
  let mut out = String::new();
  for c in value.chars() {
    if c.is_ascii_alphanumeric() || c == '_' {
      out.push(c);
    } else {
      out.push('_');
    }
  }
  let out = out.trim_matches('_').to_string();
  if out.is_empty() { fallback.to_string() } else { out }
}

fn hint_or_default(value: &Option<String>, fallback: &str) -> String {
  value
    .as_ref()
    .map(|item| item.trim())
    .filter(|item| !item.is_empty())
    .unwrap_or(fallback)
    .to_string()
}

fn analysis_output_here_expr(project_root: &Path, study_root: &Path) -> String {
  let output_root = study_root.join("07_outputs");
  if let Some(rel) = diff_paths(&output_root, project_root) {
    let parts: Vec<String> = rel
      .components()
      .map(|component| component.as_os_str().to_string_lossy().replace('"', "\\\""))
      .collect();
    if !parts.is_empty() {
      return format!(
        "here::here({})",
        parts
          .iter()
          .map(|item| format!("\"{item}\""))
          .collect::<Vec<String>>()
          .join(", ")
      );
    }
  }
  let absolute = output_root
    .to_string_lossy()
    .replace('\\', "/")
    .replace('"', "\\\"");
  format!("\"{absolute}\"")
}

fn normalized_analysis_file_base(value: &Option<String>) -> Result<String, String> {
  let mut base = value
    .as_ref()
    .map(|item| item.trim().to_string())
    .unwrap_or_else(|| "analysis".to_string());
  if base.is_empty() {
    base = "analysis".to_string();
  }
  if base.to_lowercase().ends_with(".rmd") && base.len() > 4 {
    base.truncate(base.len() - 4);
  }
  if base.trim().is_empty() {
    return Err("Analysis file name cannot be empty.".to_string());
  }
  if base.contains('/') || base.contains('\\') || base.contains("..") {
    return Err("Analysis file name must be a single file name.".to_string());
  }
  Ok(base)
}

fn write_if_missing(path: &Path, content: &str) -> Result<(), String> {
  if !path.exists() {
    fs::write(path, content).map_err(|err| err.to_string())?;
  }
  Ok(())
}

fn merge_missing_json_keys(
  current: &mut serde_json::Value,
  defaults: &serde_json::Value
) {
  match (current, defaults) {
    (serde_json::Value::Object(current_map), serde_json::Value::Object(default_map)) => {
      for (key, default_value) in default_map {
        match current_map.get_mut(key) {
          Some(current_value) => merge_missing_json_keys(current_value, default_value),
          None => {
            current_map.insert(key.clone(), default_value.clone());
          }
        }
      }
    }
    _ => {}
  }
}

fn ensure_analysis_defaults_config(project_root: &Path) -> Result<(), String> {
  let config_path = project_root.join(ANALYSIS_CONFIG_PATH);
  if let Some(parent) = config_path.parent() {
    fs::create_dir_all(parent).map_err(|err| err.to_string())?;
  }

  let defaults: serde_json::Value =
    serde_json::from_str(DEFAULT_ANALYSIS_CONFIG_JSON).map_err(|err| err.to_string())?;

  if !config_path.exists() {
    let payload = serde_json::to_string_pretty(&defaults).map_err(|err| err.to_string())?;
    fs::write(config_path, payload).map_err(|err| err.to_string())?;
    return Ok(());
  }

  let raw = fs::read_to_string(&config_path).map_err(|err| err.to_string())?;
  let mut existing: serde_json::Value = if raw.trim().is_empty() {
    serde_json::json!({})
  } else {
    serde_json::from_str(&raw).map_err(|err| {
      format!(
        "Existing analysis defaults config is not valid JSON at {}: {}",
        config_path.to_string_lossy(),
        err
      )
    })?
  };

  merge_missing_json_keys(&mut existing, &defaults);
  let merged = serde_json::to_string_pretty(&existing).map_err(|err| err.to_string())?;
  fs::write(config_path, merged).map_err(|err| err.to_string())?;
  Ok(())
}

fn ensure_project_style_kit(project_root: &Path) -> Result<(), String> {
  ensure_analysis_defaults_config(project_root)?;

  let style_dir = project_root.join(STYLE_KIT_DIR);
  fs::create_dir_all(&style_dir).map_err(|err| err.to_string())?;

  write_if_missing(&style_dir.join("theme_plots.R"), THEME_PLOTS_R)?;
  write_if_missing(&style_dir.join("tables_flextable.R"), TABLES_FLEXTABLE_R)?;
  write_if_missing(&style_dir.join("style_init.R"), STYLE_INIT_R)?;
  write_if_missing(&style_dir.join("README.md"), STYLE_README_MD)?;

  let pkg_dir = project_root.join(STYLE_PACKAGE_DIR);
  let pkg_r_dir = pkg_dir.join("R");
  fs::create_dir_all(&pkg_r_dir).map_err(|err| err.to_string())?;

  write_if_missing(&pkg_dir.join("DESCRIPTION"), STYLE_PACKAGE_DESCRIPTION)?;
  write_if_missing(&pkg_dir.join("NAMESPACE"), STYLE_PACKAGE_NAMESPACE)?;
  write_if_missing(&pkg_dir.join("LICENSE"), STYLE_PACKAGE_LICENSE)?;
  write_if_missing(&pkg_r_dir.join("plots.R"), STYLE_PACKAGE_PLOTS_R)?;
  write_if_missing(&pkg_r_dir.join("tables.R"), STYLE_PACKAGE_TABLES_R)?;
  write_if_missing(&pkg_r_dir.join("init.R"), STYLE_PACKAGE_INIT_R)?;
  write_if_missing(&pkg_dir.join("README.md"), STYLE_PACKAGE_README_MD)?;
  Ok(())
}

fn render_packages(options: &AnalysisTemplateOptions) -> String {
  let mut packages: Vec<String> = vec![
    "tidyverse".to_string(),
    "here".to_string(),
    "janitor".to_string(),
    "ggplot2".to_string(),
    "ggpubr".to_string(),
    "gganimate".to_string(),
    "flextable".to_string(),
    "modelsummary".to_string(),
    "broom".to_string(),
    "gt".to_string(),
    "kableExtra".to_string()
  ];

  if selected(&options.descriptives, "missingness") {
    add_package(&mut packages, "naniar");
  }
  if selected(&options.plots, "correlation_heatmap") {
    add_package(&mut packages, "reshape2");
  }
  if selected_model(options, "ols")
    || selected(&options.diagnostics, "linearity")
    || selected(&options.diagnostics, "multicollinearity")
  {
    add_package(&mut packages, "car");
  }
  if selected_model(options, "ols") || selected(&options.diagnostics, "homoskedasticity") {
    add_package(&mut packages, "lmtest");
    add_package(&mut packages, "sandwich");
    add_package(&mut packages, "performance");
  }
  if selected_model(options, "logit")
    || selected_model(options, "poisson")
    || selected_model(options, "negbin")
    || selected(&options.diagnostics, "overdispersion")
  {
    add_package(&mut packages, "performance");
    add_package(&mut packages, "pscl");
  }
  if selected_model(options, "negbin") {
    add_package(&mut packages, "MASS");
  }
  if selected_model(options, "mixed_effects") {
    add_package(&mut packages, "lme4");
    add_package(&mut packages, "broom.mixed");
  }
  if selected_model(options, "fixed_effects")
    || selected_model(options, "did")
    || selected_model(options, "event_study")
    || selected(&options.diagnostics, "parallel_trends")
  {
    add_package(&mut packages, "fixest");
  }
  if selected_model(options, "survival") {
    add_package(&mut packages, "survival");
    add_package(&mut packages, "survminer");
  }
  if selected_model(options, "rd") || selected(&options.diagnostics, "bandwidth_sensitivity") {
    add_package(&mut packages, "rdrobust");
  }

  let mut out = String::new();
  out.push_str("# Packages\n\n");
  out.push_str("```{r packages, message=FALSE, warning=FALSE}\n");
  out.push_str("# TODO: install packages as needed.\n");
  out.push_str("# install.packages(c(");
  out.push_str(
    &packages
      .iter()
      .map(|item| format!("\"{item}\""))
      .collect::<Vec<String>>()
      .join(", ")
  );
  out.push_str("))\n");
  for package in packages {
    out.push_str(&format!("library({package})\n"));
  }
  out.push_str("```\n\n");
  out
}

fn render_descriptives(
  options: &AnalysisTemplateOptions,
  outcomes: &[String],
  treatment: &str,
  group: &str
) -> String {
  if options.descriptives.is_empty() && options.plots.is_empty() {
    return String::new();
  }
  let mut out = String::new();
  out.push_str("# Descriptives\n\n");

  if selected(&options.tables, "table1_descriptives") {
    out.push_str("```{r descriptives_table1}\n");
    out.push_str("table1_descriptives_df <- modelsummary::datasummary(\n");
    out.push_str("  as.formula(\"");
    out.push_str(
      &outcomes
        .iter()
        .map(|item| item.replace('"', "\\\""))
        .collect::<Vec<String>>()
        .join(" + ")
    );
    out.push_str(" ~ ");
    out.push_str(&group.replace('"', "\\\""));
    out.push_str(" * (Mean + SD)\"),\n");
    out.push_str("  df,\n");
    out.push_str("  output = \"data.frame\"\n");
    out.push_str(")\n");
    out.push_str("table1_descriptives_ft <- ft_apa(table1_descriptives_df)\n");
    out.push_str("table1_descriptives_ft\n");
    out.push_str("```\n\n");
  }

  if selected(&options.descriptives, "summary_stats") {
    out.push_str("```{r descriptives_summary_stats}\n");
    out.push_str("summary_stats_ft <- ft_apa_descriptives(\n");
    out.push_str("  df,\n");
    out.push_str("  digits = 2\n");
    out.push_str(")\n");
    out.push_str("summary_stats_ft\n");
    out.push_str("```\n\n");
  }
  if selected(&options.descriptives, "counts") {
    out.push_str("```{r descriptives_counts}\n");
    out.push_str("n_obs <- nrow(df)\n");
    out.push_str(&format!("n_ids <- dplyr::n_distinct(df${treatment})\n"));
    out.push_str(&format!("counts_by_group <- df %>% count({treatment})\n"));
    out.push_str("counts_tbl <- tibble::tibble(\n");
    out.push_str("  Metric = c(\"N observations\", \"N IDs\"),\n");
    out.push_str("  Value = c(n_obs, n_ids)\n");
    out.push_str(")\n");
    out.push_str("ft_apa(counts_tbl)\n");
    out.push_str("ft_apa(counts_by_group)\n");
    out.push_str("```\n\n");
  }
  if selected(&options.descriptives, "missingness") {
    out.push_str("```{r descriptives_missingness}\n");
    out.push_str("missing_summary <- naniar::miss_var_summary(df)\n");
    out.push_str("missing_summary\n");
    out.push_str("```\n\n");
  }
  if selected(&options.descriptives, "group_summary") {
    out.push_str("```{r descriptives_group_summary}\n");
    out.push_str(&format!("group_summary <- df %>% group_by({group}) %>%\n"));
    out.push_str("  summarise(across(where(is.numeric), ~mean(.x, na.rm = TRUE)), .groups = \"drop\")\n");
    out.push_str("ft_apa(group_summary)\n");
    out.push_str("```\n\n");
  }
  if selected(&options.descriptives, "correlations") {
    out.push_str("```{r descriptives_correlations}\n");
    out.push_str("cor_matrix <- df %>% select(where(is.numeric)) %>% cor(use = \"pairwise.complete.obs\")\n");
    out.push_str("cor_matrix\n");
    out.push_str("```\n\n");
  }

  if selected(&options.plots, "histogram") {
    for outcome in outcomes {
      let token = safe_token(outcome, "outcome");
      out.push_str(&format!("```{{r descriptives_plot_histogram_{token}}}\n"));
      out.push_str(&format!("p_hist_{token} <- apa_hist(df, {outcome}, bins = 30)\n"));
      out.push_str(&format!("p_hist_{token}\n"));
      out.push_str("```\n\n");
    }
  }
  if selected(&options.plots, "boxplot") {
    for outcome in outcomes {
      let token = safe_token(outcome, "outcome");
      out.push_str(&format!("```{{r descriptives_plot_boxplot_{token}}}\n"));
      out.push_str(&format!("p_box_{token} <- apa_box(df, {treatment}, {outcome})\n"));
      out.push_str(&format!("p_box_{token}\n"));
      out.push_str("```\n\n");
    }
  }
  if selected(&options.plots, "density") {
    for outcome in outcomes {
      let token = safe_token(outcome, "outcome");
      out.push_str(&format!("```{{r descriptives_plot_density_{token}}}\n"));
      out.push_str(&format!("p_density_{token} <- ggplot(df, aes(x = {outcome})) + geom_density()\n"));
      out.push_str(&format!("p_density_{token}\n"));
      out.push_str("```\n\n");
    }
  }
  if selected(&options.plots, "scatter") {
    for outcome in outcomes {
      let token = safe_token(outcome, "outcome");
      out.push_str(&format!("```{{r descriptives_plot_scatter_{token}}}\n"));
      out.push_str(&format!("p_scatter_{token} <- apa_scatter(df, {treatment}, {outcome}, add_lm = TRUE)\n"));
      out.push_str(&format!("p_scatter_{token}\n"));
      out.push_str("```\n\n");
    }
  }
  if selected(&options.plots, "qqplot") {
    for outcome in outcomes {
      let token = safe_token(outcome, "outcome");
      out.push_str(&format!("```{{r descriptives_plot_qq_{token}}}\n"));
      out.push_str(&format!("p_qq_{token} <- ggplot(df, aes(sample = {outcome})) + stat_qq() + stat_qq_line()\n"));
      out.push_str(&format!("p_qq_{token}\n"));
      out.push_str("```\n\n");
    }
  }
  if selected(&options.plots, "correlation_heatmap") {
    out.push_str("```{r descriptives_plot_corr_heatmap}\n");
    out.push_str("cor_matrix <- df %>% select(where(is.numeric)) %>% cor(use = \"pairwise.complete.obs\")\n");
    out.push_str("cor_long <- reshape2::melt(cor_matrix)\n");
    out.push_str("p_corr <- ggplot(cor_long, aes(x = Var1, y = Var2, fill = value)) +\n");
    out.push_str("  geom_tile() +\n");
    out.push_str("  scale_fill_gradient2()\n");
    out.push_str("p_corr\n");
    out.push_str("```\n\n");
  }

  out
}

fn render_balance_checks(options: &AnalysisTemplateOptions, treatment: &str) -> String {
  if options.balance_checks.is_empty() {
    return String::new();
  }
  let mut out = String::new();
  out.push_str("# Balance Checks\n\n");

  if selected(&options.balance_checks, "baseline_table") {
    out.push_str("```{r balance_baseline_table}\n");
    out.push_str(&format!("baseline_tbl <- modelsummary::datasummary_balance(~ {treatment}, data = df)\n"));
    out.push_str("baseline_tbl\n");
    out.push_str("```\n\n");
  }
  if selected(&options.balance_checks, "std_diff") {
    out.push_str("```{r balance_std_diff}\n");
    out.push_str("std_diff <- function(x, g) {\n");
    out.push_str("  m1 <- mean(x[g == 1], na.rm = TRUE)\n");
    out.push_str("  m0 <- mean(x[g == 0], na.rm = TRUE)\n");
    out.push_str("  s <- sd(x, na.rm = TRUE)\n");
    out.push_str("  (m1 - m0) / s\n");
    out.push_str("}\n");
    out.push_str(&format!("# TODO: apply std_diff across covariates using {treatment}\n"));
    out.push_str("```\n\n");
  }
  if selected(&options.balance_checks, "randomization_check") {
    out.push_str("```{r balance_randomization_check}\n");
    out.push_str("# TODO: regress baseline covariates on treatment and inspect joint significance.\n");
    out.push_str(&format!("# lm(covariate ~ {treatment}, data = df)\n"));
    out.push_str("```\n\n");
  }

  out
}

fn render_models(options: &AnalysisTemplateOptions, _outcome: &str, treatment: &str, id: &str, time: &str) -> String {
  #[derive(Clone)]
  struct ModelPlan {
    name: String,
    model_type: String,
    outcome_var: String,
    treatment_var: String,
    layout: String,
    interaction_var: String,
    covariates: String,
    id_var: String,
    time_var: String,
    figures: Vec<String>,
    include_in_main_table: bool
  }

  let mut out = String::new();
  out.push_str("# Main Analyses\n\n");

  let mut plans: Vec<ModelPlan> = Vec::new();
  for (idx, layout) in options.model_layouts.iter().enumerate() {
    let outcome_var = layout.outcome_var.trim();
    if outcome_var.is_empty() {
      continue;
    }
    let model_type = layout.model_type.trim().to_string();
    if model_type.is_empty() {
      continue;
    }
    let name = if layout.name.trim().is_empty() {
      format!("model_{}", idx + 1)
    } else {
      layout.name.trim().to_string()
    };
    plans.push(ModelPlan {
      name,
      model_type,
      outcome_var: outcome_var.to_string(),
      treatment_var: layout
        .treatment_var
        .as_ref()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| treatment.to_string()),
      layout: layout.layout.trim().to_string(),
      interaction_var: layout.interaction_var.clone().unwrap_or_default(),
      covariates: layout.covariates.clone().unwrap_or_default(),
      id_var: layout
        .id_var
        .as_ref()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| id.to_string()),
      time_var: layout
        .time_var
        .as_ref()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| time.to_string()),
      figures: layout.figures.clone(),
      include_in_main_table: layout.include_in_main_table
    });
  }

  if plans.is_empty() {
    out.push_str("```{r model_none}\n");
    out.push_str("# TODO: Add at least one Model Layout in the model builder to generate analyses.\n");
    out.push_str("```\n\n");
    return out;
  }

  out.push_str("```{r model_registry_init}\n");
  out.push_str("model_registry <- list()\n");
  out.push_str("model_metadata <- tibble::tibble(\n");
  out.push_str("  model_name = character(),\n");
  out.push_str("  model_object = character(),\n");
  out.push_str("  outcome = character(),\n");
  out.push_str("  include_main_table = logical(),\n");
  out.push_str("  main_figure = character()\n");
  out.push_str(")\n");
  out.push_str("```\n\n");

  use std::collections::BTreeMap;
  let mut by_outcome: BTreeMap<String, Vec<(String, String, bool, String)>> = BTreeMap::new();
  let mut figure_plans: Vec<(String, String, String, String)> = Vec::new();
  for (idx, plan) in plans.iter().enumerate() {
    let model_object = format!("m_{}", idx + 1);
    let chunk_id = safe_token(
      &format!("model_{}_{}", idx + 1, plan.name.to_lowercase()),
      &format!("model_{}", idx + 1)
    );
    let outcome_var = plan.outcome_var.replace('"', "\\\"");
    let covariates = plan.covariates.trim();
    let interaction_var = if plan.interaction_var.trim().is_empty() {
      "moderator_var".to_string()
    } else {
      plan.interaction_var.trim().to_string()
    };
    let treatment_expr = plan.treatment_var.trim();
    let mut rhs = if plan.layout == "interaction" {
      format!("({}) * {}", treatment_expr, interaction_var)
    } else {
      treatment_expr.to_string()
    };
    if !covariates.is_empty() {
      rhs.push_str(" + ");
      rhs.push_str(covariates);
    }

    out.push_str(&format!(
      "## {} ({})\n\n```{{r {}}}\n",
      plan.name.replace('"', "\\\""),
      plan.model_type,
      chunk_id
    ));
    match plan.model_type.as_str() {
      "ols" => out.push_str(&format!("{} <- lm({} ~ {}, data = df)\n", model_object, outcome_var, rhs)),
      "logit" => out.push_str(&format!(
        "{} <- glm({} ~ {}, data = df, family = binomial())\n",
        model_object, outcome_var, rhs
      )),
      "poisson" => out.push_str(&format!(
        "{} <- glm({} ~ {}, data = df, family = poisson())\n",
        model_object, outcome_var, rhs
      )),
      "negbin" => out.push_str(&format!(
        "{} <- MASS::glm.nb({} ~ {}, data = df)\n",
        model_object, outcome_var, rhs
      )),
      "mixed_effects" => out.push_str(&format!(
        "{} <- lme4::lmer({} ~ {} + (1|{}), data = df)\n",
        model_object, outcome_var, rhs, plan.id_var
      )),
      "fixed_effects" => out.push_str(&format!(
        "{} <- fixest::feols({} ~ {} | {} + {}, data = df, vcov = \"cluster\")\n",
        model_object, outcome_var, rhs, plan.id_var, plan.time_var
      )),
      "survival" => out.push_str(&format!(
        "{} <- survival::coxph(Surv(time_to_event, event) ~ {}, data = df)\n",
        model_object, rhs
      )),
      "rd" => {
        out.push_str("# TODO: replace running_var and cutoff.\n");
        out.push_str(&format!(
          "{} <- rdrobust::rdrobust(y = df${}, x = df$running_var, c = 0)\n",
          model_object, outcome_var
        ));
      }
      "did" => out.push_str(&format!(
        "{} <- fixest::feols({} ~ i({}, {}, ref = 0){} | {} + {}, data = df)\n",
        model_object,
        outcome_var,
        plan.time_var,
        plan.treatment_var,
        if covariates.is_empty() { "".to_string() } else { format!(" + {covariates}") },
        plan.id_var,
        plan.time_var
      )),
      "event_study" => {
        out.push_str(&format!(
          "{} <- fixest::feols({} ~ sunab(cohort_time, {}) | {} + {}, data = df)\n",
          model_object, outcome_var, plan.time_var, plan.id_var, plan.time_var
        ));
        out.push_str("# TODO: define cohort_time for adoption timing.\n");
      }
      _ => out.push_str(&format!(
        "{} <- lm({} ~ {}, data = df)\n",
        model_object, outcome_var, rhs
      ))
    }
    out.push_str(&format!("model_registry[[\"{}\"]] <- {}\n", plan.name.replace('"', "\\\""), model_object));
    let figure_pref = plan
      .figures
      .first()
      .cloned()
      .unwrap_or_else(|| "coef_plot".to_string());
    out.push_str("model_metadata <- dplyr::bind_rows(\n");
    out.push_str("  model_metadata,\n");
    out.push_str(&format!(
      "  tibble::tibble(model_name = \"{}\", model_object = \"{}\", outcome = \"{}\", include_main_table = {}, main_figure = \"{}\")\n",
      plan.name.replace('"', "\\\""),
      model_object,
      outcome_var,
      if plan.include_in_main_table { "TRUE" } else { "FALSE" },
      figure_pref
    ));
    out.push_str(")\n");
    out.push_str("if (inherits(model_registry[[");
    out.push_str(&format!("\"{}\"", plan.name.replace('"', "\\\"")));
    out.push_str("]], c(\"lm\", \"glm\", \"fixest\", \"lmerMod\", \"coxph\"))) {\n");
    out.push_str("  print(broom::glance(model_registry[[");
    out.push_str(&format!("\"{}\"", plan.name.replace('"', "\\\"")));
    out.push_str("]]))\n");
    out.push_str("}\n");
    out.push_str("```\n\n");

    by_outcome
      .entry(plan.outcome_var.clone())
      .or_default()
      .push((
        plan.name.clone(),
        model_object.clone(),
        plan.include_in_main_table,
        figure_pref
      ));
    figure_plans.push((
      plan.name.clone(),
      model_object.clone(),
      plan.outcome_var.clone(),
      plan
        .figures
        .first()
        .cloned()
        .unwrap_or_else(|| "coef_plot".to_string())
    ));
  }

  if selected(&options.tables, "model_table") {
    out.push_str("## Main Regression Tables (Grouped by Outcome)\n\n");
    for (outcome_name, models) in &by_outcome {
      let included: Vec<(String, String)> = models
        .iter()
        .filter(|(_, _, include, _)| *include)
        .map(|(name, object, _, _)| (name.clone(), object.clone()))
        .collect();
      if included.is_empty() {
        continue;
      }
      let file_outcome = safe_token(outcome_name, "outcome");
      out.push_str(&format!("```{{r model_table_{}}}\n", file_outcome));
      out.push_str("models_for_outcome <- list(\n");
      for (idx, (name, object)) in included.iter().enumerate() {
        let suffix = if idx + 1 == included.len() { "" } else { "," };
        out.push_str(&format!("  \"{}\" = {}{}\n", name.replace('"', "\\\""), object, suffix));
      }
      out.push_str(")\n");
      out.push_str(&format!(
        "style_model_table(models_for_outcome, output_path = file.path(tables_dir, \"models_{}.html\"))\n",
        file_outcome
      ));
      out.push_str("```\n\n");
    }
  }

  out.push_str("## Main Figures by Model Builder Input\n\n");
  for (model_name, model_object, outcome_name, figure_pref) in &figure_plans {
    let chunk = safe_token(
      &format!("main_figure_{}_{}", model_name, outcome_name),
      "main_figure"
    );
    let clean_outcome = safe_token(
      &format!("{}_{}", model_name, outcome_name),
      "outcome"
    );
    out.push_str(&format!("```{{r {}}}\n", chunk));
    out.push_str(&format!("main_model <- {}\n", model_object));
    match figure_pref.as_str() {
      "fitted_plot" => {
        out.push_str("if (inherits(main_model, c(\"lm\", \"glm\"))) {\n");
        out.push_str(&format!("  p_main_{} <- ggplot(df, aes(x = fitted(main_model), y = {})) +\n", clean_outcome, outcome_name));
        out.push_str("    geom_point(alpha = 0.7) +\n");
        out.push_str("    geom_abline(slope = 1, intercept = 0, linetype = \"dashed\") +\n");
        out.push_str("    labs(x = \"Fitted\", y = \"Observed\") +\n");
        out.push_str("    theme_apa()\n");
        out.push_str(&format!("  p_main_{}\n", clean_outcome));
        out.push_str("}\n");
      }
      "residual_plot" => {
        out.push_str("if (inherits(main_model, c(\"lm\", \"glm\"))) {\n");
        out.push_str("  plot(main_model, which = 1)\n");
        out.push_str("}\n");
      }
      "event_study_plot" => {
        out.push_str("if (inherits(main_model, \"fixest\")) {\n");
        out.push_str("  fixest::iplot(main_model)\n");
        out.push_str("}\n");
      }
      _ => {
        out.push_str("if (inherits(main_model, c(\"lm\", \"glm\", \"fixest\", \"lmerMod\", \"coxph\"))) {\n");
        out.push_str("  coef_df <- broom::tidy(main_model)\n");
        out.push_str(&format!("  p_main_{} <- ggplot(coef_df, aes(x = estimate, y = term)) +\n", clean_outcome));
        out.push_str("    geom_point() +\n");
        out.push_str("    geom_errorbarh(aes(xmin = estimate - 1.96 * std.error, xmax = estimate + 1.96 * std.error), height = 0.1) +\n");
        out.push_str("    theme_apa()\n");
        out.push_str(&format!("  p_main_{}\n", clean_outcome));
        out.push_str("}\n");
      }
    }
    out.push_str("```\n\n");
  }

  out
}

fn render_diagnostics(options: &AnalysisTemplateOptions) -> String {
  if options.diagnostics.is_empty() {
    return String::new();
  }
  let mut out = String::new();
  out.push_str("# Diagnostics and Assumption Checks\n\n");
  out.push_str("```{r diagnostics_registry_guard}\n");
  out.push_str("if (!exists(\"model_registry\")) model_registry <- list()\n");
  out.push_str("```\n\n");

  if selected(&options.diagnostics, "linearity") {
    out.push_str("```{r diag_linearity}\n");
    out.push_str("for (nm in names(model_registry)) {\n");
    out.push_str("  m <- model_registry[[nm]]\n");
    out.push_str("  if (inherits(m, \"lm\")) {\n");
    out.push_str("    message(\"Linearity diagnostics: \", nm)\n");
    out.push_str("    plot(m, which = 1)\n");
    out.push_str("    car::crPlots(m)\n");
    out.push_str("  }\n");
    out.push_str("}\n");
    out.push_str("```\n\n");
  }
  if selected(&options.diagnostics, "normality_residuals") {
    out.push_str("```{r diag_normality}\n");
    out.push_str("for (nm in names(model_registry)) {\n");
    out.push_str("  m <- model_registry[[nm]]\n");
    out.push_str("  if (inherits(m, \"lm\")) {\n");
    out.push_str("    message(\"Normality diagnostics: \", nm)\n");
    out.push_str("    plot(m, which = 2)\n");
    out.push_str("  }\n");
    out.push_str("}\n");
    out.push_str("# TODO: Shapiro tests can be misleading at large N.\n");
    out.push_str("```\n\n");
  }
  if selected(&options.diagnostics, "homoskedasticity") {
    out.push_str("```{r diag_homoskedasticity}\n");
    out.push_str("for (nm in names(model_registry)) {\n");
    out.push_str("  m <- model_registry[[nm]]\n");
    out.push_str("  if (inherits(m, \"lm\")) {\n");
    out.push_str("    message(\"Homoskedasticity diagnostics: \", nm)\n");
    out.push_str("    print(lmtest::bptest(m))\n");
    out.push_str("    print(lmtest::coeftest(m, vcov = sandwich::vcovHC(m, type = \"HC1\")))\n");
    out.push_str("  }\n");
    out.push_str("}\n");
    out.push_str("```\n\n");
  }
  if selected(&options.diagnostics, "multicollinearity") {
    out.push_str("```{r diag_multicollinearity}\n");
    out.push_str("for (nm in names(model_registry)) {\n");
    out.push_str("  m <- model_registry[[nm]]\n");
    out.push_str("  if (inherits(m, \"lm\")) {\n");
    out.push_str("    message(\"VIF: \", nm)\n");
    out.push_str("    print(car::vif(m))\n");
    out.push_str("  }\n");
    out.push_str("}\n");
    out.push_str("```\n\n");
  }
  if selected(&options.diagnostics, "influential_points") {
    out.push_str("```{r diag_influential_points}\n");
    out.push_str("for (nm in names(model_registry)) {\n");
    out.push_str("  m <- model_registry[[nm]]\n");
    out.push_str("  if (inherits(m, \"lm\")) {\n");
    out.push_str("    message(\"Influence diagnostics: \", nm)\n");
    out.push_str("    plot(m, which = 4)\n");
    out.push_str("    plot(m, which = 5)\n");
    out.push_str("  }\n");
    out.push_str("}\n");
    out.push_str("```\n\n");
  }
  if selected(&options.diagnostics, "overdispersion") {
    out.push_str("```{r diag_overdispersion}\n");
    out.push_str("for (nm in names(model_registry)) {\n");
    out.push_str("  m <- model_registry[[nm]]\n");
    out.push_str("  if (inherits(m, \"glm\") && identical(stats::family(m)$family, \"poisson\")) {\n");
    out.push_str("    message(\"Overdispersion check: \", nm)\n");
    out.push_str("    print(performance::check_overdispersion(m))\n");
    out.push_str("  }\n");
    out.push_str("}\n");
    out.push_str("```\n\n");
  }
  if selected(&options.diagnostics, "parallel_trends") {
    out.push_str("```{r diag_parallel_trends}\n");
    out.push_str("# TODO: implement pre-trend test / event-study pre-period checks.\n");
    out.push_str("```\n\n");
  }
  if selected(&options.diagnostics, "common_support") {
    out.push_str("```{r diag_common_support}\n");
    out.push_str("# TODO: estimate propensity scores and plot overlap.\n");
    out.push_str("```\n\n");
  }
  if selected(&options.diagnostics, "placebo_tests") {
    out.push_str("```{r diag_placebo_tests}\n");
    out.push_str("# TODO: add placebo outcomes or pseudo-treatment timings.\n");
    out.push_str("```\n\n");
  }
  if selected(&options.diagnostics, "bandwidth_sensitivity") {
    out.push_str("```{r diag_bandwidth_sensitivity}\n");
    out.push_str("# TODO: compare RD estimates across multiple bandwidths.\n");
    out.push_str("```\n\n");
  }

  out
}

fn render_robustness(options: &AnalysisTemplateOptions) -> String {
  if options.robustness.is_empty() {
    return String::new();
  }
  let mut out = String::new();
  out.push_str("# Robustness Checks\n\n");
  for check in &options.robustness {
    out.push_str(&format!("## {}\n\n", check.replace('_', " ").to_uppercase()));
    out.push_str(&format!("```{{r robustness_{check}}}\n"));
    match check.as_str() {
      "hc_se" => {
        out.push_str("for (nm in names(model_registry)) {\n");
        out.push_str("  m <- model_registry[[nm]]\n");
        out.push_str("  if (inherits(m, \"lm\")) {\n");
        out.push_str("    print(lmtest::coeftest(m, vcov = sandwich::vcovHC(m, type = \"HC1\")))\n");
        out.push_str("  }\n");
        out.push_str("}\n");
      }
      "cluster_se" => {
        out.push_str("# TODO: set cluster variable(s).\n");
        out.push_str("for (nm in names(model_registry)) {\n");
        out.push_str("  m <- model_registry[[nm]]\n");
        out.push_str("  if (inherits(m, \"fixest\")) {\n");
        out.push_str("    print(fixest::etable(m, vcov = ~cluster_id))\n");
        out.push_str("  }\n");
        out.push_str("}\n");
      }
      "winsorize" => {
        out.push_str("# TODO: winsorize selected variables at chosen cut points.\n");
      }
      "alt_controls" => {
        out.push_str("# TODO: refit models with alternative control sets.\n");
      }
      "alt_outcome" => {
        out.push_str("# TODO: define alternative outcomes and refit models.\n");
      }
      _ => {
        out.push_str("# TODO: implement this robustness check.\n");
      }
    }
    out.push_str("```\n\n");
  }
  out
}

fn render_exploratory(options: &AnalysisTemplateOptions) -> String {
  if !options.exploratory {
    return String::new();
  }
  let mut out = String::new();
  out.push_str("# Exploratory Analyses\n\n");
  out.push_str("```{r exploratory}\n");
  out.push_str("# TODO: add subgroup analyses, heterogeneity checks, and discovery analyses.\n");
  out.push_str("```\n\n");
  out
}

fn render_exports(options: &AnalysisTemplateOptions, outcomes: &[String]) -> String {
  if !options.export_artifacts {
    return String::new();
  }
  let mut out = String::new();
  out.push_str("# Tables and Figures Export\n\n");
  out.push_str("```{r export_artifacts}\n");
  out.push_str("dir.create(tables_dir, recursive = TRUE, showWarnings = FALSE)\n");
  out.push_str("dir.create(figures_dir, recursive = TRUE, showWarnings = FALSE)\n");
  out.push_str("dir.create(reports_dir, recursive = TRUE, showWarnings = FALSE)\n");
  if selected(&options.tables, "model_table") {
    out.push_str("# Model tables are exported in Main Analyses, grouped by outcome.\n");
  }
  if selected(&options.tables, "table1_descriptives") {
    out.push_str("if (exists(\"table1_descriptives_ft\")) {\n");
    out.push_str("  flextable::save_as_docx(table1_descriptives_ft, path = file.path(tables_dir, \"table1_descriptives.docx\"))\n");
    out.push_str("} else if (exists(\"summary_stats_ft\")) {\n");
    out.push_str("  flextable::save_as_docx(summary_stats_ft, path = file.path(tables_dir, \"table1_summary_stats.docx\"))\n");
    out.push_str("}\n");
  }
  if selected(&options.tables, "balance_table") {
    out.push_str("# TODO: export balance table object.\n");
  }
  if selected(&options.tables, "marginal_effects_table") {
    out.push_str("# TODO: compute and export marginal effects table.\n");
  }
  if selected(&options.plots, "histogram") {
    for outcome in outcomes {
      let token = safe_token(outcome, "outcome");
      out.push_str(&format!(
        "if (exists(\"p_hist_{}\")) ggsave(file.path(figures_dir, \"hist_{}.png\"), plot = p_hist_{}, width = 7, height = 5, dpi = 300)\n",
        token, token, token
      ));
    }
  }
  out.push_str("if (exists(\"model_metadata\") && nrow(model_metadata) > 0) {\n");
  out.push_str("  for (i in seq_len(nrow(model_metadata))) {\n");
  out.push_str("    mn <- model_metadata$model_name[[i]]\n");
  out.push_str("    oc <- model_metadata$outcome[[i]]\n");
  out.push_str("    key <- paste(mn, oc, sep = \"_\")\n");
  out.push_str("    key_safe <- gsub(\"[^A-Za-z0-9_]+\", \"_\", key)\n");
  out.push_str("    obj <- get0(paste0(\"p_main_\", key_safe), ifnotfound = NULL)\n");
  out.push_str("    if (!is.null(obj)) {\n");
  out.push_str("      ggsave(file.path(figures_dir, paste0(\"main_figure_\", key_safe, \".png\")), plot = obj, width = 7, height = 5, dpi = 300)\n");
  out.push_str("    }\n");
  out.push_str("  }\n");
  out.push_str("}\n");
  if selected(&options.plots, "coef_plot") {
    out.push_str("# Coefficient-style figures are generated per outcome in Main Analyses.\n");
  }
  out.push_str("# TODO: save knitted reports (html/pdf/docx) to reports_dir when rendering.\n");
  out.push_str("```\n\n");
  out
}

fn render_analysis_rmd(
  project_root: &Path,
  study_root: &Path,
  study_id: &str,
  study_title: &str,
  options: &AnalysisTemplateOptions
) -> String {
  let dataset_path = hint_or_default(&options.dataset_path_hint, "data/clean/analysis.csv");
  let data_sources: Vec<String> = options
    .data_source_paths
    .as_ref()
    .map(|values| {
      values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(|value| value.replace('\\', "/"))
        .collect::<Vec<String>>()
    })
    .unwrap_or_default();
  let hinted_outcome = hint_or_default(&options.outcome_var_hint, "y");
  let treatment_hint = hint_or_default(&options.treatment_var_hint, "treat");
  let treatment = primary_treatment_from_models(options, &treatment_hint);
  let outcomes = model_outcomes(options, &hinted_outcome);
  let outcome = outcomes.first().cloned().unwrap_or_else(|| hinted_outcome.clone());
  let id = hint_or_default(&options.id_var_hint, "id");
  let time = hint_or_default(&options.time_var_hint, "time");
  let group_hint = options
    .group_var_hint
    .as_ref()
    .map(|item| item.trim().to_string())
    .filter(|item| !item.is_empty())
    .unwrap_or_else(|| treatment.clone());
  let group = primary_group_from_models(options, &group_hint);

  let mut out = String::new();
  out.push_str("---\n");
  out.push_str(&format!(
    "title: \"Analysis: {}\"\n",
    study_title.replace('"', "\\\"")
  ));
  out.push_str("output:\n");
  out.push_str("  html_document:\n");
  out.push_str("    toc: true\n");
  out.push_str("    toc_depth: 3\n");
  out.push_str("    df_print: paged\n");
  out.push_str("---\n\n");
  out.push_str(&format!("Study ID: `{study_id}`\n\n"));

  out.push_str("# Setup\n\n");
  out.push_str("```{r setup, include=FALSE}\n");
  out.push_str("knitr::opts_chunk$set(\n");
  out.push_str("  echo = TRUE,\n");
  out.push_str("  message = FALSE,\n");
  out.push_str("  warning = FALSE,\n");
  out.push_str("  fig.retina = 2,\n");
  out.push_str("  dpi = 300,\n");
  out.push_str("  fig.width = 6.5,\n");
  out.push_str("  fig.height = 4.5\n");
  out.push_str(")\n\n");
  out.push_str("suppressPackageStartupMessages({\n");
  out.push_str("  library(here)\n");
  out.push_str("  library(tidyverse)\n");
  out.push_str("  library(ggplot2)\n");
  out.push_str("  library(ggpubr)\n");
  out.push_str("  library(flextable)\n");
  out.push_str("})\n\n");
  out.push_str("# Load project style package (preferred), fallback to sourced scripts\n");
  out.push_str("style_pkg_name <- \"");
  out.push_str(STYLE_PACKAGE_NAME);
  out.push_str("\"\n");
  out.push_str("style_pkg_root <- here::here(\"R\", \"");
  out.push_str(STYLE_PACKAGE_NAME);
  out.push_str("\")\n");
  out.push_str("style_pkg_loaded <- requireNamespace(style_pkg_name, quietly = TRUE)\n");
  out.push_str("if (!style_pkg_loaded && dir.exists(style_pkg_root) && requireNamespace(\"remotes\", quietly = TRUE)) {\n");
  out.push_str("  tryCatch({\n");
  out.push_str("    remotes::install_local(style_pkg_root, dependencies = FALSE, upgrade = \"never\", quiet = TRUE)\n");
  out.push_str("    style_pkg_loaded <- requireNamespace(style_pkg_name, quietly = TRUE)\n");
  out.push_str("  }, error = function(e) {\n");
  out.push_str("    message(\"Style package install skipped: \", conditionMessage(e))\n");
  out.push_str("  })\n");
  out.push_str("}\n");
  out.push_str("if (!style_pkg_loaded) {\n");
  out.push_str("  source(here::here(\"R/style/theme_plots.R\"))\n");
  out.push_str("  source(here::here(\"R/style/tables_flextable.R\"))\n");
  out.push_str("  source(here::here(\"R/style/style_init.R\"))\n");
  out.push_str("  cfg <- init_project_style()\n");
  out.push_str("} else {\n");
  out.push_str("  cfg <- getExportedValue(style_pkg_name, \"init_project_style\")()\n");
  out.push_str("}\n\n");
  out.push_str("# Bind plotting/table helpers from local style package when available\n");
  out.push_str("if (style_pkg_loaded) {\n");
  out.push_str("  style_exports <- c(\n");
  out.push_str("    \"theme_apa\", \"set_apa_plot_defaults\", \"apa_scatter\", \"apa_hist\", \"apa_box\",\n");
  out.push_str("    \"theme_study_plot\", \"style_box_plot\", \"style_bar_plot\",\n");
  out.push_str("    \"ft_apa\", \"ft_apa_descriptives\", \"ft_apa_regression\", \"style_model_table\"\n");
  out.push_str("  )\n");
  out.push_str("  for (fn in style_exports) {\n");
  out.push_str("    assign(fn, getExportedValue(style_pkg_name, fn), envir = .GlobalEnv)\n");
  out.push_str("  }\n");
  out.push_str("}\n\n");
  out.push_str(&format!(
    "output_dir <- {}\n",
    analysis_output_here_expr(project_root, study_root)
  ));
  out.push_str("tables_dir <- file.path(output_dir, \"tables\")\n");
  out.push_str("figures_dir <- file.path(output_dir, \"figures\")\n");
  out.push_str("reports_dir <- file.path(output_dir, \"reports\")\n");
  out.push_str("dir.create(tables_dir, recursive = TRUE, showWarnings = FALSE)\n");
  out.push_str("dir.create(figures_dir, recursive = TRUE, showWarnings = FALSE)\n");
  out.push_str("dir.create(reports_dir, recursive = TRUE, showWarnings = FALSE)\n");
  out.push_str("```\n\n");

  out.push_str(&render_packages(options));

  out.push_str("# Data Import and Cleaning\n\n");
  out.push_str("```{r load_data}\n");
  if data_sources.is_empty() {
    out.push_str(&format!("raw <- readr::read_csv(\"{}\")\n", dataset_path.replace('"', "\\\"")));
  } else {
    out.push_str("read_data_source <- function(path) {\n");
    out.push_str("  ext <- tolower(tools::file_ext(path))\n");
    out.push_str("  if (ext %in% c(\"csv\")) return(readr::read_csv(path, show_col_types = FALSE))\n");
    out.push_str("  if (ext %in% c(\"tsv\", \"txt\")) return(readr::read_tsv(path, show_col_types = FALSE))\n");
    out.push_str("  if (ext %in% c(\"rds\")) return(readr::read_rds(path))\n");
    out.push_str("  stop(paste(\"Unsupported data source extension for:\", path))\n");
    out.push_str("}\n");
    out.push_str("data_sources <- c(\n");
    for (index, source) in data_sources.iter().enumerate() {
      let sep = if index + 1 == data_sources.len() { "" } else { "," };
      out.push_str(&format!("  \"{}\"{}\n", source.replace('"', "\\\""), sep));
    }
    out.push_str(")\n");
    out.push_str("loaded_data <- purrr::set_names(data_sources, basename(data_sources)) %>%\n");
    out.push_str("  purrr::map(read_data_source)\n");
    out.push_str("if (length(loaded_data) == 1) {\n");
    out.push_str("  raw <- loaded_data[[1]]\n");
    out.push_str("} else {\n");
    out.push_str("  # TODO: Replace bind_rows() with study-specific joins if sources differ in structure.\n");
    out.push_str("  raw <- dplyr::bind_rows(loaded_data, .id = \"source_file\")\n");
    out.push_str("}\n");
  }
  out.push_str("```\n\n");
  out.push_str("```{r clean_data}\n");
  out.push_str("df <- raw %>%\n");
  out.push_str("  janitor::clean_names() %>%\n");
  out.push_str("  # TODO: add study-specific cleaning steps\n");
  out.push_str("  mutate()\n");
  out.push_str("```\n\n");

  out.push_str(&render_descriptives(options, &outcomes, &treatment, &group));
  out.push_str(&render_balance_checks(options, &treatment));
  out.push_str(&render_models(options, &outcome, &treatment, &id, &time));
  out.push_str(&render_diagnostics(options));
  out.push_str(&render_robustness(options));
  out.push_str(&render_exploratory(options));
  out.push_str(&render_exports(options, &outcomes));

  out
}

fn create_analysis_template_in_dir(
  project_root: &Path,
  study_root: &Path,
  analysis_dir: &Path,
  study_id: &str,
  study_title: &str,
  options: &AnalysisTemplateOptions
) -> Result<PathBuf, String> {
  fs::create_dir_all(analysis_dir).map_err(|err| err.to_string())?;
  let output_root = study_root.join("07_outputs");
  fs::create_dir_all(output_root.join("tables")).map_err(|err| err.to_string())?;
  fs::create_dir_all(output_root.join("figures")).map_err(|err| err.to_string())?;
  fs::create_dir_all(output_root.join("reports")).map_err(|err| err.to_string())?;

  let file_base = normalized_analysis_file_base(&options.analysis_file_name)?;
  let mut template_path = analysis_dir.join(format!("{file_base}.Rmd"));
  if template_path.exists() {
    let stamp = Utc::now().format("%Y%m%d_%H%M%S");
    template_path = analysis_dir.join(format!("{file_base}_{stamp}.Rmd"));
  }

  let template = render_analysis_rmd(project_root, study_root, study_id, study_title, options);
  fs::write(&template_path, template).map_err(|err| err.to_string())?;
  Ok(template_path)
}

fn kind_from_ext(ext: Option<&OsStr>) -> String {
  let value = ext
    .and_then(|value| value.to_str())
    .unwrap_or("")
    .to_lowercase();
  match value.as_str() {
    "pdf" => "pdf".to_string(),
    "md" | "markdown" => "md".to_string(),
    "txt" => "txt".to_string(),
    "doc" | "docx" => "docx".to_string(),
    "csv" => "csv".to_string(),
    "json" => "json".to_string(),
    "png" => "png".to_string(),
    "jpg" | "jpeg" => "jpg".to_string(),
    _ => "other".to_string()
  }
}

fn unique_dest_path(dest_dir: &Path, filename: &OsStr) -> PathBuf {
  let candidate = dest_dir.join(filename);
  if !candidate.exists() {
    return candidate;
  }

  let filename_str = filename.to_string_lossy();
  let path = Path::new(&*filename_str);
  let stem = path
    .file_stem()
    .and_then(|value| value.to_str())
    .unwrap_or("file");
  let ext = path.extension().and_then(|value| value.to_str()).unwrap_or("");
  let ext_suffix = if ext.is_empty() {
    String::new()
  } else {
    format!(".{ext}")
  };

  for index in 1..=10_000 {
    let next = format!("{stem} ({index}){ext_suffix}");
    let candidate = dest_dir.join(next);
    if !candidate.exists() {
      return candidate;
    }
  }

  candidate
}

fn move_file_cross_device(src: &Path, dst: &Path) -> Result<(), String> {
  if src == dst {
    return Ok(());
  }
  match fs::rename(src, dst) {
    Ok(()) => Ok(()),
    Err(_) => {
      fs::copy(src, dst).map_err(|err| err.to_string())?;
      fs::remove_file(src).map_err(|err| err.to_string())?;
      Ok(())
    }
  }
}

fn should_skip(path: &Path, include_pilots: bool, condensed: bool) -> bool {
  let path_str = path.to_string_lossy().to_lowercase();
  if path_str.contains("08_osf_release") {
    return true;
  }
  if path_str.contains("/.git") || path_str.contains("node_modules") {
    return true;
  }
  if !include_pilots && (path_str.contains("/pilots/") || path_str.contains("pilot")) {
    return true;
  }
  if condensed {
    if path_str.contains("/raw/")
      || path_str.contains("raw_data")
      || path_str.contains("03_data/raw")
    {
      return true;
    }
  }
  false
}

fn copy_dir_filtered(
  src: &Path,
  dst: &Path,
  include_pilots: bool,
  condensed: bool
) -> Result<u64, String> {
  if should_skip(src, include_pilots, condensed) {
    return Ok(0);
  }

  if !dst.exists() {
    fs::create_dir_all(dst).map_err(|err| err.to_string())?;
  }

  let mut copied = 0;
  for entry in fs::read_dir(src).map_err(|err| err.to_string())? {
    let entry = entry.map_err(|err| err.to_string())?;
    let path = entry.path();
    if should_skip(&path, include_pilots, condensed) {
      continue;
    }
    let target = dst.join(entry.file_name());
    if path.is_dir() {
      copied += copy_dir_filtered(&path, &target, include_pilots, condensed)?;
    } else if path.is_file() {
      if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
      }
      fs::copy(&path, &target).map_err(|err| err.to_string())?;
      copied += 1;
    }
  }
  Ok(copied)
}

#[tauri::command]
fn init_db(app: AppHandle) -> Result<(), String> {
  let conn = connection(&app)?;
  init_schema(&conn)?;
  Ok(())
}

#[tauri::command]
fn list_projects(app: AppHandle) -> Result<Vec<Project>, String> {
  migrate_sqlite_projects(&app)?;
  let mut store = read_projects_store(&app)?;
  store
    .projects
    .sort_by(|a, b| b.created_at.cmp(&a.created_at));
  Ok(store.projects)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateProjectArgs {
  name: String,
  root_dir: String,
  #[serde(default)]
  use_existing_root: bool,
  google_drive_url: Option<String>
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateProjectRootArgs {
  project_id: String,
  root_dir: String
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteProjectArgs {
  project_id: String,
  #[serde(default)]
  delete_on_disk: bool
}

#[tauri::command]
fn create_project(app: AppHandle, args: CreateProjectArgs) -> Result<Project, String> {
  let id = Uuid::new_v4().to_string();
  let trimmed_name = args.name.trim();
  if trimmed_name.is_empty() {
    return Err("Project name is required.".to_string());
  }
  let root_dir_path = PathBuf::from(args.root_dir.trim());
  if !root_dir_path.exists() || !root_dir_path.is_dir() {
    return Err("Project root location must be an existing folder.".to_string());
  }

  let root = if args.use_existing_root {
    root_dir_path
  } else {
    let root = root_dir_path.join(trimmed_name);
    if root.exists() {
      return Err("Project folder already exists.".to_string());
    }
    root
  };
  ensure_folders(&root, PROJECT_FOLDERS)?;

  let project = Project {
    id: id.clone(),
    name: trimmed_name.to_string(),
    root_path: root.to_string_lossy().to_string(),
    created_at: now_string(),
    updated_at: now_string(),
    google_drive_url: args.google_drive_url
      .and_then(|value| {
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() {
          None
        } else {
          Some(trimmed)
        }
      }),
    analysis_package_defaults: None,
    studies: Vec::new()
  };

  let mut store = read_projects_store(&app)?;
  store.projects.push(project.clone());
  write_projects_store(&app, &store)?;

  Ok(project)
}

#[tauri::command]
fn update_project_root(app: AppHandle, args: UpdateProjectRootArgs) -> Result<Project, String> {
  let root_dir_path = PathBuf::from(args.root_dir.trim());
  if !root_dir_path.exists() || !root_dir_path.is_dir() {
    return Err("Project root location must be an existing folder.".to_string());
  }

  let mut store = read_projects_store(&app)?;
  let project = store
    .projects
    .iter_mut()
    .find(|project| project.id == args.project_id)
    .ok_or_else(|| "Project not found.".to_string())?;

  ensure_folders(&root_dir_path, PROJECT_FOLDERS)?;
  project.root_path = root_dir_path.to_string_lossy().to_string();
  project.updated_at = now_string();

  let updated = project.clone();
  write_projects_store(&app, &store)?;
  Ok(updated)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateProjectAnalysisDefaultsArgs {
  project_id: String,
  packages: AnalysisPackages
}

#[tauri::command]
fn update_project_analysis_defaults(
  app: AppHandle,
  args: UpdateProjectAnalysisDefaultsArgs
) -> Result<Project, String> {
  let mut store = read_projects_store(&app)?;
  let project = store
    .projects
    .iter_mut()
    .find(|project| project.id == args.project_id)
    .ok_or_else(|| "Project not found.".to_string())?;

  project.analysis_package_defaults = Some(args.packages);
  project.updated_at = now_string();

  let updated = project.clone();
  write_projects_store(&app, &store)?;
  Ok(updated)
}

#[tauri::command]
fn delete_project(app: AppHandle, args: DeleteProjectArgs) -> Result<(), String> {
  let mut store = read_projects_store(&app)?;
  let mut root_to_delete: Option<PathBuf> = None;
  let before = store.projects.len();
  store.projects.retain(|project| {
    if project.id == args.project_id {
      if args.delete_on_disk {
        root_to_delete = Some(PathBuf::from(project.root_path.clone()));
      }
      return false;
    }
    true
  });
  if store.projects.len() == before {
    return Err("Project not found.".to_string());
  }

  if let Some(root) = root_to_delete {
    let normalized = root.to_path_buf();
    let component_count = normalized.components().count();
    if component_count < 2 {
      return Err("Refusing to delete an unsafe root directory.".to_string());
    }
    if normalized.exists() && normalized.is_dir() {
      fs::remove_dir_all(&normalized).map_err(|err| err.to_string())?;
    }
  }
  write_projects_store(&app, &store)?;
  Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddStudyArgs {
  project_id: String,
  folder_name: Option<String>,
  title: Option<String>
}

#[tauri::command]
fn add_study(app: AppHandle, args: AddStudyArgs) -> Result<Project, String> {
  println!(
    "add_study called with project_id={}, folder_name={:?}, title={:?}",
    args.project_id, args.folder_name, args.title
  );
  let mut store = read_projects_store(&app)?;
  let project = store
    .projects
    .iter_mut()
    .find(|project| project.id == args.project_id)
    .ok_or_else(|| "Project not found.".to_string())?;
  println!(
    "add_study resolved project root_path={} existing studies={}",
    project.root_path,
    project.studies.len()
  );

  let mut trimmed_folder = args.folder_name.unwrap_or_default().trim().to_uppercase();
  if trimmed_folder.is_empty() {
    for _ in 0..20 {
      let candidate = generate_study_code();
      let candidate_root = PathBuf::from(project.root_path.clone())
        .join("studies")
        .join(&candidate);
      if !candidate_root.exists()
        && !project.studies.iter().any(|study| study.id == candidate)
      {
        trimmed_folder = candidate;
        break;
      }
    }
    if trimmed_folder.is_empty() {
      return Err("Unable to generate a unique study code.".to_string());
    }
  }
  if !is_valid_study_folder(&trimmed_folder) {
    return Err("Study folder name must match S-XXXXXX (letters/numbers).".to_string());
  }
  if trimmed_folder.contains('/') || trimmed_folder.contains('\\') || trimmed_folder.contains("..") {
    return Err("Study folder name must be a single folder name.".to_string());
  }
  if project.studies.iter().any(|study| study.id == trimmed_folder) {
    return Err("Study code already exists.".to_string());
  }

  let trimmed_title = args.title.unwrap_or_else(|| "Untitled Study".to_string());
  let study_root = PathBuf::from(project.root_path.clone())
    .join("studies")
    .join(&trimmed_folder);
  if study_root.exists() {
    return Err("Study folder already exists.".to_string());
  }
  ensure_folders(&study_root, STUDY_FOLDERS)?;

  let new_study = Study {
    id: trimmed_folder.to_string(),
    title: if trimmed_title.trim().is_empty() {
      "Untitled Study".to_string()
    } else {
      trimmed_title
    },
    created_at: now_string(),
    folder_path: study_root.to_string_lossy().to_string(),
    files: Vec::new()
  };

  project.studies.push(new_study);
  project.updated_at = now_string();
  let updated = project.clone();
  write_projects_store(&app, &store)?;
  Ok(updated)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenameStudyJsonArgs {
  project_id: String,
  study_id: String,
  title: String
}

#[tauri::command]
fn rename_study_json(app: AppHandle, args: RenameStudyJsonArgs) -> Result<Project, String> {
  let mut store = read_projects_store(&app)?;
  let project = store
    .projects
    .iter_mut()
    .find(|project| project.id == args.project_id)
    .ok_or_else(|| "Project not found.".to_string())?;

  let study = project
    .studies
    .iter_mut()
    .find(|study| study.id == args.study_id)
    .ok_or_else(|| "Study not found.".to_string())?;

  let trimmed = args.title.trim();
  if trimmed.is_empty() {
    return Err("Study title is required.".to_string());
  }

  study.title = trimmed.to_string();
  project.updated_at = now_string();
  let updated = project.clone();
  write_projects_store(&app, &store)?;
  Ok(updated)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenameStudyFolderArgs {
  project_id: String,
  study_id: String,
  folder_name: String
}

#[tauri::command]
fn rename_study_folder_json(app: AppHandle, args: RenameStudyFolderArgs) -> Result<Project, String> {
  let mut store = read_projects_store(&app)?;
  let project = store
    .projects
    .iter_mut()
    .find(|project| project.id == args.project_id)
    .ok_or_else(|| "Project not found.".to_string())?;

  let trimmed_folder = args.folder_name.trim();
  if trimmed_folder.is_empty() {
    return Err("Study folder name is required.".to_string());
  }
  if !is_valid_study_folder(trimmed_folder) {
    return Err("Study folder name must match S-XXXXXX (letters/numbers).".to_string());
  }
  if trimmed_folder.contains('/') || trimmed_folder.contains('\\') || trimmed_folder.contains("..") {
    return Err("Study folder name must be a single folder name.".to_string());
  }
  if project
    .studies
    .iter()
    .any(|study| study.id == trimmed_folder && study.id != args.study_id)
  {
    return Err("Study code already exists.".to_string());
  }

  let study = project
    .studies
    .iter_mut()
    .find(|study| study.id == args.study_id)
    .ok_or_else(|| "Study not found.".to_string())?;

  let base = PathBuf::from(project.root_path.clone()).join("studies");
  let old_root = if study.folder_path.trim().is_empty() {
    base.join(&study.id)
  } else {
    PathBuf::from(study.folder_path.clone())
  };
  let new_root = base.join(trimmed_folder);

  if old_root != new_root {
    if new_root.exists() {
      return Err("Study folder already exists.".to_string());
    }
    if !old_root.exists() {
      return Err("Study folder does not exist.".to_string());
    }
    fs::rename(&old_root, &new_root).map_err(|err| err.to_string())?;
  }

  study.id = trimmed_folder.to_string();
  study.folder_path = new_root.to_string_lossy().to_string();
  project.updated_at = now_string();

  let updated = project.clone();
  write_projects_store(&app, &store)?;
  Ok(updated)
}

#[tauri::command]
fn migrate_json_to_sqlite(app: AppHandle) -> Result<String, String> {
  let conn = connection(&app)?;
  init_schema(&conn)?;
  let store = read_projects_store(&app)?;

  let mut projects_added = 0;
  let mut studies_added = 0;

  for project in store.projects {
    let project_id = project.id.clone();
    let project_name = project.name.clone();
    let project_root = project.root_path.clone();
    let project_created = project.created_at.clone();
    let exists: i64 = conn
      .query_row(
        "SELECT COUNT(1) FROM projects WHERE id = ?1",
        params![&project_id],
        |row| row.get(0)
      )
      .map_err(|err| err.to_string())?;

    if exists == 0 {
      conn
        .execute(
          "INSERT INTO projects (id, name, root_path, created_at) VALUES (?1, ?2, ?3, ?4)",
          params![&project_id, &project_name, &project_root, &project_created]
        )
        .map_err(|err| err.to_string())?;
      projects_added += 1;
    }

    for study in project.studies {
      let study_exists: i64 = conn
        .query_row(
          "SELECT COUNT(1) FROM studies WHERE id = ?1",
          params![study.id],
          |row| row.get(0)
        )
        .map_err(|err| err.to_string())?;
      if study_exists > 0 {
        continue;
      }

      let folder_path = if !study.folder_path.trim().is_empty() {
        study.folder_path
      } else {
        PathBuf::from(project_root.clone())
          .join("studies")
          .join(&study.id)
          .to_string_lossy()
          .to_string()
      };

      conn
        .execute(
          "INSERT INTO studies (id, project_id, internal_name, paper_label, status, folder_path, created_at) \
          VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
          params![
            study.id,
            &project_id,
            study.title,
            Option::<String>::None,
            "planning",
            folder_path,
            study.created_at
          ]
        )
        .map_err(|err| err.to_string())?;
      studies_added += 1;
    }
  }

  Ok(format!(
    "Migration complete. Projects added: {projects_added}. Studies added: {studies_added}."
  ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListStudiesArgs {
  project_id: String
}

#[tauri::command]
fn list_studies(app: AppHandle, args: ListStudiesArgs) -> Result<Vec<DbStudy>, String> {
  let conn = connection(&app)?;
  init_schema(&conn)?;
  let mut stmt = conn
    .prepare(
      "SELECT id, project_id, internal_name, paper_label, status, folder_path, created_at \
      FROM studies WHERE project_id = ?1 ORDER BY created_at DESC"
    )
    .map_err(|err| err.to_string())?;
  let rows = stmt
    .query_map(params![args.project_id], |row| {
      Ok(DbStudy {
        id: row.get(0)?,
        project_id: row.get(1)?,
        internal_name: row.get(2)?,
        paper_label: row.get(3)?,
        status: row.get(4)?,
        folder_path: row.get(5)?,
        created_at: row.get(6)?
      })
    })
    .map_err(|err| err.to_string())?;

  let mut studies: Vec<DbStudy> = Vec::new();
  for row in rows {
    studies.push(row.map_err(|err| err.to_string())?);
  }
  Ok(studies)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateStudyArgs {
  project_id: String,
  internal_name: String,
  paper_label: Option<String>
}

#[tauri::command]
fn create_study(app: AppHandle, args: CreateStudyArgs) -> Result<DbStudy, String> {
  let conn = connection(&app)?;
  init_schema(&conn)?;

  let store = read_projects_store(&app)?;
  let project_root = store
    .projects
    .iter()
    .find(|project| project.id == args.project_id)
    .map(|project| project.root_path.clone())
    .ok_or_else(|| "Project not found.".to_string())?;

  let id = Uuid::new_v4().to_string();
  let folder = PathBuf::from(project_root).join("studies").join(&id);
  ensure_folders(&folder, STUDY_FOLDERS)?;

  let study = DbStudy {
    id: id.clone(),
    project_id: args.project_id,
    internal_name: args.internal_name,
    paper_label: args.paper_label,
    status: "planning".to_string(),
    folder_path: folder.to_string_lossy().to_string(),
    created_at: now_string()
  };

  conn
    .execute(
      "INSERT INTO studies (id, project_id, internal_name, paper_label, status, folder_path, created_at) \
      VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
      params![
        study.id,
        study.project_id,
        study.internal_name,
        study.paper_label,
        study.status,
        study.folder_path,
        study.created_at
      ]
    )
    .map_err(|err| err.to_string())?;

  Ok(study)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenameStudyArgs {
  study_id: String,
  internal_name: String,
  paper_label: Option<String>
}

#[tauri::command]
fn rename_study(app: AppHandle, args: RenameStudyArgs) -> Result<(), String> {
  let conn = connection(&app)?;
  init_schema(&conn)?;
  conn
    .execute(
      "UPDATE studies SET internal_name = ?1, paper_label = ?2 WHERE id = ?3",
      params![args.internal_name, args.paper_label, args.study_id]
    )
    .map_err(|err| err.to_string())?;
  Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateStudyStatusArgs {
  study_id: String,
  status: String
}

#[tauri::command]
fn update_study_status(app: AppHandle, args: UpdateStudyStatusArgs) -> Result<(), String> {
  let conn = connection(&app)?;
  init_schema(&conn)?;
  conn
    .execute(
      "UPDATE studies SET status = ?1 WHERE id = ?2",
      params![args.status, args.study_id]
    )
    .map_err(|err| err.to_string())?;
  Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetStudyDetailArgs {
  study_id: String
}

#[tauri::command]
fn get_study_detail(app: AppHandle, args: GetStudyDetailArgs) -> Result<StudyDetail, String> {
  let conn = connection(&app)?;
  init_schema(&conn)?;

  let study: DbStudy = conn
    .query_row(
      "SELECT id, project_id, internal_name, paper_label, status, folder_path, created_at \
      FROM studies WHERE id = ?1",
      params![args.study_id],
      |row| {
        Ok(DbStudy {
          id: row.get(0)?,
          project_id: row.get(1)?,
          internal_name: row.get(2)?,
          paper_label: row.get(3)?,
          status: row.get(4)?,
          folder_path: row.get(5)?,
          created_at: row.get(6)?
        })
      }
    )
    .map_err(|err| err.to_string())?;

  let mut stmt = conn
    .prepare(
      "SELECT id, study_id, kind, value, label, created_at FROM artifacts WHERE study_id = ?1 \
      ORDER BY created_at DESC"
    )
    .map_err(|err| err.to_string())?;

  let rows = stmt
    .query_map(params![args.study_id], |row| {
      Ok(Artifact {
        id: row.get(0)?,
        study_id: row.get(1)?,
        kind: row.get(2)?,
        value: row.get(3)?,
        label: row.get(4)?,
        created_at: row.get(5)?
      })
    })
    .map_err(|err| err.to_string())?;

  let mut artifacts = Vec::new();
  for row in rows {
    artifacts.push(row.map_err(|err| err.to_string())?);
  }

  Ok(StudyDetail { study, artifacts })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddArtifactArgs {
  study_id: String,
  kind: String,
  value: String,
  label: Option<String>
}

#[tauri::command]
fn add_artifact(app: AppHandle, args: AddArtifactArgs) -> Result<(), String> {
  let conn = connection(&app)?;
  init_schema(&conn)?;
  let id = Uuid::new_v4().to_string();
  conn
    .execute(
      "INSERT INTO artifacts (id, study_id, kind, value, label, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
      params![id, args.study_id, args.kind, args.value, args.label, now_string()]
    )
    .map_err(|err| err.to_string())?;
  Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoveArtifactArgs {
  artifact_id: String
}

#[tauri::command]
fn remove_artifact(app: AppHandle, args: RemoveArtifactArgs) -> Result<(), String> {
  let conn = connection(&app)?;
  init_schema(&conn)?;
  conn
    .execute("DELETE FROM artifacts WHERE id = ?1", params![args.artifact_id])
    .map_err(|err| err.to_string())?;
  Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenerateOsfPackagesArgs {
  study_id: String,
  include_pilots: bool
}

#[tauri::command]
fn generate_osf_packages(app: AppHandle, args: GenerateOsfPackagesArgs) -> Result<String, String> {
  let conn = connection(&app)?;
  init_schema(&conn)?;

  let folder_path: String = conn
    .query_row(
      "SELECT folder_path FROM studies WHERE id = ?1",
      params![args.study_id],
      |row| row.get(0)
    )
    .map_err(|err| err.to_string())?;

  let study_root = PathBuf::from(folder_path);
  if !study_root.exists() {
    return Err("Study folder does not exist".to_string());
  }

  let osf_root = study_root.join("08_osf_release");
  let complete_root = osf_root.join("COMPLETE");
  let condensed_root = osf_root.join("CONDENSED");

  if complete_root.exists() {
    fs::remove_dir_all(&complete_root).map_err(|err| err.to_string())?;
  }
  if condensed_root.exists() {
    fs::remove_dir_all(&condensed_root).map_err(|err| err.to_string())?;
  }

  let complete_count = copy_dir_filtered(&study_root, &complete_root, args.include_pilots, false)?;
  let condensed_count = copy_dir_filtered(&study_root, &condensed_root, args.include_pilots, true)?;

  Ok(format!(
    "OSF packages generated. COMPLETE: {complete_count} files, CONDENSED: {condensed_count} files."
  ))
}

#[tauri::command]
fn check_root_dir(root_dir: String) -> Result<RootDirInfo, String> {
  let path = PathBuf::from(root_dir.trim());
  let exists = path.exists() && path.is_dir();
  let is_git_repo = exists && path.join(".git").exists();
  Ok(RootDirInfo { exists, is_git_repo })
}

#[tauri::command]
fn create_analysis_template(
  app: AppHandle,
  project_id: String,
  study_id: String,
  options: AnalysisTemplateOptions
) -> Result<String, String> {
  let store = read_projects_store(&app)?;
  let project = store
    .projects
    .iter()
    .find(|project| project.id == project_id)
    .ok_or_else(|| "Project not found.".to_string())?;
  let study = project
    .studies
    .iter()
    .find(|study| study.id == study_id)
    .ok_or_else(|| "Study not found.".to_string())?;

  let study_root = resolve_study_root(project, study);
  if !study_root.exists() {
    return Err("Study folder does not exist.".to_string());
  }
  let project_root = PathBuf::from(project.root_path.clone());
  ensure_project_style_kit(&project_root)?;

  let analysis_dir = study_root.join(ANALYSIS_FOLDER);
  let template_path =
    create_analysis_template_in_dir(
      &project_root,
      &study_root,
      &analysis_dir,
      &study_id,
      &study.title,
      &options
    )?;

  Ok(format!(
    "Created analysis template at {}",
    template_path.to_string_lossy()
  ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListAnalysisTemplatesArgs {
  project_id: String,
  study_id: String
}

#[tauri::command]
fn list_analysis_templates(
  app: AppHandle,
  args: ListAnalysisTemplatesArgs
) -> Result<Vec<String>, String> {
  let store = read_projects_store(&app)?;
  let project = store
    .projects
    .iter()
    .find(|project| project.id == args.project_id)
    .ok_or_else(|| "Project not found.".to_string())?;
  let study = project
    .studies
    .iter()
    .find(|study| study.id == args.study_id)
    .ok_or_else(|| "Study not found.".to_string())?;

  let study_root = resolve_study_root(project, study);
  if !study_root.exists() {
    return Err("Study folder does not exist.".to_string());
  }

  let analysis_dir = study_root.join(ANALYSIS_FOLDER);
  if !analysis_dir.exists() {
    return Ok(Vec::new());
  }

  let mut names: Vec<String> = Vec::new();
  let entries = fs::read_dir(&analysis_dir).map_err(|err| err.to_string())?;
  for entry in entries {
    let entry = entry.map_err(|err| err.to_string())?;
    let path = entry.path();
    if !path.is_file() {
      continue;
    }
    let ext = path.extension().and_then(|value| value.to_str()).unwrap_or("");
    if ext != "Rmd" {
      continue;
    }
    if let Some(stem) = path.file_stem().and_then(|value| value.to_str()) {
      names.push(stem.to_string());
    }
  }
  names.sort();
  Ok(names)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteAnalysisTemplateArgs {
  project_id: String,
  study_id: String,
  analysis_name: String
}

#[tauri::command]
fn delete_analysis_template(
  app: AppHandle,
  args: DeleteAnalysisTemplateArgs
) -> Result<String, String> {
  let store = read_projects_store(&app)?;
  let project = store
    .projects
    .iter()
    .find(|project| project.id == args.project_id)
    .ok_or_else(|| "Project not found.".to_string())?;
  let study = project
    .studies
    .iter()
    .find(|study| study.id == args.study_id)
    .ok_or_else(|| "Study not found.".to_string())?;

  let trimmed_name = args.analysis_name.trim();
  if trimmed_name.is_empty() {
    return Err("Analysis name is required.".to_string());
  }
  if trimmed_name.contains('/') || trimmed_name.contains('\\') || trimmed_name.contains("..") {
    return Err("Analysis name must be a single file name.".to_string());
  }
  if trimmed_name.contains('.') {
    return Err("Analysis name should not include a file extension.".to_string());
  }

  let study_root = resolve_study_root(project, study);
  if !study_root.exists() {
    return Err("Study folder does not exist.".to_string());
  }

  let analysis_dir = study_root.join(ANALYSIS_FOLDER);
  let target = analysis_dir.join(format!("{trimmed_name}.Rmd"));
  if !target.exists() {
    return Err("Analysis template does not exist.".to_string());
  }
  fs::remove_file(&target).map_err(|err| err.to_string())?;

  Ok(format!(
    "Deleted analysis template at {}",
    target.to_string_lossy()
  ))
}

#[tauri::command]
fn import_files(
  app: AppHandle,
  project_id: String,
  study_id: String,
  paths: Vec<String>
) -> Result<Study, String> {
  let mut store = read_projects_store(&app)?;
  let project = store
    .projects
    .iter_mut()
    .find(|project| project.id == project_id)
    .ok_or_else(|| "Project not found.".to_string())?;
  let project_root = PathBuf::from(project.root_path.clone());

  let study = project
    .studies
    .iter_mut()
    .find(|study| study.id == study_id)
    .ok_or_else(|| "Study not found.".to_string())?;

  let dest_dir = project_root
    .join("studies")
    .join(&study.id)
    .join("sources");
  fs::create_dir_all(&dest_dir).map_err(|err| err.to_string())?;

  let mut known_paths: HashSet<String> =
    study.files.iter().map(|file| file.path.clone()).collect();

  for source in paths {
    let trimmed = source.trim();
    if trimmed.is_empty() {
      continue;
    }
    let src = PathBuf::from(trimmed);
    if !src.exists() || !src.is_file() {
      continue;
    }
    let filename = match src.file_name() {
      Some(value) => value,
      None => continue
    };

    let dest_path = if src.starts_with(&dest_dir) {
      src.clone()
    } else {
      unique_dest_path(&dest_dir, filename)
    };

    let rel_path = diff_paths(&dest_path, &project_root).unwrap_or(dest_path.clone());
    let mut rel_string = rel_path.to_string_lossy().to_string();
    if rel_string.contains('\\') {
      rel_string = rel_string.replace('\\', "/");
    }

    if known_paths.contains(&rel_string) {
      continue;
    }

    if src != dest_path {
      move_file_cross_device(&src, &dest_path)?;
    }

    let name = dest_path
      .file_name()
      .and_then(|value| value.to_str())
      .unwrap_or("file")
      .to_string();
    let kind = kind_from_ext(dest_path.extension());

    study.files.push(FileRef {
      path: rel_string.clone(),
      name,
      kind
    });
    known_paths.insert(rel_string);
  }

  project.updated_at = now_string();
  let updated = study.clone();
  write_projects_store(&app, &store)?;
  Ok(updated)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoveFileArgs {
  project_id: String,
  study_id: String,
  path: String
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteStudyArgs {
  project_id: String,
  study_id: String,
  #[serde(default)]
  delete_on_disk: bool
}

#[tauri::command]
fn remove_file_ref(app: AppHandle, args: RemoveFileArgs) -> Result<Study, String> {
  let mut store = read_projects_store(&app)?;
  let project = store
    .projects
    .iter_mut()
    .find(|project| project.id == args.project_id)
    .ok_or_else(|| "Project not found.".to_string())?;
  let project_root = PathBuf::from(project.root_path.clone());

  let study = project
    .studies
    .iter_mut()
    .find(|study| study.id == args.study_id)
    .ok_or_else(|| "Study not found.".to_string())?;

  let rel = args.path.trim();
  if !rel.is_empty() {
    let candidate = project_root.join(rel);
    let candidate = fs::canonicalize(&candidate).unwrap_or(candidate);
    let root = fs::canonicalize(&project_root).unwrap_or(project_root.clone());
    if candidate.starts_with(&root) && candidate.is_file() {
      let _ = fs::remove_file(&candidate);
    }
  }

  study.files.retain(|file| file.path != rel);
  project.updated_at = now_string();
  let updated = study.clone();
  write_projects_store(&app, &store)?;
  Ok(updated)
}

#[tauri::command]
fn git_status() -> Result<String, String> {
  let repo_root = std::env::current_dir().map_err(|err| err.to_string())?;
  let output = Command::new("git")
    .args(["status", "-sb"])
    .current_dir(repo_root)
    .output()
    .map_err(|err| err.to_string())?;
  if !output.status.success() {
    return Err(String::from_utf8_lossy(&output.stderr).to_string());
  }
  Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[tauri::command]
fn git_commit_push(message: String) -> Result<String, String> {
  let repo_root = std::env::current_dir().map_err(|err| err.to_string())?;

  let add_output = Command::new("git")
    .args(["add", "-A"])
    .current_dir(&repo_root)
    .output()
    .map_err(|err| err.to_string())?;
  if !add_output.status.success() {
    return Err(String::from_utf8_lossy(&add_output.stderr).to_string());
  }

  let commit_output = Command::new("git")
    .args(["commit", "-m", &message])
    .current_dir(&repo_root)
    .output()
    .map_err(|err| err.to_string())?;

  let commit_stdout = String::from_utf8_lossy(&commit_output.stdout).to_string();
  let commit_stderr = String::from_utf8_lossy(&commit_output.stderr).to_string();

  let no_changes = commit_stdout.contains("nothing to commit") || commit_stderr.contains("nothing to commit");
  if !commit_output.status.success() && !no_changes {
    return Err(commit_stderr);
  }

  let push_output = Command::new("git")
    .args(["push"])
    .current_dir(&repo_root)
    .output()
    .map_err(|err| err.to_string())?;

  if !push_output.status.success() {
    return Err(String::from_utf8_lossy(&push_output.stderr).to_string());
  }

  let push_stdout = String::from_utf8_lossy(&push_output.stdout).to_string();

  Ok(format!("{}{}", commit_stdout, push_stdout))
}

#[tauri::command]
fn delete_study(app: AppHandle, args: DeleteStudyArgs) -> Result<Project, String> {
  let mut store = read_projects_store(&app)?;
  let project = store
    .projects
    .iter_mut()
    .find(|project| project.id == args.project_id)
    .ok_or_else(|| "Project not found.".to_string())?;

  let mut removed_path: Option<PathBuf> = None;
  let before = project.studies.len();
  project.studies.retain(|study| {
    if study.id == args.study_id {
      if args.delete_on_disk {
        if !study.folder_path.trim().is_empty() {
          removed_path = Some(PathBuf::from(study.folder_path.clone()));
        } else {
          removed_path = Some(
            PathBuf::from(project.root_path.clone())
              .join("studies")
              .join(&study.id)
          );
        }
      }
      return false;
    }
    true
  });

  if project.studies.len() == before {
    return Err("Study not found.".to_string());
  }

  if let Some(folder) = removed_path {
    let root = fs::canonicalize(PathBuf::from(project.root_path.clone()))
      .unwrap_or_else(|_| PathBuf::from(project.root_path.clone()));
    let target = fs::canonicalize(&folder).unwrap_or(folder);
    if target.starts_with(&root) && target.is_dir() {
      fs::remove_dir_all(&target).map_err(|err| err.to_string())?;
    }
  }

  project.updated_at = now_string();
  let updated = project.clone();
  write_projects_store(&app, &store)?;
  Ok(updated)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn empty_options() -> AnalysisTemplateOptions {
    AnalysisTemplateOptions {
      analysis_file_name: None,
      data_source_paths: None,
      dataset_path_hint: None,
      outcome_var_hint: None,
      treatment_var_hint: None,
      id_var_hint: None,
      time_var_hint: None,
      group_var_hint: None,
      descriptives: Vec::new(),
      plots: Vec::new(),
      balance_checks: Vec::new(),
      models: Vec::new(),
      diagnostics: Vec::new(),
      tables: Vec::new(),
      robustness: Vec::new(),
      model_layouts: Vec::new(),
      exploratory: false,
      export_artifacts: false
    }
  }

  #[test]
  fn render_requires_model_layouts_for_model_scaffolding() {
    let mut options = empty_options();
    options.model_layouts = vec![ModelLayout {
      name: "OLS Main".to_string(),
      model_type: "ols".to_string(),
      outcome_var: "outcome_y".to_string(),
      treatment_var: Some("treat_x".to_string()),
      layout: "simple".to_string(),
      interaction_var: None,
      covariates: Some("cov1 + cov2".to_string()),
      id_var: None,
      time_var: None,
      figures: vec!["coef_plot".to_string()],
      include_in_main_table: true
    }];
    let rendered = render_analysis_rmd(
      Path::new("project"),
      Path::new("project/studies/S-ABC123"),
      "S-ABC123",
      "Test Study",
      &options
    );
    assert!(rendered.contains("## OLS Main (ols)"));
    assert!(rendered.contains("outcome_y ~ treat_x + cov1 + cov2"));
    assert!(rendered.contains("style_pkg_name <- \"researchworkflowstyle\""));
    assert!(rendered.contains("source(here::here(\"R/style/theme_plots.R\"))"));

    let rendered_without_layouts = render_analysis_rmd(
      Path::new("project"),
      Path::new("project/studies/S-ABC123"),
      "S-ABC123",
      "Test Study",
      &empty_options()
    );
    assert!(rendered_without_layouts.contains("Add at least one Model Layout in the model builder"));
  }

  #[test]
  fn create_template_writes_file_and_output_folders() {
    let base = std::env::temp_dir().join(format!("analysis-test-{}", Uuid::new_v4()));
    let study_root = base.join("S-ABC123");
    let analysis_dir = study_root.join("06_analysis");
    fs::create_dir_all(&analysis_dir).expect("failed to create temp analysis dir");

    let options = empty_options();
    let first = create_analysis_template_in_dir(
      &base,
      &study_root,
      &analysis_dir,
      "S-ABC123",
      "Test Study",
      &options
    )
    .expect("expected first template to be created");
    assert!(first.exists());
    assert!(study_root.join("07_outputs").exists());
    assert!(study_root.join("07_outputs").join("tables").exists());
    assert!(study_root.join("07_outputs").join("figures").exists());
    assert!(study_root.join("07_outputs").join("reports").exists());

    let second = create_analysis_template_in_dir(
      &base,
      &study_root,
      &analysis_dir,
      "S-ABC123",
      "Test Study",
      &options
    )
    .expect("expected second template to be created with timestamp");
    assert!(second.exists());
    assert_ne!(first, second);

    let _ = fs::remove_dir_all(base);
  }

  #[test]
  fn ensure_style_kit_creates_and_merges_config() {
    let base = std::env::temp_dir().join(format!("style-kit-test-{}", Uuid::new_v4()));
    fs::create_dir_all(base.join("config")).expect("failed to create temp config dir");
    fs::write(
      base.join("config").join("analysis_defaults.json"),
      "{\n  \"version\": 9,\n  \"plots\": {\"base_size\": 10}\n}\n"
    )
    .expect("failed to seed config");

    ensure_project_style_kit(&base).expect("style kit ensure should succeed");

    assert!(base.join("R").join("style").join("theme_plots.R").exists());
    assert!(base.join("R").join("style").join("tables_flextable.R").exists());
    assert!(base.join("R").join("style").join("style_init.R").exists());
    assert!(base.join("R").join("style").join("README.md").exists());
    assert!(base.join("R").join("researchworkflowstyle").join("DESCRIPTION").exists());
    assert!(base.join("R").join("researchworkflowstyle").join("NAMESPACE").exists());
    assert!(base.join("R").join("researchworkflowstyle").join("R").join("plots.R").exists());
    assert!(base.join("R").join("researchworkflowstyle").join("R").join("tables.R").exists());
    assert!(base.join("R").join("researchworkflowstyle").join("R").join("init.R").exists());

    let merged_raw = fs::read_to_string(base.join("config").join("analysis_defaults.json"))
      .expect("config should be readable");
    let merged: serde_json::Value =
      serde_json::from_str(&merged_raw).expect("config should be valid json");
    assert_eq!(merged.get("version").and_then(|v| v.as_i64()), Some(9));
    assert_eq!(
      merged
        .get("plots")
        .and_then(|v| v.get("base_size"))
        .and_then(|v| v.as_i64()),
      Some(10)
    );
    assert_eq!(
      merged
        .get("styleKit")
        .and_then(|v| v.get("path"))
        .and_then(|v| v.as_str()),
      Some("R/style")
    );
    assert_eq!(
      merged
        .get("stylePackage")
        .and_then(|v| v.get("path"))
        .and_then(|v| v.as_str()),
      Some("R/researchworkflowstyle")
    );

    let _ = fs::remove_dir_all(base);
  }

  #[test]
  fn create_template_uses_custom_analysis_file_name() {
    let base = std::env::temp_dir().join(format!("analysis-name-test-{}", Uuid::new_v4()));
    let study_root = base.join("S-ABC123");
    let analysis_dir = study_root.join("06_analysis");
    fs::create_dir_all(&analysis_dir).expect("failed to create temp analysis dir");

    let mut options = empty_options();
    options.analysis_file_name = Some("pilot_analysis".to_string());

    let first = create_analysis_template_in_dir(
      &base,
      &study_root,
      &analysis_dir,
      "S-ABC123",
      "Test Study",
      &options
    )
    .expect("expected template with custom file name");

    assert!(first.ends_with("pilot_analysis.Rmd"));

    let _ = fs::remove_dir_all(base);
  }

  #[test]
  fn render_uses_selected_data_sources_when_provided() {
    let mut options = empty_options();
    options.data_source_paths = Some(vec![
      "/tmp/project/data/clean/a.csv".to_string(),
      "/tmp/project/data/clean/b.tsv".to_string()
    ]);

    let rendered = render_analysis_rmd(
      Path::new("project"),
      Path::new("project/studies/S-ABC123"),
      "S-ABC123",
      "Test Study",
      &options
    );

    assert!(rendered.contains("read_data_source <- function(path)"));
    assert!(rendered.contains("/tmp/project/data/clean/a.csv"));
    assert!(rendered.contains("/tmp/project/data/clean/b.tsv"));
  }

  #[test]
  fn render_groups_model_tables_by_outcome_from_layouts() {
    let mut options = empty_options();
    options.tables = vec!["model_table".to_string()];
    options.model_layouts = vec![
      ModelLayout {
        name: "Model A".to_string(),
        model_type: "ols".to_string(),
        outcome_var: "y1".to_string(),
        treatment_var: Some("x1 + x2".to_string()),
        layout: "simple".to_string(),
        interaction_var: None,
        covariates: Some("x1 + x2".to_string()),
        id_var: None,
        time_var: None,
        figures: vec!["coef_plot".to_string()],
        include_in_main_table: true
      },
      ModelLayout {
        name: "Model B".to_string(),
        model_type: "ols".to_string(),
        outcome_var: "y2".to_string(),
        treatment_var: Some("x3".to_string()),
        layout: "simple".to_string(),
        interaction_var: None,
        covariates: Some("x3".to_string()),
        id_var: None,
        time_var: None,
        figures: vec!["coef_plot".to_string()],
        include_in_main_table: true
      }
    ];

    let rendered = render_analysis_rmd(
      Path::new("project"),
      Path::new("project/studies/S-ABC123"),
      "S-ABC123",
      "Test Study",
      &options
    );
    assert!(rendered.contains("models_y1.html"));
    assert!(rendered.contains("models_y2.html"));
    assert!(rendered.contains("Main Figures by Model Builder Input"));
  }

}

fn main() {
  tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![
      init_db,
      list_projects,
      create_project,
      update_project_root,
      update_project_analysis_defaults,
      delete_project,
      add_study,
      rename_study_json,
      rename_study_folder_json,
      migrate_json_to_sqlite,
      check_root_dir,
      create_analysis_template,
      list_analysis_templates,
      delete_analysis_template,
      import_files,
      remove_file_ref,
      delete_study,
      list_studies,
      create_study,
      rename_study,
      update_study_status,
      get_study_detail,
      add_artifact,
      remove_artifact,
      generate_osf_packages,
      git_status,
      git_commit_push,
      list_build_assets,
      list_prereg_assets,
      parse_qsf,
      parse_prereg,
      generate_analysis_spec,
      save_analysis_spec,
      resolve_mappings,
      render_analysis_from_spec
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
