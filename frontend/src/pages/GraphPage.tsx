import { useQuery } from '@tanstack/react-query';
import { DetailPanel } from '@ui/components/DetailPanel';
import { GraphContainer } from '@ui/components/GraphContainer';
import Header from '@ui/components/Header';
import { SearchBar } from '@ui/components/SearchBar';
import { Shell } from '@ui/components/Shell';
import { Sidebar, SidebarExpansionSignal } from '@ui/components/Sidebar';
import { useTheme } from '@ui/lib/ThemeContext';
import { useEffect, useMemo, useState } from 'react';
import { useGraphStore } from '@store';
import { loadData } from '../domains/graph/api';

export default function GraphPage() {
  const { theme, setTheme } = useTheme();

  // Graph slice
  const nodes = useGraphStore((s) => s.nodes);
  const visibleEdges = useGraphStore((s) => s.visibleEdges);
  const allEdges = useGraphStore((s) => s.allEdges);
  const allNodes = useGraphStore((s) => s.allNodes);
  const catalogNodes = useGraphStore((s) => s.catalogNodes);
  const selectedNodeId = useGraphStore((s) => s.selectedNodeId);
  const hoveredNodeId = useGraphStore((s) => s.hoveredNodeId);
  const viewport = useGraphStore((s) => s.viewport);
  const zoomBounds = useGraphStore((s) => s.zoomBounds);
  const graphWidth = useGraphStore((s) => s.graphWidth);
  const graphHeight = useGraphStore((s) => s.graphHeight);
  const graphBounds = useGraphStore((s) => s.graphBounds);
  const storeLoadingPhase = useGraphStore((s) => s.loadingPhase);
  const loadingNodeCount = useGraphStore((s) => s.loadingNodeCount);
  const layoutTransition = useGraphStore((s) => s.layoutTransition);
  const layoutSettings = useGraphStore((s) => s.layoutSettings);
  const selectNode = useGraphStore((s) => s.selectNode);
  const setHoveredNode = useGraphStore((s) => s.setHoveredNode);
  const clearHighlights = useGraphStore((s) => s.clearHighlights);
  const setViewport = useGraphStore((s) => s.setViewport);
  const setSvgRef = useGraphStore((s) => s.setSvgRef);
  const findNodeById = useGraphStore((s) => s.findNodeById);
  const toggleNodeCollapse = useGraphStore((s) => s.toggleNodeCollapse);
  const dismissChange = useGraphStore((s) => s.dismissChange);
  const dismissAllChanges = useGraphStore((s) => s.dismissAllChanges);
  const expandParentNodes = useGraphStore((s) => s.expandParentNodes);
  const moveChildWithConstraints = useGraphStore((s) => s.moveChildWithConstraints);
  const expandChildren = useGraphStore((s) => s.expandChildren);
  const collapseChildren = useGraphStore((s) => s.collapseChildren);
  const expandAll = useGraphStore((s) => s.expandAll);
  const collapseAll = useGraphStore((s) => s.collapseAll);
  const resetLayout = useGraphStore((s) => s.resetLayout);
  const flipLayoutDirection = useGraphStore((s) => s.flipLayoutDirection);
  const setLayoutSettings = useGraphStore((s) => s.setLayoutSettings);
  const exportPng = useGraphStore((s) => s.exportPng);
  const sidebarExpansionSignal = useGraphStore((s) => s.sidebarExpansionSignal);
  const setSidebarExpansionSignal = useGraphStore((s) => s.setSidebarExpansionSignal);

  // PDG slice
  const pdgFlatNodes = useGraphStore((s) => s.pdgFlatNodes);
  const pdgEdges = useGraphStore((s) => s.pdgEdges);
  const pdgAllEdges = useGraphStore((s) => s.pdgAllEdges);
  const isPdgLayoutLoading = useGraphStore((s) => s.isPdgLayoutLoading);
  const pdgVisibleEdgeTypes = useGraphStore((s) => s.pdgVisibleEdgeTypes);
  const setPdgVisibleEdgeTypes = useGraphStore((s) => s.setPdgVisibleEdgeTypes);
  const selectPdgNode = useGraphStore((s) => s.selectPdgNode);
  const togglePdgNodeCollapse = useGraphStore((s) => s.togglePdgNodeCollapse);
  const movePdgNodeWithConstraints = useGraphStore((s) => s.movePdgNodeWithConstraints);

  // Search slice
  const searchQuery = useGraphStore((s) => s.searchQuery);
  const setSearchQuery = useGraphStore((s) => s.setSearchQuery);
  const excludeQuery = useGraphStore((s) => s.excludeQuery);
  const setExcludeQuery = useGraphStore((s) => s.setExcludeQuery);
  const executeSearch = useGraphStore((s) => s.executeSearch);
  const selectorQuery = useGraphStore((s) => s.selectorQuery);
  const selectorExclude = useGraphStore((s) => s.selectorExclude);
  const selectorState = useGraphStore((s) => s.selectorState);
  const setStateFilter = useGraphStore((s) => s.setStateFilter);
  const includeStandardPackages = useGraphStore((s) => s.includeStandardPackages);
  const includeThirdPartyPackages = useGraphStore((s) => s.includeThirdPartyPackages);
  const setIncludeStandardPackages = useGraphStore((s) => s.setIncludeStandardPackages);
  const setIncludeThirdPartyPackages = useGraphStore((s) => s.setIncludeThirdPartyPackages);
  const visibleEdgeDepth = useGraphStore((s) => s.visibleEdgeDepth);
  const setVisibleEdgeDepth = useGraphStore((s) => s.setVisibleEdgeDepth);

  // Local UI state
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);

  // Seed store from URL params on first render
  const urlParams = useMemo(() => {
    const p = new URLSearchParams(window.location.search);
    return { select: p.get('select') ?? '', exclude: p.get('exclude') ?? '' };
  }, []);

  useEffect(() => {
    const state = useGraphStore.getState();
    if (urlParams.select) state.setSelectorQuery(urlParams.select);
    if (urlParams.exclude) state.setSelectorExclude(urlParams.exclude);
  }, [urlParams]);

  const activeSelect = selectorQuery || urlParams.select;
  const activeExclude = selectorExclude || urlParams.exclude;

  const { data, isFetching } = useQuery({
    queryKey: ['graph', activeSelect, activeExclude, selectorState],
    queryFn: () => loadData(activeSelect || undefined, activeExclude || undefined, selectorState || undefined),
  });

  const loadingPhase: "data" | "layout" | null = isFetching ? "data" : storeLoadingPhase;

  useEffect(() => {
    if (!data) return;
    const state = useGraphStore.getState();
    state.initialize(data.nodes, data.edges);
  }, [data]);

  const selectedNode = selectedNodeId ? (findNodeById(selectedNodeId) ?? undefined) : undefined;

  function handleExportJson() {
    const state = useGraphStore.getState();
    const blob = new Blob([JSON.stringify({ nodes: state.allNodes, edges: state.allEdges }, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'graph.json';
    a.click();
    URL.revokeObjectURL(url);
  }

  const sidebar = (
    <Sidebar
      collapsed={sidebarCollapsed}
      onToggleCollapse={() => setSidebarCollapsed((c) => !c)}
      catalogNodes={catalogNodes}
      selectedNodeId={selectedNodeId}
      selectNode={selectNode}
      expandParentNodes={expandParentNodes}
      flipLayoutDirection={flipLayoutDirection}
      dismissAllChanges={dismissAllChanges}
      graphNodes={nodes}
      expandAll={expandAll}
      collapseAll={collapseAll}
      resetLayout={resetLayout}
      sidebarExpansionSignal={sidebarExpansionSignal}
      setSidebarExpansionSignal={setSidebarExpansionSignal}
    />
  );

  const detailPanel = selectedNode ? (
    <DetailPanel
      node={selectedNode}
      onClose={() => selectNode(null)}
      allEdges={allEdges}
      visibleEdges={visibleEdges}
      findNodeById={(id) => findNodeById(id) ?? undefined}
      selectNode={selectNode}
      expandParentNodes={expandParentNodes}
    />
  ) : undefined;

  return (
    <>
      <Header
        theme={theme}
        setTheme={setTheme}
        layoutSettings={layoutSettings}
        setLayoutSettings={setLayoutSettings}
        flipLayoutDirection={flipLayoutDirection}
        expandAll={expandAll}
        collapseAll={collapseAll}
      />
      <main className="flex-1 overflow-hidden flex flex-col">
        <Shell sidebar={sidebar} detailPanel={detailPanel}>
          <GraphContainer
              loadingPhase={loadingPhase}
              loadingNodeCount={loadingNodeCount}
              nodes={nodes}
              edges={visibleEdges}
              searchQuery={searchQuery}
              excludeQuery={excludeQuery}
              selectedNodeId={selectedNodeId}
              hoveredNodeId={hoveredNodeId}
              findNodeById={findNodeById}
              graphWidth={graphWidth}
              graphHeight={graphHeight}
              graphBounds={graphBounds}
              viewport={viewport}
              zoomBounds={zoomBounds}
              setViewport={setViewport}
              setSvgRef={setSvgRef}
              clearHighlights={clearHighlights}
              moveChildWithConstraints={moveChildWithConstraints}
              layoutTransition={layoutTransition}
              selectNode={selectNode}
              setHoveredNode={setHoveredNode}
              toggleNodeCollapse={toggleNodeCollapse}
              dismissChange={dismissChange}
              expandChildren={expandChildren}
              collapseChildren={collapseChildren}
              edgeCurvature={layoutSettings.edgeCurvature}
              edgeStrokeWidth={layoutSettings.edgeStrokeWidth}
              getNodes={() => useGraphStore.getState().nodes}
            />

          <div className="pointer-events-none">
            <div className="absolute left-1/2 transform -translate-x-1/2 bottom-8 z-40 pointer-events-auto max-w-4xl w-full px-4">
              <SearchBar
                searchQuery={searchQuery}
                setSearchQuery={setSearchQuery}
                excludeQuery={excludeQuery}
                setExcludeQuery={setExcludeQuery}
                executeSearch={executeSearch}
                exportPng={exportPng}
                nodes={nodes}
                catalogNodes={catalogNodes}
                pdgVisibleEdgeTypes={pdgVisibleEdgeTypes}
                setPdgVisibleEdgeTypes={setPdgVisibleEdgeTypes}
                pdgAllEdges={pdgAllEdges}
                includeStandardPackages={includeStandardPackages}
                includeThirdPartyPackages={includeThirdPartyPackages}
                setIncludeStandardPackages={setIncludeStandardPackages}
                setIncludeThirdPartyPackages={setIncludeThirdPartyPackages}
                visibleEdgeDepth={visibleEdgeDepth}
                setVisibleEdgeDepth={setVisibleEdgeDepth}
                selectorState={selectorState}
                setStateFilter={setStateFilter}
                onExportJson={handleExportJson}
              />
            </div>
          </div>
        </Shell>
      </main>
    </>
  );
}
