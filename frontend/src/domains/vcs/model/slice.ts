import { StateCreator } from 'zustand';
import { ComparisonState, VcsRef, VcsSlice } from './types';

const DEFAULT_COMPARISON: ComparisonState = { from: '@start', to: '@current' };

export const createVcsSlice: StateCreator<any, [], [], VcsSlice> = (set) => ({
  vcsRefs: [],
  vcsAvailable: false,
  comparison: DEFAULT_COMPARISON,

  setVcsRefs: (refs: VcsRef[], available: boolean) => {
    set({ vcsRefs: refs, vcsAvailable: available });
  },

  setComparison: (comparison: ComparisonState) => {
    set({ comparison });
  },

  clearComparison: () => {
    set({ comparison: DEFAULT_COMPARISON });
  },
});
