/** Domain event types. Zero external imports. */

export type GraphUpdatedEvent = {
  type: 'GRAPH_UPDATED';
};

export type VcsComparisonSetEvent = {
  type: 'VCS_COMPARISON_SET';
  from: string;
  to: string;
};

export type VcsComparisonClearedEvent = {
  type: 'VCS_COMPARISON_CLEARED';
};

export type DomainEvent =
  | GraphUpdatedEvent
  | VcsComparisonSetEvent
  | VcsComparisonClearedEvent;
