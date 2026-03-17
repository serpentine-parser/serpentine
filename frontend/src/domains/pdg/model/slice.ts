import { StateCreator } from 'zustand';
import { CollisionDetector, flattenCfgNodes, LayoutEngine, NodeMovement, nodeUtils } from '../../graph';
import type { Node } from '../../graph';
import { expandReferenceNodes } from '../lib/expandReferenceNodes';
import { CfgEdgeData } from './types';

const collisionDetector = new CollisionDetector();
const nodeMovement = new NodeMovement(collisionDetector);

export type PdgSlice = {
  pdgNodes: Node[];
  pdgFlatNodes: Node[];
  pdgEdges: CfgEdgeData[];
  pdgAllEdges: CfgEdgeData[];
  isPdgLayoutLoading: boolean;
  pdgVisibleEdgeTypes: Set<string>;
  pdgReferencedNodeIds: Set<string>;

  initializePdg: () => Promise<void>;
  applyPdgLayout: (options?: { silent?: boolean }) => void;
  togglePdgNodeCollapse: (nodeId: string) => void;
  movePdgNodeWithConstraints: (id: string, parent: string | null, x: number, y: number) => boolean;
  findPdgNodeById: (nodeId: string) => Node | null;
  expandPdgParentNodes: (nodeId: string) => void;
  selectPdgNode: (nodeId: string) => void;
  setPdgVisibleEdgeTypes: (types: Set<string>) => void;
};

