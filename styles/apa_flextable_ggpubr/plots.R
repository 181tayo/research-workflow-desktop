plot_apa_hist <- function(df, x) {
  ggpubr::gghistogram(df, x = x, add = "mean")
}

plot_apa_box <- function(df, x, y) {
  ggpubr::ggboxplot(df, x = x, y = y, add = "jitter")
}

plot_apa_coef <- function(model) {
  coef_df <- broom::tidy(model)
  ggplot2::ggplot(coef_df, ggplot2::aes(x = estimate, y = term)) +
    ggplot2::geom_point() +
    ggplot2::geom_errorbarh(ggplot2::aes(xmin = estimate - 1.96 * std.error, xmax = estimate + 1.96 * std.error), height = 0.1)
}

save_apa_plot <- function(p, path) {
  ggplot2::ggsave(path, p, width = 7, height = 5, dpi = 300)
}
