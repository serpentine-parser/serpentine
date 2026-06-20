import { createActor } from 'xstate';
import { wsMachine, sendToSocket } from './wsMachine';
import { queryClient } from './queryClient';
import { bus } from './bus';
import { transformData } from '@domains/graph';
import { fetchVcsRefs, VcsRef } from '@domains/vcs';
import { useGraphStore } from '../store';

const API_BASE = (import.meta.env.VITE_API_URL ?? '').replace(/\/$/, '');

/** Derive the WebSocket URL from VITE_API_URL, or fall back to same-host /ws at runtime. */
export const wsUrl: string | undefined = API_BASE
  ? API_BASE.replace(/^https?/, 'ws') + '/ws'
  : undefined;

export const wsActor = createActor(
  wsMachine.provide({
    actions: {
      onMessage: ({ event }) => {
        if (event.type !== 'MESSAGE') return;
        if (event.payload?.type === 'graph_update') {
          // Populate the no-filter React Query cache directly from the WS payload,
          // eliminating the redundant HTTP GET on every update.
          // The WS always sends the full unfiltered graph, so we only set the base key.
          if (event.payload.data) {
            queryClient.setQueryData(
              ['graph', '', ''],
              // eslint-disable-next-line @typescript-eslint/no-explicit-any
              transformData(event.payload.data as any),
            );
          }

          // Invalidate any active filter queries so they re-fetch with their params.
          queryClient.invalidateQueries({
            queryKey: ['graph'],
            predicate: (query) => {
              const [, select, exclude] = query.queryKey as string[];
              return !!(select || exclude);
            },
          });

          bus.publish({ type: 'GRAPH_UPDATED' });
        } else if (event.payload?.type === 'node_code') {
          const { qualname, code } = event.payload.data as { qualname: string; code: string | null };
          if (qualname && code) {
            useGraphStore.getState().setNodeCodeBlock(qualname, code);
          }
        } else if (event.payload?.type === 'error') {
          bus.publish({ type: 'GRAPH_UPDATED' });
        } else if (event.payload?.type === 'vcs_refs') {
          const { refs, available } = event.payload.data as { refs: VcsRef[]; available: boolean };
          useGraphStore.getState().setVcsRefs(refs, available);
        }
      },
    },
  }),
);

wsActor.start();

// On every WS connect (including reconnects), clear any lingering server-side
// comparison state so the client always starts from a clean baseline.
let _wasConnected = false;
wsActor.subscribe((snapshot) => {
  const isConnected = snapshot.matches({ active: 'connected' });
  if (isConnected && !_wasConnected) {
    sendToSocket({ action: 'clear_vcs_comparison' });
  }
  _wasConnected = isConnected;
});

// Fetch VCS refs on startup
fetchVcsRefs().then(({ refs, available }) => {
  useGraphStore.getState().setVcsRefs(refs, available);
});

// Wire VCS comparison bus events → WebSocket actions
bus.subscribe((event) => {
  if (event.type === 'VCS_COMPARISON_SET') {
    sendToSocket({ action: 'set_vcs_comparison', data: { from: event.from, to: event.to } });
  } else if (event.type === 'VCS_COMPARISON_CLEARED') {
    sendToSocket({ action: 'clear_vcs_comparison' });
  }
});

export { sendToSocket as sendWsMessage };
