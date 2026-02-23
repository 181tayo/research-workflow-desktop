make_apa_descriptives <- function(df, vars) {
  vars <- vars[vars %in% names(df)]
  out <- data.frame(
    variable = vars,
    mean = vapply(vars, function(v) mean(df[[v]], na.rm = TRUE), numeric(1)),
    sd = vapply(vars, function(v) stats::sd(df[[v]], na.rm = TRUE), numeric(1))
  )
  flextable::flextable(out)
}

make_apa_balance <- function(df, group_vars, vars) {
  vars <- vars[vars %in% names(df)]
  group <- group_vars[group_vars %in% names(df)][1]
  if (is.na(group) || is.null(group)) return(flextable::flextable(data.frame()))
  out <- df |> dplyr::group_by(.data[[group]]) |> dplyr::summarise(dplyr::across(dplyr::all_of(vars), ~mean(.x, na.rm = TRUE)))
  flextable::flextable(out)
}

make_apa_model_table <- function(model, title) {
  tab <- modelsummary::modelsummary(list(title = model), output = "data.frame")
  flextable::flextable(tab)
}

save_apa_table <- function(ft, path) {
  flextable::save_as_docx(ft, path = path)
}