export const createPdgSlice: StateCreator<any, [], [], PdgSlice> = (set, get) => ({
  pdgNodes: [] as Node[],
  pdgFlatNodes: [] as Node[],
  pdgEdges: [] as CfgEdgeData[],
  pdgAllEdges: [] as CfgEdgeData[],
  isPdgLayoutLoading: false,
  pdgVisibleEdgeTypes: new Set<string>(),
  pdgReferencedNodeIds: new Set<string>(),

  initializePdg: async () => {
    const state = get();

    const getNodeShape = (nodeType: string): string => {
      const typeToShapeMap: Record<string, string> = {
        "condition": "condition", "call": "statement", "assignment": "statement",
        "return": "statement", "break": "statement", "continue": "statement",
        "raise": "statement", "name": "statement", "attribute_access": "statement",
        "parameter": "statement", "assignment_target": "statement", "literal": "literal",
        "merge": "merge", "name_reference": "statement", "parameter_reference": "statement",
        "identifier": "statement",
      };
      return typeToShapeMap[nodeType] || "statement";
    };

    const getLabel = (node: Node): string => {
      if (node.label) return node.label;
      const parts = node.id.split(".");
      if (parts.length >= 2) {
        const last = parts[parts.length - 1];
        const secondLast = parts.length >= 3 ? parts[parts.length - 2] : null;
        if (secondLast && /^[A-Z]/.test(secondLast)) return `${secondLast}.${last}`;
        return last;
      }
      return node.id;
    };

    const hasCalls = (node: Node): boolean => {
      if (!node.pdg || !node.pdg.nodes || node.pdg.nodes.length === 0) return false;
      const hasStatements = node.pdg.nodes.some((pdgNode: any) => pdgNode.type !== "parameter");
      return hasStatements;
    };

    const buildNodesMap = (nodes: Node[]): Map<string, Node> => {
      const map = new Map<string, Node>();
      const addNode = (n: Node) => { map.set(n.id, n); (n.children || []).forEach(addNode); };
      nodes.forEach(addNode);
      return map;
    };
    const allNodesMap = buildNodesMap(state.nodes);

    const transformPdgNodeRecursive = (pdgNode: any, parentId: string): Node => {
      if ((pdgNode.type === 'call' || pdgNode.type === 'reference') && pdgNode._expandedPdgNodes) {
        const scopeChildren = (pdgNode._expandedPdgNodes || []).map((innerNode: any) =>
          transformPdgNodeRecursive(innerNode, pdgNode.id)
        );
        return {
          id: pdgNode.id, label: pdgNode.text || pdgNode.references,
          x: 0, y: 0, width: 180, height: 50,
          nodeShape: 'scope', parent: parentId, collapsed: false, isScope: true,
          children: scopeChildren, waitingForLayout: true, highlighted: null,
          _isReference: true, _referencesNode: pdgNode.references,
        };
      }
      if (pdgNode.type === 'block' && pdgNode._expandedPdgNodes) {
        const blockChildren = (pdgNode._expandedPdgNodes || []).map((innerNode: any) =>
          transformPdgNodeRecursive(innerNode, pdgNode.id)
        );
        return {
          id: pdgNode.id, label: pdgNode.text || 'block',
          x: 0, y: 0, width: 180, height: 50,
          nodeShape: 'scope', parent: parentId, collapsed: false, isScope: true,
          children: blockChildren, waitingForLayout: true, highlighted: null,
        };
      }
      return {
        id: pdgNode.id, label: pdgNode.text,
        x: 0, y: 0, width: Math.max(100, (pdgNode.text?.length || 10) * 8), height: 30,
        nodeShape: getNodeShape(pdgNode.type), parent: parentId,
        collapsed: false, children: [], waitingForLayout: true, highlighted: null,
        _isReference: pdgNode._isExpanded || false, _referencesNode: pdgNode.references,
      };
    };

    const transformToPdgTree = (node: Node): Node | null => {
      if (node.type !== 'module' && !hasCalls(node)) return null;
      const transformedChildren: Node[] = [];
      if (node.children) {
        for (const child of node.children) {
          if (child.type === 'class' || child.type === 'function') continue;
          const transformed = transformToPdgTree(child);
          if (transformed) transformedChildren.push(transformed);
        }
      }
      if (hasCalls(node)) {
        const { nodes: expandedPdgNodes, edges: expandedPdgEdges } = expandReferenceNodes(node, allNodesMap);
        const pdgLeafNodes: Node[] = expandedPdgNodes.map((pdgNode: any) =>
          transformPdgNodeRecursive(pdgNode, node.id)
        );
        const mappedPdgNodes = expandedPdgNodes.map((n: any) => ({
          id: n.id, block: n.id, line: n.line || 0, column: n.column || 0,
          type: n.type, text: n.text || "",
        }));
        const mappedPdgEdges = expandedPdgEdges.map((e: any) => ({
          from: e.from, to: e.to, type: e.type as any, label: e.label,
        }));
        const updatedPdg = { ...node.pdg, nodes: mappedPdgNodes, edges: mappedPdgEdges };
        transformedChildren.push(...pdgLeafNodes);
        return { ...node, label: getLabel(node), isScope: true, collapsed: false, children: transformedChildren, pdg: updatedPdg };
      }
      if (transformedChildren.length > 0)
        return { ...node, label: getLabel(node), isScope: true, collapsed: false, children: transformedChildren };
      return null;
    };

    const extractPdgEdges = (scopes: Node[]): CfgEdgeData[] => {
      const edges: CfgEdgeData[] = [];
      let edgeIdCounter = 0;
      const validNodeIds = new Set<string>();
      const collectIds = (node: Node) => {
        validNodeIds.add(node.id);
        (node.children || []).forEach(collectIds);
      };
      scopes.forEach(collectIds);

      const walkNode = (node: Node) => {
        if (node.pdg && node.pdg.edges) {
          for (const edge of node.pdg.edges) {
            const isUsesEdge = edge.type === "uses";
            if (!validNodeIds.has(edge.from) && !isUsesEdge) continue;
            const isExternalEdge = edge.type === "calls" || edge.type === "uses";
            if (!isExternalEdge && !validNodeIds.has(edge.to)) continue;
            let label = edge.label;
            if (!label) {
              if (edge.type === "true_branch") label = "true";
              else if (edge.type === "false_branch") label = "false";
            }
            edges.push({ id: `pdg-edge-${edgeIdCounter++}`, source: edge.from, target: edge.to, type: edge.type, label, highlighted: false });
          }
        }
        (node.children || []).forEach(walkNode);
      };
      scopes.forEach(walkNode);
      return edges;
    };

    const pdgNodeTree: Node[] = [];
    const entryNodeId = state.selectedNodeId;
    if (!entryNodeId) {
      set({ pdgNodes: [], pdgFlatNodes: [], pdgEdges: [], pdgAllEdges: [], isPdgLayoutLoading: false });
      return;
    }
    const entryNode = allNodesMap.get(entryNodeId);
    if (!entryNode) {
      set({ pdgNodes: [], pdgFlatNodes: [], pdgEdges: [], pdgAllEdges: [], isPdgLayoutLoading: false });
      return;
    }
    const transformed = transformToPdgTree(entryNode);
    if (transformed) pdgNodeTree.push(transformed);

    const allPdgEdges = extractPdgEdges(pdgNodeTree);

    if (allPdgEdges.length === 0) {
      set({
        pdgNodes: pdgNodeTree.length > 0 ? pdgNodeTree : [],
        pdgFlatNodes: pdgNodeTree.length > 0 ? flattenCfgNodes(pdgNodeTree) : [],
        pdgEdges: [], pdgAllEdges: [], isPdgLayoutLoading: false,
      });
      return;
    }

    let visibleTypes = state.pdgVisibleEdgeTypes;
    if (visibleTypes.size === 0) visibleTypes = new Set(allPdgEdges.map((e: any) => e.type));
    const filteredEdges = allPdgEdges.filter((e: any) => visibleTypes.has(e.type));

    set({ pdgAllEdges: allPdgEdges, pdgEdges: filteredEdges, pdgVisibleEdgeTypes: visibleTypes, isPdgLayoutLoading: true });

    const layoutEngine = new LayoutEngine(pdgNodeTree, state.layoutSettings);
    const validNodeIds = new Set<string>();
    const collectAllIds = (node: Node) => {
      validNodeIds.add(node.id);
      (node.children || []).forEach(collectAllIds);
    };
    pdgNodeTree.forEach(collectAllIds);

    const elkEdges = filteredEdges
      .filter((e: any) => validNodeIds.has(e.source) && validNodeIds.has(e.target))
      .map((e: any) => ({ source: e.source, target: e.target, type: "calls" as const }));
    const elkGraph = await layoutEngine.applyHierarchicalDependencyLayout(pdgNodeTree, elkEdges, undefined, true);
    const laid = layoutEngine.convertFromELKResults(elkGraph.children);

    set({ pdgNodes: laid, pdgFlatNodes: flattenCfgNodes(laid), isPdgLayoutLoading: false });
  },

  applyPdgLayout: ({ silent = false } = {}) => {
    const state = get();
    const originalNodes = state.pdgNodes;
    const layoutEngine = new LayoutEngine(originalNodes, state.layoutSettings);
    if (!silent) set({ isPdgLayoutLoading: true });

    function mapNode(n: Node): Node {
      return { ...n, children: n.collapsed ? [] : n.children ? n.children.map(mapNode) : [] };
    }
    const mappedNodes = originalNodes.map(mapNode);

    const visibleIds = new Set<string>();
    const collectVisibleIds = (nodes: Node[]) => {
      for (const n of nodes) {
        visibleIds.add(n.id);
        if (n.children) collectVisibleIds(n.children);
      }
    };
    collectVisibleIds(mappedNodes);

    const elkEdges = state.pdgEdges
      .filter((e: any) => visibleIds.has(e.source) && visibleIds.has(e.target))
      .map((e: any) => ({ source: e.source, target: e.target, type: "calls" as const }));

    layoutEngine.applyHierarchicalDependencyLayout(mappedNodes, elkEdges, undefined, true).then((newNodes) => {
      const positionedNodes = layoutEngine.convertFromELKResults(newNodes.children);

      const posMap = new Map<string, { x: number; y: number; width: number; height: number }>();
      const collectPositions = (nodes: Node[]) => {
        for (const n of nodes) {
          posMap.set(n.id, { x: n.x, y: n.y, width: n.width, height: n.height });
          if (n.children) collectPositions(n.children);
        }
      };
      collectPositions(positionedNodes);

      const mergePositions = (nodes: Node[]): Node[] =>
        nodes.map(n => {
          const pos = posMap.get(n.id);
          return { ...n, ...(pos ?? {}), children: n.children ? mergePositions(n.children) : undefined };
        });

      const merged = mergePositions(originalNodes);
      set({ pdgNodes: merged, pdgFlatNodes: flattenCfgNodes(merged), ...(silent ? {} : { isPdgLayoutLoading: false }) });
    });
  },

  togglePdgNodeCollapse: (nodeId: string) => {
    const state = get();
    const targetNode = nodeUtils.findNodeById(nodeId, state.pdgNodes);
    if (!targetNode) return;
    const nodes = nodeUtils.toggleNodeCollapse(nodeId, state.pdgNodes);
    set({ pdgNodes: [...nodes] });
    get().applyPdgLayout();
  },

  movePdgNodeWithConstraints: (id: string, parent: string | null, x: number, y: number) => {
    const state = get();
    const result = nodeMovement.moveChildWithConstraints(id, parent, x, y, state.pdgNodes, state.padding);
    if (result.success) {
      set({ pdgNodes: result.updatedNodes, pdgFlatNodes: flattenCfgNodes(result.updatedNodes) });
      const updateAllAncestorBounds = (currentParentId: string | undefined | null) => {
        if (!currentParentId) return;
        const state = get();
        const updatedNodes = nodeMovement.updateParentBounds(currentParentId, state.pdgNodes, state.padding);
        set({ pdgNodes: updatedNodes, pdgFlatNodes: flattenCfgNodes(updatedNodes) });
        const grandParent = nodeUtils.findParentOfChild(currentParentId, state.pdgNodes);
        if (grandParent) updateAllAncestorBounds(grandParent.id);
      };
      updateAllAncestorBounds(parent);
      return true;
    }
    return false;
  },

  findPdgNodeById: (nodeId: string) => nodeUtils.findNodeById(nodeId, get().pdgNodes),

  expandPdgParentNodes: (nodeId: string) => {
    const state = get();
    const updatedNodes = nodeUtils.expandParentNodes(nodeId, state.pdgNodes);
    set({ pdgNodes: updatedNodes });
    get().applyPdgLayout();
  },

  selectPdgNode: (nodeId: string) => {
    const state = get();
    const allEdges = state.pdgAllEdges;
    const upstream = new Set<string>();
    const downstream = new Set<string>();
    const downQ: string[] = [nodeId];
    while (downQ.length > 0) {
      const cur = downQ.shift()!;
      if (downstream.has(cur)) continue;
      downstream.add(cur);
      for (const e of allEdges) if (e.source === cur && !downstream.has(e.target)) downQ.push(e.target);
    }
    const upQ: string[] = [nodeId];
    while (upQ.length > 0) {
      const cur = upQ.shift()!;
      if (upstream.has(cur)) continue;
      upstream.add(cur);
      for (const e of allEdges) if (e.target === cur && !upstream.has(e.source)) upQ.push(e.source);
    }
    const connected = new Set(Array.from(upstream).concat(Array.from(downstream)));
    const highlightPdgNodes = (nodes: Node[]): Node[] =>
      nodes.map((n) => ({
        ...n, highlighted: connected.has(n.id) ? "path" as const : null,
        children: n.children ? highlightPdgNodes(n.children) : undefined,
      }));
    const highlightedEdges = state.pdgEdges.map((e: any) => ({
      ...e, highlighted: connected.has(e.source) && connected.has(e.target),
    }));
    const highlightedNodes = highlightPdgNodes(state.pdgNodes);
    set({
      pdgNodes: highlightedNodes,
      pdgFlatNodes: flattenCfgNodes(highlightedNodes), pdgEdges: highlightedEdges,
    });
  },

  setPdgVisibleEdgeTypes: (types: Set<string>) => {
    const state = get();
    const filteredEdges = state.pdgAllEdges.filter((e: any) => types.has(e.type));
    set({ pdgVisibleEdgeTypes: types, pdgEdges: filteredEdges });
  },
});
