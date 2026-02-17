import { useEffect, useMemo, useState } from "react";
import {
  AnalysisTemplateOptions,
  BalanceCheck,
  DescriptiveBlock,
  Diagnostic,
  ModelType,
  PlotType,
  TableType
} from "../types/analysisTemplate";
import { suggestDiagnostics } from "../utils/analysisTemplate";

type WizardProps = {
  isOpen: boolean;
  projectId: string;
  studyId: string;
  loading: boolean;
  onClose: () => void;
  onSubmit: (options: AnalysisTemplateOptions) => Promise<void>;
};

const DESCRIPTIVE_OPTIONS: Array<{ value: DescriptiveBlock; label: string }> = [
  { value: "summary_stats", label: "Summary stats" },
  { value: "counts", label: "Counts" },
  { value: "missingness", label: "Missingness" },
  { value: "group_summary", label: "Group summary" },
  { value: "correlations", label: "Correlations" }
];

const PLOT_OPTIONS: Array<{ value: PlotType; label: string }> = [
  { value: "histogram", label: "Histogram" },
  { value: "boxplot", label: "Boxplot" },
  { value: "density", label: "Density" },
  { value: "scatter", label: "Scatter" },
  { value: "qqplot", label: "QQ plot" },
  { value: "correlation_heatmap", label: "Correlation heatmap" },
  { value: "coef_plot", label: "Coefficient plot" },
  { value: "event_study_plot", label: "Event-study plot" }
];

const BALANCE_OPTIONS: Array<{ value: BalanceCheck; label: string }> = [
  { value: "baseline_table", label: "Baseline table" },
  { value: "std_diff", label: "Standardized differences" },
  { value: "randomization_check", label: "Randomization checks" }
];

const MODEL_OPTIONS: Array<{ value: ModelType; label: string }> = [
  { value: "ols", label: "OLS" },
  { value: "logit", label: "Logistic" },
  { value: "poisson", label: "Poisson" },
  { value: "negbin", label: "NegBin" },
  { value: "mixed_effects", label: "Mixed effects" },
  { value: "fixed_effects", label: "Fixed effects" },
  { value: "survival", label: "Survival" },
  { value: "rd", label: "Regression discontinuity" },
  { value: "did", label: "DiD" },
  { value: "event_study", label: "Event study" }
];

const DIAGNOSTIC_OPTIONS: Array<{ value: Diagnostic; label: string }> = [
  { value: "linearity", label: "Linearity" },
  { value: "normality_residuals", label: "Normality of residuals" },
  { value: "homoskedasticity", label: "Homoskedasticity" },
  { value: "multicollinearity", label: "Multicollinearity" },
  { value: "influential_points", label: "Influential points" },
  { value: "overdispersion", label: "Overdispersion" },
  { value: "parallel_trends", label: "Parallel trends" },
  { value: "common_support", label: "Common support" },
  { value: "placebo_tests", label: "Placebo tests" },
  { value: "bandwidth_sensitivity", label: "Bandwidth sensitivity" }
];

const TABLE_OPTIONS: Array<{ value: TableType; label: string }> = [
  { value: "table1_descriptives", label: "Table 1 descriptives" },
  { value: "balance_table", label: "Balance table" },
  { value: "model_table", label: "Model table" },
  { value: "marginal_effects_table", label: "Marginal effects table" }
];

const ROBUSTNESS_OPTIONS = [
  "cluster_se",
  "hc_se",
  "winsorize",
  "alt_controls",
  "alt_outcome",
  "placebo_tests",
  "sensitivity"
];

const STEP_TITLES = [
  "Data + Variables",
  "Descriptives & Plots",
  "Balance Checks",
  "Models",
  "Diagnostics",
  "Tables/Figures & Exports",
  "Review + Create"
];

const defaultOptions = (): AnalysisTemplateOptions => ({
  analysisFileName: "analysis",
  datasetPathHint: "data/clean/analysis.csv",
  outcomeVarHint: "y",
  treatmentVarHint: "treat",
  idVarHint: "id",
  timeVarHint: "time",
  groupVarHint: "group",
  descriptives: [],
  plots: [],
  balanceChecks: [],
  models: [],
  diagnostics: [],
  tables: [],
  robustness: [],
  exploratory: false,
  exportArtifacts: true
});

