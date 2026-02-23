import { useEffect, useMemo, useState } from "react";
import { AnalysisTemplateOptions, ModelLayout, ModelType } from "../types/analysisTemplate";
import { generateAnalysisSpec, listBuildAssets, listPreregAssets, saveAnalysisSpec } from "../tauri/api";
import { setAnalysisSpec } from "../state/analysisStore";
import { WarningsPanel } from "./WarningsPanel";

type Props = {
  projectId: string;
  studyId: string;
  onUseInBuilder: (options: Partial<AnalysisTemplateOptions>) => void;
};

type Confidence = "high" | "medium" | "low";

type MappingRow = {
  preregVar: string;
  resolvedTo: string | null;
  topCandidate: string | null;
  topScore: number;
  candidates: Array<{ key: string; score: number }>;
  confidence: Confidence;
};

const HIGH_CONFIDENCE = 0.9;
const MEDIUM_CONFIDENCE = 0.75;

export function AnalysisCreateFromInputs({ projectId, studyId, onUseInBuilder }: Props) {
  const [buildAssets, setBuildAssets] = useState<Array<{ name: string; path: string }>>([]);
  const [preregAssets, setPreregAssets] = useState<Array<{ name: string; path: string }>>([]);
  const [qsfPath, setQsfPath] = useState("");
  const [preregPath, setPreregPath] = useState("");
  const [analysisId, setAnalysisId] = useState("analysis");
  const [status, setStatus] = useState("");
  const [spec, setSpec] = useState<any | null>(null);
  const [selectedMappings, setSelectedMappings] = useState<Record<string, string>>({});
  const [selectedDvs, setSelectedDvs] = useState<string[]>([]);
  const [selectedIvs, setSelectedIvs] = useState<string[]>([]);
  const [selectedControls, setSelectedControls] = useState<string[]>([]);
  const [templateChoice, setTemplateChoice] = useState<"auto" | "factorial_2x2" | "simple_ols">("auto");

  useEffect(() => {
    listBuildAssets(projectId, studyId).then(setBuildAssets).catch(() => setBuildAssets([]));
    listPreregAssets(projectId, studyId).then(setPreregAssets).catch(() => setPreregAssets([]));
  }, [projectId, studyId]);

  const mappingRows = useMemo(() => getMappingRows(spec), [spec]);
  const inventory = useMemo(() => getAnalyzableColumns(spec), [spec]);

  const onCreate = async () => {
    try {
      setStatus("Generating mapping + model suggestions...");
      const generated: any = await generateAnalysisSpec({
        projectId,
        studyId,
        analysisId,
        qsfPath,
        preregPath,
        templateSet: "apa_v1",
        styleProfile: "apa_flextable_ggpubr"
      });
      setSpec(generated);
      setAnalysisSpec(generated);

      const autoSelected: Record<string, string> = {};
      for (const row of getMappingRows(generated)) {
        if (row.confidence === "high" && row.topCandidate) {
          autoSelected[row.preregVar] = row.topCandidate;
        } else if (row.resolvedTo) {
          autoSelected[row.preregVar] = row.resolvedTo;
        }
      }
      setSelectedMappings(autoSelected);

      const seeded = seedManualSelections(generated, autoSelected);
      setSelectedDvs(seeded.dv);
      setSelectedIvs(seeded.iv);
      setSelectedControls(seeded.controls);
      setTemplateChoice((generated?.models?.main?.length ?? 0) > 0 ? "auto" : "factorial_2x2");

      setStatus("Review mappings and plan, then continue to model builder.");
    } catch (err) {
      setStatus(`Error: ${formatError(err)}`);
    }
  };

  const unresolvedLow = mappingRows.filter((row) => row.confidence === "low" && !selectedMappings[row.preregVar]);

  const prefillOptions = useMemo(() => {
    if (!spec) return null;
    return specToWizardOptions(spec, analysisId, selectedMappings, selectedDvs, selectedIvs, selectedControls, templateChoice);
  }, [spec, analysisId, selectedMappings, selectedDvs, selectedIvs, selectedControls, templateChoice]);

  const onUseSuggestions = async () => {
    if (!spec || !prefillOptions) return;
    if (unresolvedLow.length > 0) {
      setStatus("Select mappings for low-confidence items before continuing.");
      return;
    }

    try {
      const updatedSpec = applyMappingsToSpec(spec, selectedMappings);
      await saveAnalysisSpec({
        projectId,
        studyId,
        analysisId,
        spec: updatedSpec
      });
      setAnalysisSpec(updatedSpec);
      setStatus("Saved spec. Opening model builder...");
      onUseInBuilder(prefillOptions);
    } catch (err) {
      setStatus(`Error: ${formatError(err)}`);
    }
  };

  return (
    <div>
      <h3>Prereg + QSF Mapping Wizard</h3>
      <p className="muted">Offline-first deterministic flow. Review mappings, confirm plan, then prefill the existing model builder.</p>

      <input value={analysisId} onChange={(e) => setAnalysisId(e.target.value)} placeholder="analysis id" />
      <select value={qsfPath} onChange={(e) => setQsfPath(e.target.value)}>
        <option value="">Select QSF</option>
        {buildAssets.map((a) => (
          <option key={a.path} value={a.path}>{a.name}</option>
        ))}
      </select>
      <select value={preregPath} onChange={(e) => setPreregPath(e.target.value)}>
        <option value="">Select prereg</option>
        {preregAssets.map((a) => (
          <option key={a.path} value={a.path}>{a.name}</option>
        ))}
      </select>
      <button onClick={onCreate} disabled={!qsfPath || !preregPath}>Build Suggestions</button>
      <p>{status}</p>

      {spec && <WarningsPanel warnings={spec.warnings ?? []} />}

      {mappingRows.length > 0 && (
        <div>
          <h4>Variable Mapping</h4>
          <div
            style={{
              maxHeight: 320,
              overflowY: "auto",
              border: "1px solid #ddd",
              borderRadius: 6,
              padding: 10
            }}
          >
            {mappingRows.map((row) => {
              const selected = selectedMappings[row.preregVar] ?? "";
              return (
                <div key={row.preregVar} style={{ marginBottom: 10 }}>
                  <div>
                    <strong>{row.preregVar}</strong> <span className="muted">({row.confidence}, top {row.topScore.toFixed(2)})</span>
                  </div>
                  <div>
                    {row.confidence === "high" && row.topCandidate && (
                      <span className="muted">Auto-accepted: {row.topCandidate}</span>
                    )}
                    {row.confidence === "medium" && row.topCandidate && !selected && (
                      <button
                        onClick={() => setSelectedMappings((prev) => ({ ...prev, [row.preregVar]: row.topCandidate as string }))}
                      >
                        Accept Suggested Match
                      </button>
                    )}
                  </div>
                  <select
                    value={selected}
                    onChange={(e) => setSelectedMappings((prev) => ({ ...prev, [row.preregVar]: e.target.value }))}
                  >
                    <option value="">{row.confidence === "low" ? "Select mapping (required)" : "Optional override"}</option>
                    {row.candidates.map((c) => (
                      <option key={`${row.preregVar}-${c.key}`} value={c.key}>
                        {c.key} ({c.score.toFixed(2)})
                      </option>
                    ))}
                  </select>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {inventory.length > 0 && (
        <div>
          <h4>Manual Analysis Plan</h4>
          <p className="muted">Choose variables directly from QSF inventory.</p>

          <label>Dependent variable(s)</label>
          <select multiple value={selectedDvs} onChange={(e) => setSelectedDvs(getMultiSelectValues(e.currentTarget))}>
            {inventory.map((col) => (
              <option key={`dv-${col}`} value={col}>{col}</option>
            ))}
          </select>

          <label>Independent variable(s)</label>
          <select multiple value={selectedIvs} onChange={(e) => setSelectedIvs(getMultiSelectValues(e.currentTarget))}>
            {inventory.map((col) => (
              <option key={`iv-${col}`} value={col}>{col}</option>
            ))}
          </select>

          <label>Controls</label>
          <select multiple value={selectedControls} onChange={(e) => setSelectedControls(getMultiSelectValues(e.currentTarget))}>
            {inventory.map((col) => (
              <option key={`ctrl-${col}`} value={col}>{col}</option>
            ))}
          </select>

          <label>Template (used when models are not extracted)</label>
          <select value={templateChoice} onChange={(e) => setTemplateChoice(e.target.value as "auto" | "factorial_2x2" | "simple_ols") }>
            <option value="auto">Auto from extracted models</option>
            <option value="factorial_2x2">2x2 factorial (interaction)</option>
            <option value="simple_ols">Simple OLS</option>
          </select>
        </div>
      )}

      {prefillOptions && (
        <button onClick={onUseSuggestions}>Save + Use In Model Builder</button>
      )}
    </div>
  );
}

function getMappingRows(spec: any | null): MappingRow[] {
  if (!spec?.variableMappings) return [];
  return (spec.variableMappings as any[]).map((m) => {
    const candidates = Array.isArray(m?.candidates) ? m.candidates : [];
    const top = candidates[0] ?? null;
    const topScore = Number(top?.score ?? 0);
    const confidence: Confidence = topScore >= HIGH_CONFIDENCE ? "high" : topScore >= MEDIUM_CONFIDENCE ? "medium" : "low";
    return {
      preregVar: String(m?.preregVar ?? ""),
      resolvedTo: m?.resolvedTo ? String(m.resolvedTo) : null,
      topCandidate: top?.key ? String(top.key) : null,
      topScore,
      candidates: candidates.map((c: any) => ({ key: String(c.key), score: Number(c.score ?? 0) })),
      confidence
    };
  }).filter((r) => r.preregVar);
}

function getAnalyzableColumns(spec: any | null): string[] {
  const cols = (spec?.dataContract?.expectedColumns ?? []) as string[];
  const blocked = new Set([
    "responseid",
    "recipientlastname",
    "recipientfirstname",
    "recipientemail",
    "externalreference",
    "locationlatitude",
    "locationlongitude",
    "distributionchannel",
    "userlanguage",
    "startdate",
    "enddate",
    "status",
    "ipaddress",
    "progress",
    "durationinseconds",
    "finished"
  ]);
  return cols
    .filter((c) => !!c)
    .filter((c) => !blocked.has(c.toLowerCase()))
    .filter((c) => !c.toLowerCase().startsWith("qid"))
    .sort();
}

function seedManualSelections(spec: any, selectedMappings: Record<string, string>) {
  const mapVar = (value: string) => selectedMappings[value] || value;
  const main = (spec?.models?.main ?? []) as any[];
  const dv = Array.from(new Set(main.map((m) => mapVar(String(m?.dv ?? ""))).filter(Boolean)));
  const iv = Array.from(new Set(main.flatMap((m) => (m?.iv ?? []).map((v: string) => mapVar(v))).filter(Boolean)));
  const controls = Array.from(new Set(main.flatMap((m) => (m?.controls ?? []).map((v: string) => mapVar(v))).filter(Boolean)));
  return { dv, iv, controls };
}

function applyMappingsToSpec(spec: any, selectedMappings: Record<string, string>) {
  const clone = JSON.parse(JSON.stringify(spec));
  const unresolved = new Set<string>();
  clone.variableMappings = (clone.variableMappings ?? []).map((m: any) => {
    const preregVar = String(m?.preregVar ?? "");
    const resolvedTo = selectedMappings[preregVar] || m?.resolvedTo || null;
    if (!resolvedTo) unresolved.add(preregVar);
    return { ...m, resolvedTo };
  });

  clone.warnings = (clone.warnings ?? []).filter((w: any) => {
    if (w?.code !== "UNRESOLVED_VARIABLE") return true;
    const pv = String(w?.details?.preregVar ?? "");
    return unresolved.has(pv);
  });
  return clone;
}

function specToWizardOptions(
  spec: any,
  analysisId: string,
  selectedMappings: Record<string, string>,
  selectedDvs: string[],
  selectedIvs: string[],
  selectedControls: string[],
  templateChoice: "auto" | "factorial_2x2" | "simple_ols"
): Partial<AnalysisTemplateOptions> {
  const extractedLayouts = buildLayoutsFromExtractedModels(spec, selectedMappings);
  const fallbackLayouts = buildLayoutsFromManualSelection(selectedDvs, selectedIvs, selectedControls, templateChoice);
  const modelLayouts = templateChoice === "auto" && extractedLayouts.length > 0 ? extractedLayouts : fallbackLayouts;

  const first = modelLayouts[0];
  const modelTypes = Array.from(new Set(modelLayouts.map((m) => m.modelType)));

  return {
    analysisFileName: analysisId || "analysis",
    outcomeVarHint: first?.outcomeVar || selectedDvs[0] || "y",
    treatmentVarHint: first?.treatmentVar || selectedIvs[0] || "treat",
    groupVarHint: first?.interactionVar || selectedIvs[1] || "group",
    descriptives: ["summary_stats", "missingness", "group_summary"],
    plots: ["boxplot", "coef_plot"],
    balanceChecks: ["baseline_table", "randomization_check"],
    models: modelTypes,
    diagnostics: [],
    tables: ["table1_descriptives", "model_table", "balance_table"],
    robustness: spec?.models?.robustness?.length ? ["alt_controls"] : [],
    modelLayouts,
    exploratory: Boolean(spec?.models?.exploratory?.length),
    exportArtifacts: true
  };
}

function buildLayoutsFromExtractedModels(spec: any, selectedMappings: Record<string, string>): ModelLayout[] {
  const mapVar = (value: string) => selectedMappings[value] || value;
  const mainModels = (spec?.models?.main ?? []) as any[];
  return mainModels.map((m, idx) => {
    const iv = ((m?.iv ?? []) as string[]).map((v) => mapVar(v));
    const controls = ((m?.controls ?? []) as string[]).map((v) => mapVar(v));
    const interaction = extractInteractionVar(m?.interactions ?? []);
    return {
      name: String(m?.id || `model_${idx + 1}`),
      modelType: mapFamilyToModelType(String(m?.family ?? "gaussian")),
      outcomeVar: mapVar(String(m?.dv ?? "y")),
      treatmentVar: iv[0] || "treat",
      layout: interaction ? "interaction" : "simple",
      interactionVar: interaction,
      covariates: controls.join(", "),
      idVar: "id",
      timeVar: "time",
      figures: ["coef_plot"],
      includeInMainTable: true
    };
  });
}

function buildLayoutsFromManualSelection(
  selectedDvs: string[],
  selectedIvs: string[],
  selectedControls: string[],
  templateChoice: "auto" | "factorial_2x2" | "simple_ols"
): ModelLayout[] {
  if (selectedDvs.length === 0 || selectedIvs.length === 0) return [];

  if (templateChoice === "factorial_2x2" && selectedIvs.length >= 2) {
    return selectedDvs.map((dv, idx) => ({
      name: `factorial_${idx + 1}`,
      modelType: "ols",
      outcomeVar: dv,
      treatmentVar: selectedIvs[0],
      layout: "interaction",
      interactionVar: selectedIvs[1],
      covariates: selectedControls.join(", "),
      idVar: "id",
      timeVar: "time",
      figures: ["coef_plot"],
      includeInMainTable: true
    }));
  }

  return selectedDvs.map((dv, idx) => ({
    name: `model_${idx + 1}`,
    modelType: "ols",
    outcomeVar: dv,
    treatmentVar: selectedIvs[0],
    layout: "simple",
    interactionVar: "",
    covariates: selectedControls.join(", "),
    idVar: "id",
    timeVar: "time",
    figures: ["coef_plot"],
    includeInMainTable: true
  }));
}

function getMultiSelectValues(select: HTMLSelectElement): string[] {
  return Array.from(select.selectedOptions).map((option) => option.value);
}

function extractInteractionVar(interactions: string[]): string {
  if (!Array.isArray(interactions) || interactions.length === 0) return "";
  const first = interactions[0] || "";
  const parts = String(first).split(":").map((v) => v.trim()).filter(Boolean);
  return parts[1] || "";
}

function mapFamilyToModelType(family: string): ModelType {
  const lower = family.toLowerCase();
  if (lower.includes("binomial") || lower.includes("logit")) return "logit";
  if (lower.includes("poisson")) return "poisson";
  return "ols";
}

function formatError(err: unknown): string {
  if (typeof err === "string") return err;
  if (err && typeof err === "object" && "message" in err) {
    return String((err as any).message);
  }
  return "Unknown error";
}
