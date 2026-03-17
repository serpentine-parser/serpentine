// Pure ELK graph conversion — no ELK runtime dependency.
// Safe to import in both main-thread and Web Worker contexts.
import type { ElkExtendedEdge, ElkNode } from "elkjs";
import { DEFAULT_LAYOUT_SETTINGS, LayoutSettings } from '../model/layoutTypes';
import { Edge, Node } from '../model/types';
import { nodeUtils } from './nodeUtils';

// Height of the visible header bar rendered at the top of each container node.
const HEADER_HEIGHT = 25;

export interface PositionHints {
  /** Absolute positions for nodes (used as ELK ordering/placement seeds). */
  positions: Record<string, { x: number; y: number }>;
  /** Subset of positions that ELK should treat as fixed constraints. */
  pinned: Set<string>;
}

export interface ElkGraph {
  id: string;
  layoutOptions: Record<string, string>;
  children: ElkNode[];
  edges: ElkExtendedEdge[];
  width: number;
  height: number;
}

export function convertToELKGraph(
  nodes: Node[],
  edges: Edge[],
  hints: PositionHints | undefined,
  settings: LayoutSettings = DEFAULT_LAYOUT_SETTINGS,
): ElkGraph {
  const s = settings;
  const rootDirection = s.rootDirection;

  // Children are positioned relative to their parent in ELK, so hints only
  // apply to top-level nodes (applyHints = true) to avoid coordinate mismatch.
  const convertNode = (node: Node, applyHints: boolean = false): any => {
    const isCompound = Array.isArray(node.children) && node.children.length > 0;
    const elkNode: any = {
      id: node.id,
      width: nodeUtils.calculateMinWidth(node),
      layoutOptions: {
        "elk.algorithm": "layered",
        "elk.direction": "DOWN",
        "elk.layered.spacing.nodeNodeBetweenSiblings": String(s.childNodeNode),
        "elk.layered.spacing.nodeNodeBetweenLayers": String(s.childNodeBetweenLayers),
        "elk.hierarchyHandling": "INCLUDE_CHILDREN",
        "elk.spacing.nodeNode": String(s.childNodeNode),
        "elk.spacing.edgeNode": String(s.edgeNode),
        "elk.spacing.componentComponent": String(s.componentComponent),
        "elk.padding": isCompound
          ? `[top=${s.padding + HEADER_HEIGHT}, left=${s.padding}, bottom=${s.padding}, right=${s.padding}]`
          : String(s.padding),
      },
      children: node.children ? node.children.map((c) => convertNode(c, false)) : [],
    };
    if (!isCompound) elkNode.height = node.height;

    if (applyHints && hints?.positions[node.id]) {
      const pos = hints.positions[node.id];
      elkNode.x = pos.x;
      elkNode.y = pos.y;
      if (hints.pinned.has(node.id)) {
        elkNode.layoutOptions["org.eclipse.elk.position"] = `(${pos.x}, ${pos.y})`;
      }
    }

    return elkNode;
  };

  const { height, width } = nodeUtils.calculateNodeDimensionsBasedOnChildren(nodes);

  const rootLayoutOptions: Record<string, string> = {
    "elk.algorithm": "layered",
    "elk.direction": rootDirection,
    "elk.hierarchyHandling": "INCLUDE_CHILDREN",
    "elk.layered.spacing.nodeNodeBetweenLayers": String(s.rootNodeBetweenLayers),
    "elk.spacing.nodeNode": String(s.rootNodeNode),
    "elk.spacing.edgeNode": String(s.edgeNode),
    "elk.spacing.componentComponent": String(s.componentComponent),
    "elk.padding": String(s.padding),
  };

  if (hints) rootLayoutOptions["elk.interactiveLayout"] = "true";

  return {
    id: "root",
    layoutOptions: rootLayoutOptions,
    children: nodes.map((n) => convertNode(n, true)),
    edges: edges.map((e) => ({ id: `${e.source}-${e.target}`, sources: [e.source], targets: [e.target] })),
    width: width ?? 100,
    height: height ?? 100,
  };
}
