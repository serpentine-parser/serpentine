import { Node } from '../model/types';

export interface PositionedNode {
  id: string;
  x: number;
  y: number;
  width: number;
  height: number;
}

let _cachedNodes: Node[] | null = null;
let _nodeIndex: Map<string, Node> = new Map();

function buildIndex(nodes: Node[]): Map<string, Node> {
  const index = new Map<string, Node>();
  const visit = (list: Node[]) => {
    for (const n of list) {
      index.set(n.id, n);
      if (n.children) visit(n.children);
    }
  };
  visit(nodes);
  return index;
}

export function findNodeById(nodeId: string, nodes: Node[]): Node | null {
  if (nodes !== _cachedNodes) {
    _cachedNodes = nodes;
    _nodeIndex = buildIndex(nodes);
  }
  return _nodeIndex.get(nodeId) ?? null;
}

export function findParentOfChild(childId: string, nodes: Node[]): Node | null {
  const findParentRecursive = (nodeList: Node[], targetId: string): Node | null => {
    for (const node of nodeList) {
      if (node.children && node.children.some((c) => c.id === targetId)) return node;
      if (node.children && node.children.length > 0) {
        const found = findParentRecursive(node.children, targetId);
        if (found) return found;
      }
    }
    return null;
  };
  return findParentRecursive(nodes, childId);
}

export function expandParentNodes(nodeId: string, nodes: Node[]): Node[] {
  const findParentIds = (targetId: string, nodeList: Node[], parentIds: string[] = []): string[] => {
    for (const node of nodeList) {
      if (node.children) {
        const isDirectChild = node.children.some((child) => child.id === targetId);
        if (isDirectChild) {
          parentIds.push(node.id);
          return findParentIds(node.id, nodes, parentIds);
        }
        const foundInChildren = findParentIds(targetId, node.children, []);
        if (foundInChildren.length > 0) {
          parentIds.push(node.id);
          parentIds.push(...foundInChildren);
          return findParentIds(node.id, nodes, parentIds);
        }
      }
    }
    return parentIds;
  };

  const parentIds = findParentIds(nodeId, nodes);
  if (parentIds.length === 0) return nodes;

  const expandNodes = (nodeList: Node[]): Node[] => {
    return nodeList.map((node) => {
      let updatedNode = node;
      if (parentIds.includes(node.id)) updatedNode = { ...node, collapsed: false };
      if (node.children) updatedNode = { ...updatedNode, children: expandNodes(node.children) };
      return updatedNode;
    });
  };

  return expandNodes(nodes);
}

export function setAllCollapsed(nodes: Node[], collapsed: boolean): Node[] {
  return nodes.map((node) => {
    const updated: Node = collapsed
      ? { ...node, width: calculateMinWidth({ ...node, width: 180 }), height: 50, collapsed: true }
      : { ...node, collapsed: false };
    return { ...updated, children: node.children ? setAllCollapsed(node.children, collapsed) : undefined };
  });
}

export function setSubtreeCollapsed(nodes: Node[], targetId: string, collapsed: boolean): Node[] {
  return nodes.map((node) => {
    if (node.id === targetId) {
      // When expanding: also uncollapse the target so children become visible.
      // When collapsing: keep the target open (so collapsed children are visible inside it).
      const targetCollapsed = collapsed ? node.collapsed : false;
      return { ...node, collapsed: targetCollapsed, children: node.children ? setAllCollapsed(node.children, collapsed) : undefined };
    }
    if (node.children) return { ...node, children: setSubtreeCollapsed(node.children, targetId, collapsed) };
    return node;
  });
}

export function toggleNodeCollapse(nodeId: string, nodes: Node[]): Node[] {
  const updateNodeCollapse = (nodeList: Node[]): Node[] => {
    return nodeList.map((node) => {
      if (node.id === nodeId) {
        return {
          ...node,
          width: calculateMinWidth({ ...node, width: 180 }),
          height: 50,
          collapsed: !node.collapsed,
        };
      }
      if (node.children) return { ...node, children: updateNodeCollapse(node.children) };
      return node;
    });
  };
  return updateNodeCollapse(nodes);
}

const calculateNodeDimensionsBasedOnChildren = (
  nodes: PositionedNode[]
): { width: number | undefined; height: number | undefined } => {
  if (!nodes || nodes.length === 0) return { width: undefined, height: undefined };
  const padding = 10;
  const minX = Math.min(...nodes.map((child) => child.x));
  const minY = Math.min(...nodes.map((child) => child.y));
  const maxX = Math.max(...nodes.map((child) => child.x + child.width));
  const maxY = Math.max(...nodes.map((child) => child.y + child.height));
  return { width: maxX - minX + padding * 2, height: maxY - minY + padding * 2 };
};

const nodeTreeDepth = (node: Node, currentDepth = 0): number => {
  if (!node.children || node.children.length === 0 || node.collapsed) return currentDepth;
  return Math.max(...node.children.map((child) => nodeTreeDepth(child, currentDepth + 1)));
};

const MIN_NODE_WIDTH = 80;

const calculateMinWidth = (node: Node): number => {
  const name = node.id.split(".").pop() || node.id;
  const hasToggle = Array.isArray(node.children) && node.children.length > 0;
  const textWidth = name.length * 9 + 20 + (hasToggle ? 25 : 0);
  return Math.max(textWidth, MIN_NODE_WIDTH);
};

const findVisibleParent = (nodeId: string, nodes: Node[]): string | null => {
  const node = findNodeById(nodeId, nodes);
  if (!node) return null;
  if (!node.parent) return node.id;
  const parentNode = findNodeById(node.parent, nodes);
  if (parentNode && parentNode.collapsed) return findVisibleParent(parentNode.id, nodes);
  return node.id;
};

export function flattenCfgNodes(nodes: Node[], referencedNodeIds?: Set<string>): Node[] {
  const flattened: Node[] = [];

  const flatten = (node: Node) => {
    const hasCfg = node.pdg && node.pdg.nodes && node.pdg.nodes.length > 0;
    const hasSuffix = node.id.includes("::call_");
    const isInReferencedSet = referencedNodeIds && referencedNodeIds.has(node.id);
    const isModule = node.type === "module";

    const isReferencedDefinition =
      referencedNodeIds && hasCfg && !hasSuffix && isInReferencedSet && !isModule;

    if (isReferencedDefinition) {
      return;
    }

    flattened.push(node);

    if (!node.collapsed && node.children && node.children.length > 0) {
      node.children.forEach(flatten);
    }
  };

  nodes.forEach(flatten);
  return flattened;
}

export const nodeUtils = {
  findNodeById,
  findParentOfChild,
  calculateMinWidth,
  calculateNodeDimensionsBasedOnChildren,
  nodeTreeDepth,
  expandParentNodes,
  setAllCollapsed,
  setSubtreeCollapsed,
  toggleNodeCollapse,
  findVisibleParent,
};
