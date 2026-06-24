export type RefSelection = '@current' | '@start' | string;

export interface ComparisonState {
  from: RefSelection;
  to: RefSelection;
}

export interface VcsRef {
  id: string;
  display: string;
  kind: 'branch' | 'tag' | 'commit';
}

export interface VcsSlice {
  vcsRefs: VcsRef[];
  vcsAvailable: boolean;
  comparison: ComparisonState;

  setVcsRefs: (refs: VcsRef[], available: boolean) => void;
  setComparison: (comparison: ComparisonState) => void;
  clearComparison: () => void;
}
