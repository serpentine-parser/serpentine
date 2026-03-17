import { setup, assign, fromCallback } from 'xstate';

export type WsContext = { url: string; retries: number };

export type WsEvent =
  | { type: 'CONNECT'; url: string }
  | { type: 'WS_OPEN' }
  | { type: 'WS_CLOSE' }
  | { type: 'WS_ERROR' }
  | { type: 'MESSAGE'; payload: Record<string, unknown> };

// Module-level socket reference for sending messages
let _socket: WebSocket | null = null;

export function sendToSocket(message: object): void {
  if (_socket?.readyState === WebSocket.OPEN) {
    _socket.send(JSON.stringify(message));
  }
}

export const wsMachine = setup({
  types: {
    context: {} as WsContext,
    events: {} as WsEvent,
  },
  actors: {
    socketActor: fromCallback(({
      sendBack,
      input,
    }: {
      sendBack: (e: WsEvent) => void;
      input: { url: string };
    }) => {
      const socket = new WebSocket(input.url);
      _socket = socket;
      socket.onopen = () => sendBack({ type: 'WS_OPEN' });
      socket.onclose = () => { _socket = null; sendBack({ type: 'WS_CLOSE' }); };
      socket.onerror = () => { _socket = null; sendBack({ type: 'WS_ERROR' }); };
      socket.onmessage = (e) => {
        try {
          sendBack({ type: 'MESSAGE', payload: JSON.parse(e.data) });
        } catch {
          // ignore malformed messages
        }
      };
      return () => { _socket = null; socket.close(); };
    }),
  },
  actions: {
    onMessage: () => {},
  },
}).createMachine({
  id: 'ws',
  initial: 'idle',
  context: { url: '', retries: 0 },
  states: {
    idle: {
      on: {
        CONNECT: {
          target: 'active',
          actions: assign({ url: ({ event }) => event.url }),
        },
      },
    },
    active: {
      invoke: { src: 'socketActor', input: ({ context }) => ({ url: context.url }) },
      initial: 'connecting',
      states: {
        connecting: {
          on: { WS_OPEN: 'connected' },
        },
        connected: {
          on: {
            MESSAGE: { actions: 'onMessage' },
          },
        },
      },
      on: {
        WS_CLOSE: 'reconnecting',
        WS_ERROR: 'reconnecting',
      },
    },
    reconnecting: {
      after: {
        3000: {
          target: 'active',
          actions: assign({ retries: ({ context }) => context.retries + 1 }),
        },
      },
    },
  },
});
