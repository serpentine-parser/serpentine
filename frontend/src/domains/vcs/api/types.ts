export interface RawVcsRef {
  id: string;
  display: string;
  kind: 'branch' | 'tag' | 'commit';
}

export interface VcsRefsResponse {
  available: boolean;
  refs: RawVcsRef[];
}
