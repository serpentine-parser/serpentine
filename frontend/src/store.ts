import { create } from 'zustand';
import { createGraphSlice, GraphSlice } from './domains/graph';
import { createPdgSlice, PdgSlice } from './domains/pdg';
import { createSearchSlice, SearchSlice } from './domains/search';

export type AppState = GraphSlice & PdgSlice & SearchSlice;

export const useGraphStore = create<AppState>()((...a) => ({
  ...createGraphSlice(...a),
  ...createPdgSlice(...a),
  ...createSearchSlice(...a),
}));

// Helper re-export for backward compat
export { flattenCfgNodes } from './domains/pdg';
export type { Node, Edge, Viewport, ZoomBounds } from './domains/graph';
