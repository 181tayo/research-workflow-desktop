import { useEffect, useMemo, useState } from "react";
import {
  AnalysisTemplateOptions,
  BalanceCheck,
  DescriptiveBlock,
  Diagnostic,
  ModelFigureType,
  ModelLayout,
  ModelLayoutKind,
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
  initialOptions?: Partial<AnalysisTemplateOptions> | null;
  onPickDataSources: () => Promise<string[]>;
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

const MODEL_LAYOUT_OPTIONS: Array<{ value: ModelLayoutKind; label: string }> = [
  { value: "simple", label: "Simple model" },
  { value: "interaction", label: "Interaction model" }
];

const MODEL_FIGURE_OPTIONS: Array<{ value: ModelFigureType; label: string }> = [
  { value: "coef_plot", label: "Coefficient plot" },
  { value: "fitted_plot", label: "Fitted vs observed" },
  { value: "residual_plot", label: "Residual plot" },
  { value: "event_study_plot", label: "Event-study plot" }
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
  "Diagnostics",
  "Tables/Figures & Exports",
  "Review + Create"
];

const defaultOptions = (): AnalysisTemplateOptions => ({
  analysisFileName: "analysis",
  dataSourcePaths: [],
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
  modelLayouts: [],
  exploratory: false,
  exportArtifacts: true
});

const defaultModelLayoutDraft = (): ModelLayout => ({
  name: "",
  modelType: "ols",
  outcomeVar: "y",
  treatmentVar: "treat",
  layout: "simple",
  interactionVar: "",
  covariates: "",
  idVar: "id",
  timeVar: "time",
  figures: ["coef_plot"],
  includeInMainTable: true
});

const buildDraftFormulaPreview = (draft: ModelLayout): string => {
  const outcome = (draft.outcomeVar || "").trim() || "y";
  const treatment = (draft.treatmentVar || "").trim() || "treat";
  const interaction = (draft.interactionVar || "").trim() || "moderator_var";
  const covariates = (draft.covariates || "").trim();

  let rhs = draft.layout === "interaction" ? `(${treatment}) * ${interaction}` : treatment;
  if (covariates) rhs += ` + ${covariates}`;

  return `${outcome} ~ ${rhs}`;
};

export function AnalysisTemplateWizard({
  isOpen,
  projectId,
  studyId,
  loading,
  initialOptions,
  onPickDataSources,
  onClose,
  onSubmit
}: WizardProps) {
  const [step, setStep] = useState(0);
  const [options, setOptions] = useState<AnalysisTemplateOptions>(defaultOptions);
  const [modelLayoutDraft, setModelLayoutDraft] = useState<ModelLayout>(defaultModelLayoutDraft);
  const [showAllDataSources, setShowAllDataSources] = useState(false);

  useEffect(() => {
    if (!isOpen) return;
    setStep(0);
    setOptions(mergeInitialOptions(initialOptions));
    setModelLayoutDraft(defaultModelLayoutDraft());
    setShowAllDataSources(false);
  }, [isOpen, projectId, studyId, initialOptions]);

  const selectedModelTypes = useMemo(
    () =>
      Array.from(
        new Set([
          ...options.models,
          ...((options.modelLayouts ?? []).map((item) => item.modelType))
        ])
      ),
    [options.models, options.modelLayouts]
  );
  const modelKey = useMemo(() => selectedModelTypes.join("|"), [selectedModelTypes]);

  useEffect(() => {
    if (!isOpen) return;
    const suggestions = suggestDiagnostics(selectedModelTypes);
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
  }, [isOpen, modelKey, selectedModelTypes]);

  if (!isOpen) return null;

  const textEntryProps = {
    autoCapitalize: "none" as const,
    autoCorrect: "off" as const,
    spellCheck: false
  };

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

  const toggleModelFigure = (figure: ModelFigureType) => {
    setModelLayoutDraft((prev) => {
      const current = prev.figures ?? [];
      const next = current.includes(figure)
        ? current.filter((value) => value !== figure)
        : [...current, figure];
      return { ...prev, figures: next };
    });
  };

  const addModelLayout = () => {
    const outcome = (modelLayoutDraft.outcomeVar || "").trim();
    const treatment = (modelLayoutDraft.treatmentVar || "").trim();
    if (!outcome || !treatment) return;

    const modelName =
      modelLayoutDraft.name.trim() ||
      `${modelLayoutDraft.modelType.toUpperCase()}_${outcome.replace(/[^A-Za-z0-9]+/g, "_")}`;

    const nextLayout: ModelLayout = {
      ...modelLayoutDraft,
      name: modelName,
      outcomeVar: outcome,
      treatmentVar: treatment,
      covariates: (modelLayoutDraft.covariates || "").trim(),
      interactionVar: (modelLayoutDraft.interactionVar || "").trim(),
      idVar: (modelLayoutDraft.idVar || "").trim(),
      timeVar: (modelLayoutDraft.timeVar || "").trim()
    };

    setOptions((prev) => ({
      ...prev,
      modelLayouts: [...(prev.modelLayouts ?? []), nextLayout],
      models: Array.from(new Set([...prev.models, nextLayout.modelType]))
    }));
    setModelLayoutDraft(defaultModelLayoutDraft());
  };

  const renderStep = () => {
    if (step === 0) {
      return (
        <div className="wizard-step">
          <p className="muted">These are placeholders only; replace during cleaning.</p>
          <label>
            Analysis file name (without extension)
            <input
              {...textEntryProps}
              value={options.analysisFileName ?? ""}
              onChange={(event) =>
                setOptions((prev) => ({ ...prev, analysisFileName: event.target.value }))
              }
              placeholder="analysis or pilot_analysis"
            />
          </label>
          <label>
            Data sources (optional, multi-select)
            <div className="inline-field">
              <input
                {...textEntryProps}
                value={options.dataSourcePaths?.length ? `${options.dataSourcePaths.length} files selected` : ""}
                readOnly
                placeholder="Choose data files to preload in this analysis"
              />
              <button
                type="button"
                className="ghost"
                disabled={loading}
                onClick={async () => {
                  const selected = await onPickDataSources();
                  if (selected.length === 0) return;
                  setOptions((prev) => ({
                    ...prev,
                    dataSourcePaths: Array.from(new Set([...(prev.dataSourcePaths ?? []), ...selected]))
                  }));
                }}
              >
                Choose
              </button>
              <button
                type="button"
                className="ghost"
                disabled={loading || !(options.dataSourcePaths && options.dataSourcePaths.length > 0)}
                onClick={() => setOptions((prev) => ({ ...prev, dataSourcePaths: [] }))}
              >
                Clear
              </button>
            </div>
          </label>
          {options.dataSourcePaths && options.dataSourcePaths.length > 0 && (
            <>
              <ul className="selected-data-list">
                {(showAllDataSources ? options.dataSourcePaths : options.dataSourcePaths.slice(0, 5)).map((path) => (
                <li key={path} className="selected-data-item">
                  <span title={path}>{path}</span>
                  <button
                    type="button"
                    className="ghost"
                    onClick={() =>
                      setOptions((prev) => ({
                        ...prev,
                        dataSourcePaths: (prev.dataSourcePaths ?? []).filter((item) => item !== path)
                      }))
                    }
                  >
                    Remove
                  </button>
                </li>
                ))}
              </ul>
              {options.dataSourcePaths.length > 5 && (
                <button
                  type="button"
                  className="ghost"
                  onClick={() => setShowAllDataSources((prev) => !prev)}
                >
                  {showAllDataSources ? "Collapse list" : `Show all (${options.dataSourcePaths.length})`}
                </button>
              )}
            </>
          )}
          <label>
            Dataset path hint
            <input
              {...textEntryProps}
              value={options.datasetPathHint ?? ""}
              onChange={(event) =>
                setOptions((prev) => ({ ...prev, datasetPathHint: event.target.value }))
              }
            />
          </label>
          <h3>Model Layout Builder</h3>
          <p className="muted">
            Add all intended models here. Model-dependent sections run across this list.
          </p>
          <div className="wizard-grid">
            <label>
              Model name
              <input
                {...textEntryProps}
                value={modelLayoutDraft.name}
                onChange={(event) =>
                  setModelLayoutDraft((prev) => ({ ...prev, name: event.target.value }))
                }
                placeholder="e.g., Main OLS"
              />
            </label>
            <label>
              Outcome variable
              <input
                {...textEntryProps}
                value={modelLayoutDraft.outcomeVar}
                onChange={(event) =>
                  setModelLayoutDraft((prev) => ({ ...prev, outcomeVar: event.target.value }))
                }
                placeholder="y"
              />
            </label>
            <label>
              Treatment variable(s)
              <input
                {...textEntryProps}
                value={modelLayoutDraft.treatmentVar}
                onChange={(event) =>
                  setModelLayoutDraft((prev) => ({ ...prev, treatmentVar: event.target.value }))
                }
                placeholder="x1 + x2"
              />
            </label>
            <label>
              Model family
              <select
                value={modelLayoutDraft.modelType}
                onChange={(event) =>
                  setModelLayoutDraft((prev) => ({ ...prev, modelType: event.target.value as ModelType }))
                }
              >
                {MODEL_OPTIONS.map((item) => (
                  <option key={item.value} value={item.value}>
                    {item.label}
                  </option>
                ))}
              </select>
            </label>
            <label>
              Layout
              <select
                value={modelLayoutDraft.layout}
                onChange={(event) =>
                  setModelLayoutDraft((prev) => ({ ...prev, layout: event.target.value as ModelLayoutKind }))
                }
              >
                {MODEL_LAYOUT_OPTIONS.map((item) => (
                  <option key={item.value} value={item.value}>
                    {item.label}
                  </option>
                ))}
              </select>
            </label>
            {modelLayoutDraft.layout === "interaction" && (
              <label>
                Group/interaction variable
                <input
                  {...textEntryProps}
                  value={modelLayoutDraft.interactionVar ?? ""}
                  onChange={(event) =>
                    setModelLayoutDraft((prev) => ({ ...prev, interactionVar: event.target.value }))
                  }
                  placeholder="moderator_var"
                />
              </label>
            )}
            {(modelLayoutDraft.modelType === "mixed_effects" ||
              modelLayoutDraft.modelType === "fixed_effects" ||
              modelLayoutDraft.modelType === "did" ||
              modelLayoutDraft.modelType === "event_study") && (
              <label>
                ID variable
                <input
                  {...textEntryProps}
                  value={modelLayoutDraft.idVar ?? ""}
                  onChange={(event) =>
                    setModelLayoutDraft((prev) => ({ ...prev, idVar: event.target.value }))
                  }
                  placeholder="id"
                />
              </label>
            )}
            {(modelLayoutDraft.modelType === "fixed_effects" ||
              modelLayoutDraft.modelType === "did" ||
              modelLayoutDraft.modelType === "event_study") && (
              <label>
                Time variable
                <input
                  {...textEntryProps}
                  value={modelLayoutDraft.timeVar ?? ""}
                  onChange={(event) =>
                    setModelLayoutDraft((prev) => ({ ...prev, timeVar: event.target.value }))
                  }
                  placeholder="time"
                />
              </label>
            )}
            <label>
              Covariates (R formula terms)
              <input
                {...textEntryProps}
                value={modelLayoutDraft.covariates ?? ""}
                onChange={(event) =>
                  setModelLayoutDraft((prev) => ({ ...prev, covariates: event.target.value }))
                }
                placeholder="age + sex + baseline_score"
              />
            </label>
          </div>
          <p className="muted">
            Formula preview: <code>{buildDraftFormulaPreview(modelLayoutDraft)}</code>
          </p>
          <div className="wizard-grid">
            {MODEL_FIGURE_OPTIONS.map((item) => (
              <label key={item.value} className="checkbox compact">
                <input
                  type="checkbox"
                  checked={(modelLayoutDraft.figures ?? []).includes(item.value)}
                  onChange={() => toggleModelFigure(item.value)}
                />
                {item.label}
              </label>
            ))}
          </div>
          <label className="checkbox">
            <input
              type="checkbox"
              checked={modelLayoutDraft.includeInMainTable}
              onChange={(event) =>
                setModelLayoutDraft((prev) => ({ ...prev, includeInMainTable: event.target.checked }))
              }
            />
            Include this model in main regression table
          </label>
          <div className="modal-actions">
            <button
              type="button"
              onClick={addModelLayout}
              disabled={
                loading ||
                !(modelLayoutDraft.outcomeVar || "").trim() ||
                !(modelLayoutDraft.treatmentVar || "").trim()
              }
            >
              Add model
            </button>
          </div>
          {(options.modelLayouts ?? []).length > 0 && (
            <ul className="selected-data-list">
              {(options.modelLayouts ?? []).map((item, index) => (
                <li key={`${item.name}-${index}`} className="selected-data-item">
                  <span title={item.name}>
                    {item.name} | {item.modelType} | {item.outcomeVar} ~ {item.layout === "interaction" ? `(${item.treatmentVar}) * ${item.interactionVar || "moderator_var"}` : item.treatmentVar}
                    {(item.covariates || "").trim() ? ` + ${item.covariates}` : ""}
                    {" | "}
                    figures: {item.figures.join(", ") || "none"} | main table: {item.includeInMainTable ? "yes" : "no"}
                  </span>
                  <button
                    type="button"
                    className="ghost"
                    onClick={() =>
                      setOptions((prev) => ({
                        ...prev,
                        modelLayouts: (prev.modelLayouts ?? []).filter((_, i) => i !== index)
                      }))
                    }
                  >
                    Remove
                  </button>
                </li>
              ))}
            </ul>
          )}
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
      const suggested = new Set(suggestDiagnostics(selectedModelTypes));
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

    if (step === 4) {
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
          <li>Data sources selected: {options.dataSourcePaths?.length ?? 0}</li>
          <li>Dataset hint: {options.datasetPathHint || "(blank)"}</li>
          <li>Descriptives: {options.descriptives.join(", ") || "none"}</li>
          <li>Plots: {options.plots.join(", ") || "none"}</li>
          <li>Balance checks: {options.balanceChecks.join(", ") || "none"}</li>
          <li>Model layouts: {(options.modelLayouts ?? []).length}</li>
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

const mergeInitialOptions = (
  initialOptions?: Partial<AnalysisTemplateOptions> | null
): AnalysisTemplateOptions => {
  const base = defaultOptions();
  if (!initialOptions) return base;
  return {
    ...base,
    ...initialOptions,
    descriptives: initialOptions.descriptives ?? base.descriptives,
    plots: initialOptions.plots ?? base.plots,
    balanceChecks: initialOptions.balanceChecks ?? base.balanceChecks,
    models: initialOptions.models ?? base.models,
    diagnostics: initialOptions.diagnostics ?? base.diagnostics,
    tables: initialOptions.tables ?? base.tables,
    robustness: initialOptions.robustness ?? base.robustness,
    modelLayouts: initialOptions.modelLayouts ?? base.modelLayouts
  };
};
