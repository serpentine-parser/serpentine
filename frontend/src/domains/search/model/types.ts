export type SearchState = {
  searchQuery: string;
  excludeQuery: string;
  selectorQuery: string;
  selectorExclude: string;
  includeStandardPackages: boolean;
  includeThirdPartyPackages: boolean;
  visibleEdgeDepth: number;
};
