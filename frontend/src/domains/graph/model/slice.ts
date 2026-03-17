import React from 'react';
import { StateCreator } from 'zustand';
import { CollisionDetector } from '../lib/collision';
import { edgeUtils } from '../lib/edgeUtils';
import { PositionHints, WorkerLayoutEngine } from '../lib/layout';
import { layoutCache } from '../lib/layoutPersistence';
import { NodeMovement } from '../lib/movement';
import { flattenCfgNodes, nodeUtils } from '../lib/nodeUtils';
import { SelectionAndHighlighting } from '../lib/selectionAndHighlighting';
import { LayoutSettings } from './layoutTypes';
import { Edge, Node, Viewport, ZoomBounds } from './types';

const collisionDetector = new CollisionDetector();
const nodeMovement = new NodeMovement(collisionDetector);
const selectionAndHighlighting = new SelectionAndHighlighting(nodeUtils);

export type GraphSlice = {
  nodes: Node[];
  visibleEdges: Edge[];
  allNodes: Node[];
  catalogNodes: Node[];
  allEdges: Edge[];
  apiNodes: Node[];
  apiEdges: Edge[];
  sessionPositions: Record<string, { x: number; y: number }>;
  layoutTransition: boolean;
  lastSelectorKey: string;
  graphWidth: number;
  graphHeight: number;
  graphBounds: { minX: number; minY: number; maxX: number; maxY: number } | null;
  padding: number;
  selectedNodeId: string | null;
  hoveredNodeId: string | null;
  renderMode: "svg";
  viewport: Viewport;
  zoomBounds: ZoomBounds;
  isLayoutLoading: boolean;
  loadingPhase: "layout" | null;
  loadingNodeCount: number;
  svgRef: React.RefObject<SVGSVGElement> | null;
  layoutSettings: LayoutSettings;

  initialize: (n: Node[], e?: Edge[]) => void;
  setVisibleEdges: () => void;
  updateParentBounds: (parent: string) => void;
  moveChildWithConstraints: (id: string, parent: string | null, x: number, y: number) => boolean;
  toggleNodeCollapse: (nodeId: string) => void;
  expandParentNodes: (nodeId: string) => void;
  findNodeById: (nodeId: string, nodes?: Node[]) => Node | null;
  applyHierarchicalDependencyLayout: (options?: { clearPositions?: boolean; silent?: boolean; pendingNodes?: Node[] }) => void;
  setLayoutLoading: (loading: boolean) => void;
  setGraphDimensions: (width: number, height: number) => void;
  selectNode: (nodeId: string | null) => void;
  setHoveredNode: (nodeId: string | null) => void;
  highlightDependencies: (nodeId: string, direction: "upstream" | "downstream" | "both") => void;
  highlightDependenciesCustom: (selectedNodeId: string, dependencies: Set<string>, dependents: Set<string>) => void;
  clearHighlights: () => void;
  setRenderMode: (mode: "svg") => void;
  setViewport: (viewport: Viewport) => void;
  setZoomBounds: (bounds: ZoomBounds) => void;
  setSvgRef: (ref: React.RefObject<SVGSVGElement> | null) => void;
  exportPng: () => Promise<void>;
  setLayoutSettings: (patch: Partial<LayoutSettings>) => void;
  flipLayoutDirection: () => void;
  wsSend: ((msg: object) => void) | null;
  setWsSend: (fn: (msg: object) => void) => void;
  dismissChange: (nodeId: string) => void;
  dismissAllChanges: () => void;
  sidebarExpansionSignal: { type: "expand" | "collapse"; nodeId: string | null } | null;
  setSidebarExpansionSignal: (signal: { type: "expand" | "collapse"; nodeId: string | null } | null) => void;
  expandAll: () => void;
  collapseAll: () => void;
  expandChildren: (nodeId: string) => void;
  collapseChildren: (nodeId: string) => void;
  resetLayout: () => void;
};

