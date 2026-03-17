import { Edge, Node } from '../model/types';

interface RawNode {
  id: string;
  name: string;
  object_type: string;
  position: [number, number] | null;
  docstring: string | null;
  code_block: string | null;
  file_path: string | null;
  origin: string | null;
  children: RawNode[];
  pdg?: any | null;
  change_status?: string | null;
  isGhost?: boolean;
}

interface RawEdge {
  caller: string;
  callee: string;
  filename: string;
  type: string;
  change_status?: string | null;
}

interface RawData {
  nodes: RawNode[];
  edges: RawEdge[];
  cfg_edges?: any[];
}

export interface TransformedData {
  nodes: Node[];
  edges: Edge[];
  cfgEdges?: any[];
}

const getNodeDimensions = (type: string) => {
  switch (type) {
    case "module": return { width: 180, height: 50 };
    case "class": return { width: 120, height: 40 };
    default: return { width: 120, height: 40 };
  }
};

export function transformData(rawData: RawData): TransformedData {
  const transformNode = (rawNode: RawNode, parent?: string): Node => {
    const isCollapsed = rawNode.object_type === "module" && rawNode.children.length > 0;
    const { width, height } = getNodeDimensions(rawNode.object_type);

    return {
      id: rawNode.id,
      x: 0, y: 0, width, height,
      type: rawNode.object_type === "unknown"
        ? (rawNode.children.length > 0 ? "module" : "function")
        : rawNode.object_type,
      origin: rawNode.origin || "local",
      parent,
      collapsed: (rawNode.object_type === "module" ||
        (rawNode.object_type === "unknown" && rawNode.children.length > 0)) &&
        rawNode.children.length > 0,
      children: rawNode.children.map((child) => transformNode(child, rawNode.id)),
      docstring: rawNode.docstring,
      code_block: rawNode.code_block,
      file_path: rawNode.file_path ?? null,
      line_positions: rawNode.position,
      waitingForLayout: true,
      pdg: rawNode.pdg || null,
      changeStatus: (rawNode.change_status as "added" | "modified" | "deleted" | null) ?? null,
      isGhost: rawNode.isGhost ?? false,
    };
  };

  const transformEdge = (rawEdge: RawEdge): Edge => ({
    source: rawEdge.caller,
    target: rawEdge.callee,
    type: rawEdge.type as "calls" | "is-a" | "has-a",
    changeStatus: (rawEdge.change_status as "added" | "deleted" | null) ?? null,
  });

  return {
    nodes: rawData.nodes.map((rawNode) => transformNode(rawNode)),
    edges: rawData.edges.map(transformEdge),
    cfgEdges: rawData.cfg_edges || [],
  };
}

export function transformFlowGraph(flowGraph: any): Node[] {
  if (!flowGraph || !flowGraph.root) return [];

  const transformFlowNode = (flowNode: any, parent?: string, depth: number = 0): Node => ({
    id: flowNode.id,
    label: flowNode.label,
    x: 0, y: 0,
    width: flowNode.width || 120,
    height: flowNode.height || 30,
    nodeShape: flowNode.shape,
    parent,
    collapsed: false,
    isScope: flowNode.shape === "scope" || (flowNode.children?.length ?? 0) > 0,
    children: flowNode.children
      ? flowNode.children.map((child: any) => transformFlowNode(child, flowNode.id, depth + 1))
      : [],
    waitingForLayout: true,
    highlighted: null,
  });

  return [transformFlowNode(flowGraph.root)];
}

