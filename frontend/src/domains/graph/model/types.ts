export type CfgNode = {
  id: string;
  block: string;
  line: number;
  column: number;
  type: string;
  text: string;
};

export type CfgEdgeType =
  | "data_flow" | "argument" | "fallthrough" | "true_branch" | "false_branch"
  | "exception" | "return" | "uses" | "calls";

export type CfgEdge = {
  from: string;
  to: string;
  type: CfgEdgeType;
  label?: string;
};

export type CfgData = {
  nodes: CfgNode[];
  edges: CfgEdge[];
};

export type CfgEdgeData = {
  id: string;
  source: string;
  target: string;
  type: string;
  label?: string;
  highlighted?: boolean;
};

export type Node = {
  id: string;
  x: number;
  y: number;
  width: number;
  height: number;
  parent?: string;
  type?: string;
  nodeShape?: string;
  origin?: string;
  children?: Node[];
  collapsed: boolean;
  isChildrenLoading?: boolean;
  selected?: boolean;
  highlighted?: "upstream" | "downstream" | "path" | null;
  docstring?: string | null;
  code_block?: string | null;
  line_positions?: [number, number] | null;
  waitingForLayout: boolean;
  pdg?: CfgData | null;
  isScope?: boolean;
  label?: string;
  file_path?: string | null;
  changeStatus?: "added" | "modified" | "deleted" | null;
  isGhost?: boolean;
  [key: string]: any;
};

export type EdgeData = {
  source: string;
  target: string;
  type: "calls" | "is-a" | "has-a";
};

export type Edge = {
  source: string;
  target: string;
  type: "calls" | "is-a" | "has-a";
  highlighted?: boolean;
  changeStatus?: "added" | "deleted" | null;
};

export type Viewport = {
  x: number;
  y: number;
  zoom: number;
};

export type ZoomBounds = {
  minZoom: number;
  maxZoom: number;
};
