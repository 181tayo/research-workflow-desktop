import { invoke } from "@tauri-apps/api/tauri";

export type AssetRef = { name: string; path: string };

export const listBuildAssets = (projectId: string, studyId: string) =>
  invoke<AssetRef[]>("list_build_assets", { projectId, studyId });

export const listPreregAssets = (projectId: string, studyId: string) =>
  invoke<AssetRef[]>("list_prereg_assets", { projectId, studyId });

export const generateAnalysisSpec = (payload: {
  projectId: string;
  studyId: string;
  analysisId: string;
  qsfPath: string;
  preregPath: string;
  templateSet: string;
  styleProfile: string;
}) => invoke("generate_analysis_spec", { args: payload });

export const saveAnalysisSpec = (payload: {
  projectId: string;
  studyId: string;
  analysisId: string;
  spec: unknown;
}) => invoke("save_analysis_spec", { args: payload });

export const resolveMappings = (payload: {
  projectId: string;
  studyId: string;
  analysisId: string;
  mappingUpdates: Array<{ preregVar: string; resolvedTo: string }>;
}) => invoke("resolve_mappings", { args: payload });

export const renderAnalysisFromSpec = (payload: {
  projectId: string;
  studyId: string;
  analysisId: string;
}) => invoke<{ rmdPath: string; rPath: string }>("render_analysis_from_spec", { args: payload });
