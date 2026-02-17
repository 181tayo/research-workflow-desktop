import { Diagnostic, ModelType } from "../types/analysisTemplate";

export const suggestDiagnostics = (models: ModelType[]): Diagnostic[] => {
  const suggestions = new Set<Diagnostic>();

  for (const model of models) {
    if (model === "ols") {
      suggestions.add("linearity");
      suggestions.add("normality_residuals");
      suggestions.add("homoskedasticity");
      suggestions.add("multicollinearity");
      suggestions.add("influential_points");
    }
    if (model === "logit") {
      suggestions.add("multicollinearity");
      suggestions.add("influential_points");
    }
    if (model === "poisson" || model === "negbin") {
      suggestions.add("overdispersion");
    }
    if (model === "did" || model === "event_study") {
      suggestions.add("parallel_trends");
      suggestions.add("placebo_tests");
    }
    if (model === "rd") {
      suggestions.add("bandwidth_sensitivity");
    }
  }

  return Array.from(suggestions);
};
