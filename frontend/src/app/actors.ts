import { createActor } from 'xstate';
import { wsMachine, sendToSocket } from './wsMachine';
import { queryClient } from './queryClient';
import { bus } from './bus';
import { transformData } from '@domains/graph';

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
              ['graph', '', '', ''],
              // eslint-disable-next-line @typescript-eslint/no-explicit-any
              transformData(event.payload.data as any),
            );
          }

          // Invalidate any active filter queries so they re-fetch with their params.
          queryClient.invalidateQueries({
            queryKey: ['graph'],
            predicate: (query) => {
              const [, select, exclude, state] = query.queryKey as string[];
              return !!(select || exclude || state);
            },
          });

          bus.publish({ type: 'GRAPH_UPDATED' });
        }
      },
    },
  }),
);

wsActor.start();

export { sendToSocket as sendWsMessage };
