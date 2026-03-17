export type NodePersistState = {
  collapsed?: boolean;
  pinned?: boolean;
  pinnedX?: number;
  pinnedY?: number;
};

export type DisplaySettings = {
  includeThirdPartyPackages: boolean;
  includeStandardPackages: boolean;
  visibleEdgeDepth: number;
  selectorQuery?: string;
  selectorExclude?: string;
};

export type LayoutSettings = {
  rootDirection: "RIGHT" | "DOWN";
  // ELK spacing
  rootNodeBetweenLayers: number;
  rootNodeNode: number;
  childNodeBetweenLayers: number;
  childNodeNode: number;
  edgeNode: number;
  componentComponent: number;
  padding: number;
  // Bezier edge
  edgeCurvature: number;
  edgeStrokeWidth: number;
};

export const DEFAULT_LAYOUT_SETTINGS: LayoutSettings = {
  rootDirection: "RIGHT",
  rootNodeBetweenLayers: 150,
  rootNodeNode: 100,
  childNodeBetweenLayers: 50,
  childNodeNode: 50,
  edgeNode: 50,
  componentComponent: 20,
  padding: 50,
  edgeCurvature: 40,
  edgeStrokeWidth: 1.2,
};

export type PersistedLayoutData = {
  version: 1;
  settings: Partial<DisplaySettings>;
  nodes: Record<string, NodePersistState>;
  layoutSettings?: Partial<LayoutSettings>;
};