export const createGraphSlice: StateCreator<any, [], [], GraphSlice> = (set, get) => ({
  nodes: [],
  visibleEdges: [],
  allNodes: [],
  catalogNodes: [],
  allEdges: [],
  apiNodes: [],
  apiEdges: [],
  sessionPositions: {},
  layoutTransition: false,
  lastSelectorKey: "",
  graphWidth: 1200,
  graphHeight: 800,
  graphBounds: null,
  padding: 10,
  selectedNodeId: null,
  hoveredNodeId: null,
  renderMode: "svg" as "svg",
  viewport: { x: 0, y: 0, zoom: 1 },
  zoomBounds: { minZoom: 0.1, maxZoom: 10 },
  isLayoutLoading: false,
  loadingPhase: null as "layout" | null,
  loadingNodeCount: 0,
  svgRef: null,
  wsSend: null,
  layoutSettings: layoutCache.getLayoutSettings(),
  sidebarExpansionSignal: null,

  initialize: (nodes, edges = []) => {

    // Count all nodes for loading feedback
    const countNodes = (nodeList: Node[]): number =>
      nodeList.reduce((sum, n) => sum + 1 + (n.children ? countNodes(n.children) : 0), 0);
    const nodeCount = countNodes(nodes);
    set({ loadingPhase: "layout", loadingNodeCount: nodeCount });

    // Single pass: collect IDs, apply collapsed defaults, auto-expand single-child chains,
    // and overlay persisted state — replaces four separate recursive passes.
    const liveIds = new Set<string>();
    const prepareNodes = (nodeList: Node[], hasFoundMultiChild: boolean = false): Node[] =>
      nodeList.map((node) => {
        liveIds.add(node.id);
        const thisNodeHasMultipleChildren = node.children && node.children.length > 1;
        const childHasFoundMultiChild = hasFoundMultiChild || !!thisNodeHasMultipleChildren;
        const processedChildren = node.children
          ? prepareNodes(node.children, childHasFoundMultiChild)
          : undefined;
        const shouldAutoExpand =
          node.origin === "local" && node.type === "module" &&
          processedChildren && processedChildren.length > 0 && !hasFoundMultiChild;
        const persisted = layoutCache.getNodeState(node.id);
        const collapsed = persisted?.collapsed !== undefined ? persisted.collapsed : !shouldAutoExpand;
        return { ...node, collapsed, children: processedChildren };
      });

    const preparedNodes = prepareNodes(nodes);
    layoutCache.pruneStaleNodes(liveIds);

    // Detect selector change to decide whether to clear positions
    const state = get();
    const selectorKey = `${state.selectorQuery}|${state.selectorExclude}`;
    const isNewQuery = selectorKey !== state.lastSelectorKey;

    set({
      nodes: preparedNodes,
      allNodes: preparedNodes,
      allEdges: edges,
      apiNodes: nodes,
      apiEdges: edges,
      lastSelectorKey: selectorKey,
      isLayoutLoading: true,
    });
    get().filterNodes(true); // suppress layout — we call it below with the right options
    get().loadCatalog();
    get().applyHierarchicalDependencyLayout({ clearPositions: isNewQuery });
  },

  setVisibleEdges: () => {
    const state = get();
    const edges = edgeUtils.getVisibleEdges(state.allEdges, state.nodes, state.visibleEdgeDepth);
    set({ visibleEdges: edges });
  },

  updateParentBounds: (parentId) => {
    const state = get();
    const updatedNodes = nodeMovement.updateParentBounds(parentId, state.nodes, state.padding);
    set({ nodes: updatedNodes });
  },

  moveChildWithConstraints: (id, parent, x, y) => {
    const state = get();
    const result = nodeMovement.moveChildWithConstraints(id, parent, x, y, state.nodes, state.padding);
    if (result.success) {
      set({ nodes: result.updatedNodes });
      const updateAllAncestorBounds = (currentParentId: string | undefined | null) => {
        if (!currentParentId) return;
        const state = get();
        const updatedNodes = nodeMovement.updateParentBounds(currentParentId, state.nodes, state.padding);
        set({ nodes: updatedNodes });
        const grandParent = nodeUtils.findParentOfChild(currentParentId, state.nodes);
        if (grandParent) updateAllAncestorBounds(grandParent.id);
      };
      updateAllAncestorBounds(parent);
      return true;
    }
    return false;
  },

  toggleNodeCollapse: (nodeId: string) => {
    const state = get();
    const targetNode = nodeUtils.findNodeById(nodeId, state.nodes);
    if (!targetNode) return;
    const nodes = nodeUtils.toggleNodeCollapse(nodeId, state.nodes);
    const toggledNode = nodeUtils.findNodeById(nodeId, nodes);
    if (toggledNode) layoutCache.setNodeCollapsed(nodeId, toggledNode.collapsed);
    get().applyHierarchicalDependencyLayout({ pendingNodes: nodes });
  },

  expandParentNodes: (nodeId: string) => {
    const state = get();
    const updatedNodes = nodeUtils.expandParentNodes(nodeId, state.nodes);
    get().applyHierarchicalDependencyLayout({ pendingNodes: updatedNodes });
  },

  findNodeById: (nodeId: string, nodes?: Node[]) => nodeUtils.findNodeById(nodeId, nodes || get().nodes),

  applyHierarchicalDependencyLayout: ({ clearPositions = false, silent = false, pendingNodes }: { clearPositions?: boolean; silent?: boolean; pendingNodes?: Node[] } = {}) => {
    const state = get();
    const nodesSource = pendingNodes ?? state.nodes;

    // Compute visible edges from the nodes we're about to lay out.
    // Do NOT commit to the store yet — applied atomically with nodes in the .then().
    const visibleEdges = edgeUtils.getVisibleEdges(state.allEdges, nodesSource, state.visibleEdgeDepth);

    const layoutEngine = new WorkerLayoutEngine(nodesSource, state.layoutSettings);
    if (!silent) {
      if (clearPositions) {
        set({ isLayoutLoading: true, loadingPhase: "layout" });
      } else {
        set({ isLayoutLoading: true });
      }
    }

    // Optionally clear session positions (e.g. on filter/search change)
    if (clearPositions) set({ sessionPositions: {} });

    function mapNode(n: Node): Node {
      return { ...n, children: n.collapsed ? [] : n.children ? n.children.map(mapNode) : [] };
    }
    const nodes = nodesSource.map(mapNode);

    // Build position hints from session positions + pinned positions
    const currentPositions = clearPositions ? {} : get().sessionPositions;
    const pinnedPositions = layoutCache.getPinnedPositions();
    const mergedPositions = { ...currentPositions, ...pinnedPositions };
    const pinnedIds = new Set(Object.keys(pinnedPositions));
    const hints: PositionHints | undefined = Object.keys(mergedPositions).length > 0
      ? { positions: mergedPositions, pinned: pinnedIds }
      : undefined;

    layoutEngine.applyHierarchicalDependencyLayout(nodes, visibleEdges, hints).then((newNodes) => {
      const convertedNodes = layoutEngine.convertFromELKResults(newNodes.children);

      // Update sessionPositions from new layout (non-pinned nodes only)
      const newSessionPositions: Record<string, { x: number; y: number }> = {};
      const collectPositions = (nodeList: Node[]) => {
        nodeList.forEach((n) => {
          if (!pinnedIds.has(n.id)) newSessionPositions[n.id] = { x: n.x, y: n.y };
          if (n.children) collectPositions(n.children);
        });
      };
      collectPositions(convertedNodes);

      const calculateNodeBounds = (nodeList: Node[]) => {
        const allNodes: Node[] = [];
        const collectAllNodes = (nodes: Node[]) => {
          nodes.forEach((node) => { allNodes.push(node); if (node.children) collectAllNodes(node.children); });
        };
        collectAllNodes(nodeList);
        if (allNodes.length === 0) return { minX: 0, minY: 0, maxX: 1200, maxY: 800 };
        return {
          minX: Math.min(...allNodes.map((n) => n.x)),
          minY: Math.min(...allNodes.map((n) => n.y)),
          maxX: Math.max(...allNodes.map((n) => n.x + n.width)),
          maxY: Math.max(...allNodes.map((n) => n.y + n.height)),
        };
      };
      const bounds = calculateNodeBounds(convertedNodes);
      set({
        nodes: convertedNodes,
        visibleEdges,
        sessionPositions: newSessionPositions,
        layoutTransition: !clearPositions,
        graphWidth: bounds.maxX - bounds.minX,
        graphHeight: bounds.maxY - bounds.minY,
        graphBounds: bounds,
        ...(clearPositions ? { viewport: { x: 0, y: 0, zoom: 1 } } : {}),
        isLayoutLoading: false,
        loadingPhase: null,
      });
    });
  },

  setLayoutLoading: (loading: boolean) => {
    set({ isLayoutLoading: loading });
  },

  setGraphDimensions: (width: number, height: number) => {
    set({ graphWidth: width, graphHeight: height });
  },

  selectNode: (nodeId: string | null) => {
    const state = get();
    if (nodeId === state.selectedNodeId) { get().clearHighlights(); return; }
    if (nodeId) {
      const { selectedNodeId, highlightedNodes } = selectionAndHighlighting.selectNode(
        nodeId, state.selectedNodeId, state.nodes, state.allEdges,
      );
      const highlightedIds = new Set<string>();
      const collectHighlighted = (nodes: Node[]) => {
        nodes.forEach((node) => { if (node.highlighted) highlightedIds.add(node.id); if (node.children) collectHighlighted(node.children); });
      };
      collectHighlighted(highlightedNodes);
      const highlightedEdges = state.visibleEdges.map((edge) => ({
        ...edge, highlighted: highlightedIds.has(edge.source) && highlightedIds.has(edge.target),
      }));
      set({ selectedNodeId, nodes: highlightedNodes, visibleEdges: highlightedEdges });
    } else {
      get().clearHighlights();
    }
  },

  setHoveredNode: (nodeId: string | null) => {
    set({ hoveredNodeId: nodeId });
  },

  highlightDependencies: (nodeId: string, direction: "upstream" | "downstream" | "both") => {
    const state = get();
    const highlightedNodes = selectionAndHighlighting.highlightDependencies(nodeId, direction, state.nodes, state.allEdges);
    set({ nodes: highlightedNodes });
  },

  highlightDependenciesCustom: (selectedNodeId: string, dependencies: Set<string>, dependents: Set<string>) => {
    const state = get();
    const highlightedNodes = selectionAndHighlighting.highlightDependenciesCustom(selectedNodeId, dependencies, dependents, state.nodes);
    set({ nodes: highlightedNodes });
  },

  clearHighlights: () => {
    const state = get();
    const clearNodeHighlights = (nodes: Node[]): Node[] =>
      nodes.map((node) => ({ ...node, highlighted: null, children: node.children ? clearNodeHighlights(node.children) : undefined }));
    const clearEdgeHighlights = (edges: Edge[]): Edge[] => edges.map((edge) => ({ ...edge, highlighted: false }));

    set({
      nodes: clearNodeHighlights(state.nodes),
      visibleEdges: clearEdgeHighlights(state.visibleEdges),
      selectedNodeId: null, hoveredNodeId: null,
    });
  },

  setRenderMode: (_mode: "svg") => {
    set({ renderMode: "svg" });
  },

  setViewport: (viewport: Viewport) => {
    set({ viewport });
  },

  setZoomBounds: (bounds: ZoomBounds) => {
    set({ zoomBounds: bounds });
  },

  setSvgRef: (ref: React.RefObject<SVGSVGElement> | null) => {
    set({ svgRef: ref });
  },

  exportPng: async () => {
    const state = get();
    if (!state.svgRef?.current) { console.warn("SVG ref not available for PNG export"); return; }
    try {
      const { exportSvgToPng, getThemeBackgroundColor } = await import('@ui/lib/svgToPng');
      await exportSvgToPng(state.svgRef.current, "serpentine-graph.png", {
        backgroundColor: getThemeBackgroundColor(), scale: 2,
      });
    } catch (error) {
      console.error("Failed to export PNG:", error);
    }
  },

  setLayoutSettings: (patch: Partial<LayoutSettings>) => {
    const state = get();
    const updated = { ...state.layoutSettings, ...patch };
    layoutCache.saveLayoutSettings(patch);
    set({ layoutSettings: updated });
    get().applyHierarchicalDependencyLayout({ silent: true });
  },

  flipLayoutDirection: () => {
    const state = get();
    const newDirection = state.layoutSettings.rootDirection === "RIGHT" ? "DOWN" : "RIGHT";
    get().setLayoutSettings({ rootDirection: newDirection });
  },

  setWsSend: (fn: (msg: object) => void) => {
    set({ wsSend: fn });
  },

  dismissChange: (nodeId: string) => {
    const state = get();
    state.wsSend?.({ action: "dismiss_change", data: { node_id: nodeId } });
    const updateNodes = (nodes: Node[]): Node[] =>
      nodes
        .filter((n) => !(n.isGhost && n.id === nodeId))
        .map((n) =>
          n.id === nodeId
            ? { ...n, changeStatus: null }
            : { ...n, children: n.children ? updateNodes(n.children) : undefined }
        );
    set({ nodes: updateNodes(state.nodes) });
  },

  dismissAllChanges: () => {
    const state = get();
    state.wsSend?.({ action: "dismiss_all_changes" });
    const clearNodes = (nodes: Node[]): Node[] =>
      nodes
        .filter((n) => !n.isGhost)
        .map((n) => ({
          ...n,
          changeStatus: null,
          children: n.children ? clearNodes(n.children) : undefined,
        }));
    set({ nodes: clearNodes(state.nodes) });
  },

  setSidebarExpansionSignal: (signal) => {
    set({ sidebarExpansionSignal: signal });
  },

  expandAll: () => {
    const state = get();
    const updated = nodeUtils.setAllCollapsed(state.nodes, false);
    const persistAll = (nodes: Node[]) => nodes.forEach((n) => { layoutCache.setNodeCollapsed(n.id, false); if (n.children) persistAll(n.children); });
    persistAll(updated);
    get().applyHierarchicalDependencyLayout({ pendingNodes: updated });
    get().setSidebarExpansionSignal({ type: "expand", nodeId: null });
  },

  collapseAll: () => {
    const state = get();
    const updated = nodeUtils.setAllCollapsed(state.nodes, true);
    const persistAll = (nodes: Node[]) => nodes.forEach((n) => { layoutCache.setNodeCollapsed(n.id, true); if (n.children) persistAll(n.children); });
    persistAll(updated);
    get().applyHierarchicalDependencyLayout({ pendingNodes: updated });
    get().setSidebarExpansionSignal({ type: "collapse", nodeId: null });
  },

  expandChildren: (nodeId: string) => {
    const state = get();
    const updated = nodeUtils.setSubtreeCollapsed(state.nodes, nodeId, false);
    const persistSubtree = (nodes: Node[], target: string, found: boolean) =>
      nodes.forEach((n) => {
        if (found || n.id === target) { layoutCache.setNodeCollapsed(n.id, false); if (n.children) persistSubtree(n.children, target, true); }
        else if (n.children) persistSubtree(n.children, target, false);
      });
    persistSubtree(updated, nodeId, false);
    get().applyHierarchicalDependencyLayout({ pendingNodes: updated });
    get().setSidebarExpansionSignal({ type: "expand", nodeId });
  },

  collapseChildren: (nodeId: string) => {
    const state = get();
    const updated = nodeUtils.setSubtreeCollapsed(state.nodes, nodeId, true);
    const persistSubtree = (nodes: Node[], target: string, found: boolean) =>
      nodes.forEach((n) => {
        if (found) { layoutCache.setNodeCollapsed(n.id, true); if (n.children) persistSubtree(n.children, target, true); }
        else if (n.id === target && n.children) persistSubtree(n.children, target, true);
        else if (n.children) persistSubtree(n.children, target, false);
      });
    persistSubtree(updated, nodeId, false);
    get().applyHierarchicalDependencyLayout({ pendingNodes: updated });
    get().setSidebarExpansionSignal({ type: "collapse", nodeId });
  },

  resetLayout: () => {
    layoutCache.clear();
    const state = get();
    get().initialize(state.apiNodes, state.apiEdges);
  },
});
