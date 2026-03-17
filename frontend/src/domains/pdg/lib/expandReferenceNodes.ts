import type { Node } from '../../graph';

export interface PdgNodeRaw {
  id: string;
  type: string;
  references?: string;
  line?: number;
  column?: number;
  text?: string;
  [key: string]: any;
}

export interface PdgEdgeRaw {
  from: string;
  to: string;
  type: string;
  label?: string;
  [key: string]: any;
}

export interface ExpandedPdg {
  nodes: PdgNodeRaw[];
  edges: PdgEdgeRaw[];
}

export function expandReferenceNodes(
  ownerNode: Node,
  allNodesMap: Map<string, Node>,
  visitedRefs: Set<string> = new Set()
): ExpandedPdg {
  if (!ownerNode.pdg || !ownerNode.pdg.nodes) return { nodes: [], edges: [] };

  const expandedNodes: PdgNodeRaw[] = [];
  const expandedEdges: PdgEdgeRaw[] = [];

  for (const node of ownerNode.pdg.nodes as any[]) {
    if ((node.type === 'call' || node.type === 'reference') && node.references) {
      const referencedQualname = node.references;
      if (visitedRefs.has(referencedQualname)) {
        console.warn(`[expandReferenceNodes] Circular reference detected: ${referencedQualname}`);
        expandedNodes.push(node);
        continue;
      }
      const referencedNode = allNodesMap.get(referencedQualname);
      if (!referencedNode || !referencedNode.pdg || !referencedNode.pdg.nodes) {
        expandedNodes.push(node);
        continue;
      }
      const newVisited = new Set(visitedRefs);
      newVisited.add(referencedQualname);
      const { nodes: refExpandedNodes, edges: refExpandedEdges } = expandReferenceNodes(referencedNode, allNodesMap, newVisited);
      const prefix = `${node.id}::`;
      const inlinedNodes = refExpandedNodes.map((refNode: any) => ({
        ...refNode,
        id: `${prefix}${refNode.id}`,
        _expandedPdgNodes: refNode._expandedPdgNodes?.map((child: any) => ({ ...child, id: `${prefix}${child.id}` })),
      }));
      const inlinedEdges = refExpandedEdges.map((refEdge) => ({
        ...refEdge, from: `${prefix}${refEdge.from}`, to: `${prefix}${refEdge.to}`,
      }));

      expandedNodes.push({ ...node, _expandedPdgNodes: inlinedNodes, _expandedPdgEdges: inlinedEdges });
      expandedEdges.push(...inlinedEdges);
    } else if (node.type === 'block' && node.pdg && node.pdg.nodes) {
      const blockAsOwner = { ...ownerNode, id: node.id, pdg: node.pdg };
      const { nodes: blockExpandedNodes, edges: blockExpandedEdges } = expandReferenceNodes(blockAsOwner as any, allNodesMap, visitedRefs);
      expandedNodes.push({ ...node, _expandedPdgNodes: blockExpandedNodes, _expandedPdgEdges: blockExpandedEdges });
    } else {
      expandedNodes.push(node);
    }
  }

  if (ownerNode.pdg.edges) {
    const expandedNodeIds = new Set<string>();
    const collectNodeIds = (nodes: any[]) => {
      for (const n of nodes) {
        expandedNodeIds.add(n.id);
        if (n._expandedPdgNodes) collectNodeIds(n._expandedPdgNodes);
      }
    };
    collectNodeIds(expandedNodes);

    for (const edge of ownerNode.pdg.edges) {
      let remappedEdge = { ...edge };
      const sourceInExpandedSet = expandedNodeIds.has(edge.from);
      if (sourceInExpandedSet) {
        let targetWasRemapped = false;
        for (const node of expandedNodes) {
          if (node.type === 'reference' && node.references && node._expandedPdgNodes) {
            for (const expandedNode of node._expandedPdgNodes) {
              const parts = expandedNode.id.split('::');
              if (parts.length >= 2) {
                const originalNodeId = parts[parts.length - 1];
                if (edge.to === originalNodeId) {
                  remappedEdge.to = expandedNode.id;
                  targetWasRemapped = true;
                }
              }
            }
          }
        }
        if (targetWasRemapped || expandedNodeIds.has(edge.to)) {
          expandedEdges.push(remappedEdge);
        }
      }
    }
  }

  return { nodes: expandedNodes, edges: expandedEdges };
}
