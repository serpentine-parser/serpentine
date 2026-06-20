import { StateCreator } from 'zustand';
import { loadComparison, saveComparison } from '../lib/comparisonPersistence';
import { ComparisonState, VcsRef, VcsSlice } from './types';

const DEFAULT_COMPARISON: ComparisonState = { from: '@start', to: '@current' };

export const createVcsSlice: StateCreator<any, [], [], VcsSlice> = (set) => ({
  vcsRefs: [],
  vcsAvailable: false,
  comparison: loadComparison() ?? DEFAULT_COMPARISON,

  setVcsRefs: (refs: VcsRef[], available: boolean) => {
    set({ vcsRefs: refs, vcsAvailable: available });
  },

  setComparison: (comparison: ComparisonState) => {
    saveComparison(comparison);
    set({ comparison });
  },

  clearComparison: () => {
    saveComparison(DEFAULT_COMPARISON);
    set({ comparison: DEFAULT_COMPARISON });
  },
});
