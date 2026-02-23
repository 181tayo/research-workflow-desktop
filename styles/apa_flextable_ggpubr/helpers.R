apply_qsf_labels <- function(df, label_map) {
  for (nm in names(label_map)) {
    if (nm %in% names(df)) attr(df[[nm]], "label") <- label_map[[nm]]
  }
  df
}

apply_qsf_choices <- function(df, choices_map) {
  for (nm in names(choices_map)) {
    if (nm %in% names(df)) {
      df[[nm]] <- factor(df[[nm]], labels = choices_map[[nm]])
    }
  }
  df
}

score_scale_mean <- function(df, items, new_name) {
  valid <- items[items %in% names(df)]
  if (length(valid) == 0) {
    warning(sprintf("No items found for scale '%s'", new_name))
    return(df)
  }
  df[[new_name]] <- rowMeans(df[, valid, drop = FALSE], na.rm = TRUE)
  df
}
