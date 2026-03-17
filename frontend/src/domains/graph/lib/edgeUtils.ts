import { Edge, Node } from '../model/types';
import { nodeUtils } from './nodeUtils';

export function getSimpleEdges(allEdges: Edge[], nodes: Node[], maxDepth: number = Infinity): Edge[] {
  const edgePriority: Record<Edge["type"], number> = { calls: 3, "is-a": 2, "has-a": 1 };
  const allNodeIds = new Set<string>();
  const visibleNodeIds = new Set<string>();

  const collectNodeIds = (nodeList: Node[]) => {
    for (const node of nodeList) {
      allNodeIds.add(node.id);
      if (node.children) collectNodeIds(node.children);
    }
  };
  collectNodeIds(nodes);

  const collectVisibleNodeIds = (nodeList: Node[]) => {
    for (const node of nodeList) {
      visibleNodeIds.add(node.id);
      if (node.children && node.children.length > 0 && !node.collapsed)
        collectVisibleNodeIds(node.children);
    }
  };
  collectVisibleNodeIds(nodes);

  const bestEdgeByPair = new Map<string, Edge>();

  const findVisibleParentStrict = (nodeId: string): string | null => {
    if (visibleNodeIds.has(nodeId)) return nodeId;
    const node = nodeUtils.findNodeById(nodeId, nodes);
    const parentId = node?.parent ?? (nodeId.includes('.') ? nodeId.split('.').slice(0, -1).join('.') : null);
    if (!parentId) return null;
    return findVisibleParentStrict(parentId);
  };

  // Walks up to find the deepest ancestor whose dot-path has at most maxSegments segments.
  // Uses real node.parent links when available; falls back to string-based parent inference
  // when the node isn't in the visible tree (e.g. children of collapsed nodes are cleared).
  const findAncestorAtDepth = (nodeId: string, maxSegments: number): string => {
    if (nodeId.split('.').length <= maxSegments) return nodeId;
    const node = nodeUtils.findNodeById(nodeId, nodes);
    const parentId = node?.parent ?? (nodeId.includes('.') ? nodeId.split('.').slice(0, -1).join('.') : null);
    if (!parentId) return nodeId;
    return findAncestorAtDepth(parentId, maxSegments);
  };

  for (const edge of allEdges) {
    const parts1 = edge.source.split('.');
    const parts2 = edge.target.split('.');
    const lcaParts: string[] = [];
    for (let i = 0; i < Math.min(parts1.length, parts2.length); i++) {
      if (parts1[i] === parts2[i]) lcaParts.push(parts1[i]);
      else break;
    }
    const depthLimit = lcaParts.length + maxDepth;

    const source = findAncestorAtDepth(edge.source, depthLimit);
    const target = findAncestorAtDepth(edge.target, depthLimit);

    const visibleSource = findVisibleParentStrict(source);
    const visibleTarget = findVisibleParentStrict(target);

    if (!visibleSource && allNodeIds.has(source))
      console.warn(`Edge source ${source} exists but findVisibleParent returned null`);
    if (!visibleTarget && allNodeIds.has(target))
      console.warn(`Edge target ${target} exists but findVisibleParent returned null`);

    if (!visibleSource || !visibleTarget) continue;
    if (visibleSource === visibleTarget) continue;

    const pairKey = `${visibleSource}::${visibleTarget}`;
    const candidate: Edge = { source: visibleSource, target: visibleTarget, type: edge.type, changeStatus: edge.changeStatus };
    const existing = bestEdgeByPair.get(pairKey);
    if (!existing) {
      bestEdgeByPair.set(pairKey, candidate);
    } else if (edgePriority[candidate.type] > edgePriority[existing.type]) {
      bestEdgeByPair.set(pairKey, candidate);
    }
  }

  const bestEdges = Array.from(bestEdgeByPair.values());
  return bestEdges;
}

function getVisibleEdges(allEdges: Edge[], nodes: Node[], maxDepth: number = 1): Edge[] {
  return getSimpleEdges(allEdges, nodes, maxDepth);
}

export const edgeUtils = { getSimpleEdges, getVisibleEdges };