export function AnalysisTemplateWizard({
  isOpen,
  projectId,
  studyId,
  loading,
  onClose,
  onSubmit
}: WizardProps) {
  const [step, setStep] = useState(0);
  const [options, setOptions] = useState<AnalysisTemplateOptions>(defaultOptions);

  useEffect(() => {
    if (!isOpen) return;
    setStep(0);
    setOptions(defaultOptions());
  }, [isOpen, projectId, studyId]);

  const modelKey = useMemo(() => options.models.join("|"), [options.models]);

  useEffect(() => {
    if (!isOpen) return;
    const suggestions = suggestDiagnostics(options.models);
    setOptions((prev) => {
      const merged = Array.from(new Set([...prev.diagnostics, ...suggestions]));
      if (
        merged.length === prev.diagnostics.length &&
        merged.every((value, idx) => value === prev.diagnostics[idx])
      ) {
        return prev;
      }
      return { ...prev, diagnostics: merged };
    });
  }, [isOpen, modelKey, options.models]);

  if (!isOpen) return null;

  const toggleListValue = <K extends "descriptives" | "plots" | "balanceChecks" | "models" | "diagnostics" | "tables" | "robustness">(
    key: K,
    value: AnalysisTemplateOptions[K][number]
  ) => {
    setOptions((prev) => {
      const values = prev[key] as Array<AnalysisTemplateOptions[K][number]>;
      const next = values.includes(value)
        ? values.filter((item) => item !== value)
        : [...values, value];
      return { ...prev, [key]: next } as AnalysisTemplateOptions;
    });
  };

  const needsPanelHint = options.models.includes("did") || options.models.includes("event_study");

  const renderStep = () => {
    if (step === 0) {
      return (
        <div className="wizard-step">
          <p className="muted">These are placeholders only; replace during cleaning.</p>
          <label>
            Analysis file name (without extension)
            <input
              value={options.analysisFileName ?? ""}
              onChange={(event) =>
                setOptions((prev) => ({ ...prev, analysisFileName: event.target.value }))
              }
              placeholder="analysis or pilot_analysis"
            />
          </label>
          <label>
            Dataset path hint
            <input
              value={options.datasetPathHint ?? ""}
              onChange={(event) =>
                setOptions((prev) => ({ ...prev, datasetPathHint: event.target.value }))
              }
            />
          </label>
          <label>
            Outcome variable hint
            <input
              value={options.outcomeVarHint ?? ""}
              onChange={(event) =>
                setOptions((prev) => ({ ...prev, outcomeVarHint: event.target.value }))
              }
            />
          </label>
          <label>
            Treatment variable hint
            <input
              value={options.treatmentVarHint ?? ""}
              onChange={(event) =>
                setOptions((prev) => ({ ...prev, treatmentVarHint: event.target.value }))
              }
            />
          </label>
          <div className="inline-field">
            <label>
              ID variable hint
              <input
                value={options.idVarHint ?? ""}
                onChange={(event) =>
                  setOptions((prev) => ({ ...prev, idVarHint: event.target.value }))
                }
              />
            </label>
            <label>
              Time variable hint
              <input
                value={options.timeVarHint ?? ""}
                onChange={(event) =>
                  setOptions((prev) => ({ ...prev, timeVarHint: event.target.value }))
                }
              />
            </label>
          </div>
          <label>
            Group variable hint
            <input
              value={options.groupVarHint ?? ""}
              onChange={(event) =>
                setOptions((prev) => ({ ...prev, groupVarHint: event.target.value }))
              }
            />
          </label>
        </div>
      );
    }

    if (step === 1) {
      return (
        <div className="wizard-step">
          <h3>Descriptives</h3>
          <div className="wizard-grid">
            {DESCRIPTIVE_OPTIONS.map((item) => (
              <label key={item.value} className="checkbox compact">
                <input
                  type="checkbox"
                  checked={options.descriptives.includes(item.value)}
                  onChange={() => toggleListValue("descriptives", item.value)}
                />
                {item.label}
              </label>
            ))}
          </div>
          <h3>Plots</h3>
          <div className="wizard-grid">
            {PLOT_OPTIONS.map((item) => (
              <label key={item.value} className="checkbox compact">
                <input
                  type="checkbox"
                  checked={options.plots.includes(item.value)}
                  onChange={() => toggleListValue("plots", item.value)}
                />
                {item.label}
              </label>
            ))}
          </div>
        </div>
      );
    }

    if (step === 2) {
      return (
        <div className="wizard-step">
          <div className="wizard-grid">
            {BALANCE_OPTIONS.map((item) => (
              <label key={item.value} className="checkbox compact">
                <input
                  type="checkbox"
                  checked={options.balanceChecks.includes(item.value)}
                  onChange={() => toggleListValue("balanceChecks", item.value)}
                />
                {item.label}
              </label>
            ))}
          </div>
        </div>
      );
    }

    if (step === 3) {
      return (
        <div className="wizard-step">
          <div className="wizard-grid">
            {MODEL_OPTIONS.map((item) => (
              <label key={item.value} className="checkbox compact">
                <input
                  type="checkbox"
                  checked={options.models.includes(item.value)}
                  onChange={() => toggleListValue("models", item.value)}
                />
                {item.label}
              </label>
            ))}
          </div>
          {needsPanelHint && (
            <p className="muted">
              DiD/Event-study selected. Keep ID/time/treatment hints filled; if blank,
              scaffold TODOs will be generated.
            </p>
          )}
        </div>
      );
    }

    if (step === 4) {
      const suggested = new Set(suggestDiagnostics(options.models));
      return (
        <div className="wizard-step">
          <p className="muted">Suggested diagnostics are pre-checked based on model selection.</p>
          <div className="wizard-grid">
            {DIAGNOSTIC_OPTIONS.map((item) => (
              <label key={item.value} className="checkbox compact">
                <input
                  type="checkbox"
                  checked={options.diagnostics.includes(item.value)}
                  onChange={() => toggleListValue("diagnostics", item.value)}
                />
                {item.label}
                {suggested.has(item.value) ? " (suggested)" : ""}
              </label>
            ))}
          </div>
        </div>
      );
    }

    if (step === 5) {
      return (
        <div className="wizard-step">
          <h3>Tables</h3>
          <div className="wizard-grid">
            {TABLE_OPTIONS.map((item) => (
              <label key={item.value} className="checkbox compact">
                <input
                  type="checkbox"
                  checked={options.tables.includes(item.value)}
                  onChange={() => toggleListValue("tables", item.value)}
                />
                {item.label}
              </label>
            ))}
          </div>
          <h3>Robustness scaffolding</h3>
          <div className="wizard-grid">
            {ROBUSTNESS_OPTIONS.map((item) => (
              <label key={item} className="checkbox compact">
                <input
                  type="checkbox"
                  checked={options.robustness.includes(item)}
                  onChange={() => toggleListValue("robustness", item)}
                />
                {item}
              </label>
            ))}
          </div>
          <label className="checkbox">
            <input
              type="checkbox"
              checked={options.exploratory}
              onChange={(event) =>
                setOptions((prev) => ({ ...prev, exploratory: event.target.checked }))
              }
            />
            Include exploratory analyses section
          </label>
          <label className="checkbox">
            <input
              type="checkbox"
              checked={options.exportArtifacts}
              onChange={(event) =>
                setOptions((prev) => ({ ...prev, exportArtifacts: event.target.checked }))
              }
            />
            Include export scaffolding to {"07_outputs/{tables,figures,reports}"}
          </label>
          <p className="muted">
            Output paths: 07_outputs/tables, 07_outputs/figures, 07_outputs/reports
          </p>
        </div>
      );
    }

    return (
      <div className="wizard-step">
        <h3>Review</h3>
        <ul className="wizard-review-list">
          <li>Analysis file: {(options.analysisFileName || "analysis").trim() || "analysis"}.Rmd</li>
          <li>Dataset hint: {options.datasetPathHint || "(blank)"}</li>
          <li>Descriptives: {options.descriptives.join(", ") || "none"}</li>
          <li>Plots: {options.plots.join(", ") || "none"}</li>
          <li>Balance checks: {options.balanceChecks.join(", ") || "none"}</li>
          <li>Models: {options.models.join(", ") || "none"}</li>
          <li>Diagnostics: {options.diagnostics.join(", ") || "none"}</li>
          <li>Tables: {options.tables.join(", ") || "none"}</li>
          <li>Robustness: {options.robustness.join(", ") || "none"}</li>
          <li>Exploratory section: {options.exploratory ? "yes" : "no"}</li>
          <li>Export artifacts: {options.exportArtifacts ? "yes" : "no"}</li>
        </ul>
      </div>
    );
  };

  const isLastStep = step === STEP_TITLES.length - 1;

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal wizard-modal" onClick={(event) => event.stopPropagation()}>
        <div className="modal-header">
          <h2>Analysis Template Wizard</h2>
          <button className="ghost" onClick={onClose}>
            Close
          </button>
        </div>
        <div className="modal-body">
          <p className="muted">
            Study: {studyId} | Step {step + 1} of {STEP_TITLES.length} ({STEP_TITLES[step]})
          </p>
          {renderStep()}
        </div>
        <div className="modal-actions">
          <button className="ghost" onClick={() => setStep((prev) => Math.max(0, prev - 1))} disabled={step === 0 || loading}>
            Back
          </button>
          {!isLastStep && (
            <button onClick={() => setStep((prev) => Math.min(STEP_TITLES.length - 1, prev + 1))} disabled={loading}>
              Next
            </button>
          )}
          {isLastStep && (
            <button
              onClick={async () => {
                await onSubmit(options);
              }}
              disabled={loading}
            >
              Create Analysis Template
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
