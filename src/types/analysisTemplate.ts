export type PlotType =
  | "histogram"
  | "boxplot"
  | "density"
  | "scatter"
  | "qqplot"
  | "correlation_heatmap"
  | "coef_plot"
  | "event_study_plot";

export type DescriptiveBlock =
  | "summary_stats"
  | "counts"
  | "missingness"
  | "group_summary"
  | "correlations";

export type BalanceCheck =
  | "baseline_table"
  | "std_diff"
  | "randomization_check";

export type ModelType =
  | "ols"
  | "logit"
  | "poisson"
  | "negbin"
  | "mixed_effects"
  | "fixed_effects"
  | "survival"
  | "rd"
  | "did"
  | "event_study";

export type Diagnostic =
  | "linearity"
  | "normality_residuals"
  | "homoskedasticity"
  | "multicollinearity"
  | "influential_points"
  | "overdispersion"
  | "parallel_trends"
  | "common_support"
  | "placebo_tests"
  | "bandwidth_sensitivity";

export type TableType =
  | "table1_descriptives"
  | "balance_table"
  | "model_table"
  | "marginal_effects_table";

export interface AnalysisTemplateOptions {
  datasetPathHint?: string;
  outcomeVarHint?: string;
  treatmentVarHint?: string;
  idVarHint?: string;
  timeVarHint?: string;
  groupVarHint?: string;
  descriptives: DescriptiveBlock[];
  plots: PlotType[];
  balanceChecks: BalanceCheck[];
  models: ModelType[];
  diagnostics: Diagnostic[];
  tables: TableType[];
  robustness: string[];
  exploratory: boolean;
  exportArtifacts: boolean;
}
