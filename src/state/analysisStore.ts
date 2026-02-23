export type AnalysisState = {
  spec: any | null;
  warnings: any[];
};

let state: AnalysisState = {
  spec: null,
  warnings: []
};

export const getAnalysisState = () => state;

export const setAnalysisSpec = (spec: any) => {
  state = { ...state, spec, warnings: spec?.warnings ?? [] };
};
