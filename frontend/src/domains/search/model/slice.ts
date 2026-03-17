import { StateCreator } from 'zustand';
import { layoutCache } from '../../graph/lib/layoutPersistence';
import { SearchAndFilter } from '../lib/filter';

export type SearchSlice = {
  searchQuery: string;
  excludeQuery: string;
  selectorQuery: string;
  selectorExclude: string;
  selectorState: string;
  includeStandardPackages: boolean;
  includeThirdPartyPackages: boolean;
  visibleEdgeDepth: number;

  setSearchQuery: (query: string) => void;
  setExcludeQuery: (query: string) => void;
  setSelectorQuery: (query: string) => void;
  setSelectorExclude: (query: string) => void;
  setStateFilter: (state: string) => void;
  setIncludeStandardPackages: (include: boolean) => void;
  setIncludeThirdPartyPackages: (include: boolean) => void;
  setVisibleEdgeDepth: (depth: number) => void;
  filterNodes: (suppressLayout?: boolean) => void;
  executeSearch: () => void;
  loadCatalog: () => Promise<void>;
};

const searchAndFilter = new SearchAndFilter();

export const createSearchSlice: StateCreator<any, [], [], SearchSlice> = (set, get) => ({
  searchQuery: layoutCache.getDisplaySettings().selectorQuery ?? "",
  excludeQuery: layoutCache.getDisplaySettings().selectorExclude ?? "",
  selectorQuery: layoutCache.getDisplaySettings().selectorQuery ?? "",
  selectorExclude: layoutCache.getDisplaySettings().selectorExclude ?? "",
  selectorState: "",
  includeStandardPackages: layoutCache.getDisplaySettings().includeStandardPackages ?? false,
  includeThirdPartyPackages: layoutCache.getDisplaySettings().includeThirdPartyPackages ?? false,
  visibleEdgeDepth: layoutCache.getDisplaySettings().visibleEdgeDepth ?? Infinity,

  setSearchQuery: (query: string) => {
    set({ searchQuery: query });
  },

  setExcludeQuery: (query: string) => {
    set({ excludeQuery: query });
  },

  setSelectorQuery: (query: string) => {
    set({ selectorQuery: query });
    layoutCache.saveDisplaySettings({ selectorQuery: query });
  },

  setSelectorExclude: (query: string) => {
    set({ selectorExclude: query });
    layoutCache.saveDisplaySettings({ selectorExclude: query });
  },

  setStateFilter: (state: string) => {
    set({ selectorState: state });
  },

  setIncludeStandardPackages: (include: boolean) => {
    set({ includeStandardPackages: include });
    layoutCache.saveDisplaySettings({ includeStandardPackages: include });
    get().filterNodes();
    get().loadCatalog();
  },

  setIncludeThirdPartyPackages: (include: boolean) => {
    set({ includeThirdPartyPackages: include });
    layoutCache.saveDisplaySettings({ includeThirdPartyPackages: include });
    get().filterNodes();
    get().loadCatalog();
  },

  setVisibleEdgeDepth: (depth: number) => {
    set({ visibleEdgeDepth: depth });
    layoutCache.saveDisplaySettings({ visibleEdgeDepth: depth });
    get().setVisibleEdges();
  },

  filterNodes: (suppressLayout = false) => {
    const state = get();
    const result = searchAndFilter.filterNodes(state);
    set({ nodes: result.nodes });
    if (!suppressLayout) get().applyHierarchicalDependencyLayout({ clearPositions: true });
  },

  executeSearch: () => {
    const state = get();
    const selectorQuery = state.searchQuery.trim();
    const selectorExclude = state.excludeQuery.trim();
    // Update selector params — GraphPage's useQuery key changes and React Query refetches
    set({ selectorQuery, selectorExclude });
    layoutCache.saveDisplaySettings({ selectorQuery, selectorExclude });
  },

  loadCatalog: async () => {
    const state = get();
    const API_BASE = (import.meta.env.VITE_API_URL || "").replace(/\/$/, "");
    const params = new URLSearchParams({
      include_standard: String(state.includeStandardPackages),
      include_third_party: String(state.includeThirdPartyPackages),
    });
    try {
      const response = await fetch(`${API_BASE}/api/catalog?${params}`, { cache: "no-store" });
      if (!response.ok) throw new Error(`Catalog fetch failed: ${response.statusText}`);
      const data = await response.json();

      const mapNode = (n: any): any => ({
        id: n.id,
        x: 0, y: 0, width: 0, height: 0,
        collapsed: false,
        waitingForLayout: false,
        parent: n.parent ?? undefined,
        type: n.type,
        label: n.label ?? n.name,
        origin: n.origin,
        children: (n.children ?? []).map(mapNode),
      });

      set({ catalogNodes: (data.nodes ?? []).map(mapNode) });
    } catch (error) {
      console.error("Error loading catalog:", error);
    }
  },
});
