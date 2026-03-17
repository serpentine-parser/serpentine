import { Node } from '@domains/graph/model/types';
import {
  EDGE_COLORS as THEME_EDGE_COLORS,
  EDGE_DIMMED as THEME_EDGE_DIMMED,
  EDGE_HIGHLIGHTED as THEME_EDGE_HIGHLIGHTED,
} from "./graphTheme";

export type NodeState = "default" | "selected" | "highlighted" | "dimmed";
export type NodeType = "module" | "class" | "function";

export interface NodeStyleClasses {
  container: string;
  header: string;
  text: string;
  isNested?: boolean;
}

export function getNodeState(
  node: Node,
  selectedNodeId: string | null,
): NodeState {
  const isSelected = selectedNodeId === node.id;
  const isHighlighted = node.highlighted;
  const hasSelection = selectedNodeId !== null;

  if (isSelected) {
    return "selected";
  } else if (isHighlighted) {
    return "highlighted";
  } else if (hasSelection) {
    return "dimmed";
  } else {
    return "default";
  }
}

export function getNodeStyleClasses(
  nodeType: NodeType,
  state: NodeState,
  isNested: boolean = false,
  isEditMode: boolean = false,
): NodeStyleClasses {
  const prefix = isEditMode ? "edit-" : "";
  const containerClass = `${prefix}${nodeType}-${state}${
    isNested && nodeType === "function"
      ? state === "dimmed"
        ? ` ${prefix}function-nested-dimmed`
        : ` ${prefix}function-nested`
      : ""
  }`;
  const headerClass = `${prefix}${nodeType}-header-${state}`;

  let textClass: string;
  if (state === "selected") {
    textClass = "node-text-dark";
  } else if (nodeType === "function") {
    textClass = "node-text-dark";
  } else if (nodeType === "class") {
    textClass = "node-text-dark";
  } else {
    textClass = "node-text-dark";
  }

  return {
    container: containerClass,
    header: headerClass,
    text: textClass,
    isNested,
  };
}

export type EdgeState = "default" | "highlighted" | "dimmed";
export type EdgeType = "calls" | "is-a" | "has-a";

export interface EdgeStyleClasses {
  path: string;
  markerStart: string;
  markerEnd: string;
}

export function getEdgeState(
  sourceHighlighted: boolean,
  targetHighlighted: boolean,
  hasSelection: boolean,
): EdgeState {
  if (sourceHighlighted && targetHighlighted) {
    return "highlighted";
  } else if (hasSelection) {
    return "dimmed";
  } else {
    return "default";
  }
}

export function getEdgeStyleClasses(
  state: EdgeState,
  edgeType: EdgeType = "calls",
  isEditMode: boolean = false,
): EdgeStyleClasses {
  const prefix = isEditMode ? "edit-" : "";
  const markerPrefix = isEditMode ? "edit-" : "";
  return {
    path: `${prefix}edge-${state} edge-${edgeType}`,
    markerStart: `url(#start-circle-${markerPrefix}${state})`,
    markerEnd: `url(#${markerPrefix}arrow-${state})`,
  };
}

export const DEP_EDGE_COLORS = THEME_EDGE_COLORS;
export const DEP_EDGE_DIMMED = THEME_EDGE_DIMMED;
export const DEP_EDGE_HIGHLIGHTED = THEME_EDGE_HIGHLIGHTED;

export function getChangeStatusColor(changeStatus: "added" | "modified" | "deleted" | null | undefined): string | null {
  switch (changeStatus) {
    case "added": return "#0ea5e9";
    case "modified": return "#f59e0b";
    case "deleted": return "#ef4444";
    default: return null;
  }
}
