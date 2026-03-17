// Web Worker for ELK graph layout.
// This worker runs the ELK algorithm off the main thread. ELK itself spawns an inner
// worker (elk-worker.min.js) for the actual layout computation — a total of two worker
// hops from the main thread, which is acceptable and keeps the main thread fully free.
import ELK from 'elkjs/lib/elk.bundled.js';
// ?url import tells Vite to resolve elk-worker.min.js to a real serveable URL in both
// dev and prod. We pass this as workerUrl so ELK's factory receives an actual URL
// (not `undefined`, which is what happens when only workerFactory is provided).
import elkWorkerUrl from 'elkjs/lib/elk-worker.min.js?url';
import { convertToELKGraph, ElkGraph, PositionHints } from './elkConversion';
import type { LayoutSettings } from '../model/layoutTypes';
import type { Edge, Node } from '../model/types';

interface LayoutPayload {
  nodes: Node[];
  edges: Edge[];
  hints?: PositionHints;
  settings: LayoutSettings;
}

// Do not pass workerUrl — that triggers ELK's Node.js "web-worker package" branch
// which prints a spurious warning. Instead, the factory ignores its (undefined) url
// argument and directly uses our resolved elkWorkerUrl.
const elk = new (ELK as any)({
  workerFactory: () => new Worker(elkWorkerUrl),
});

addEventListener('message', async (event: MessageEvent) => {
  const { type, payload } = event.data as { type: string; payload: LayoutPayload };
  if (type !== 'LAYOUT') return;

  const { nodes, edges, hints, settings } = payload;

  // Reconstruct a Set for hints.pinned — structured clone preserves Sets, but
  // we reconstruct defensively in case older browsers serialize them as arrays.
  const typedHints: PositionHints | undefined = hints
    ? { positions: hints.positions, pinned: new Set(hints.pinned) }
    : undefined;

  try {
    postMessage({ type: 'LAYOUT_PROGRESS', phase: 'preparing' });
    const elkGraph = convertToELKGraph(nodes, edges, typedHints, settings);
    postMessage({ type: 'LAYOUT_PROGRESS', phase: 'running' });
    const laidOut = await elk.layout(elkGraph);
    const result: ElkGraph = { ...laidOut, children: laidOut.children ?? [] };
    postMessage({ type: 'LAYOUT_DONE', payload: result });
  } catch (error) {
    postMessage({ type: 'LAYOUT_ERROR', error: String(error) });
  }
});
