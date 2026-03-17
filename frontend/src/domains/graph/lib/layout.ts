import ELK, { ElkNode } from "elkjs";
import { DEFAULT_LAYOUT_SETTINGS, LayoutSettings } from '../model/layoutTypes';
import { Edge, Node } from '../model/types';
import { getSimpleEdges } from './edgeUtils';
import { convertToELKGraph, ElkGraph, PositionHints } from './elkConversion';
import { nodeUtils } from './nodeUtils';

export type { ElkGraph, PositionHints };

export class LayoutEngine {
  private elk: InstanceType<typeof ELK>;
  protected nodeMap: Map<string, Node>;
  protected settings: LayoutSettings;

  constructor(nodes: Node[], settings?: LayoutSettings) {
    this.elk = new ELK();
    this.nodeMap = new Map();
    this.settings = settings ?? DEFAULT_LAYOUT_SETTINGS;
    this.buildNodeMap(nodes);
  }

  private buildNodeMap(nodes: Node[]) {
    for (const n of nodes) {
      this.nodeMap.set(n.id, n);
      if (Array.isArray(n.children) && n.children.length > 0) this.buildNodeMap(n.children);
    }
  }

  async applyHierarchicalDependencyLayout(
    nodes: Node[],
    edges: Edge[],
    hints?: PositionHints,
    skipEdgeProcessing: boolean = false,
  ): Promise<ElkGraph> {
    const processedEdges = skipEdgeProcessing ? edges : getSimpleEdges(edges, nodes, Infinity);
    try {
      const elkGraph = convertToELKGraph(nodes, processedEdges, hints, this.settings);
      const laidOutGraph = await this.elk.layout(elkGraph);
      return { ...laidOutGraph, children: laidOutGraph.children ?? [] };
    } catch (error) {
      console.error("ELK layout failed:", error);
      console.error("Error details:", JSON.stringify(error, null, 2));
      throw new Error(`ELK layout failed: ${error}`);
    }
  }

  // Height of the visible header bar — kept here as a constant for convertFromELKResults.
  private static readonly HEADER_HEIGHT = 25;

  convertFromELKResults(elkNodes: ElkNode[], offset: { x: number; y: number } = { x: 0, y: 0 }): Node[] {
    if (!elkNodes || elkNodes.length === 0) return [];
    return elkNodes.map((elkNode): Node => {
      const original = this.nodeMap.get(elkNode.id) || ({} as Node);
      const absX = (elkNode.x ?? 0) + offset.x;
      const absY = (elkNode.y ?? 0) + offset.y;

      let children: Node[] = [];
      if (elkNode.children && elkNode.children.length > 0) {
        children = this.convertFromELKResults(elkNode.children, { x: absX, y: absY });
      } else if (original.children && original.children.length > 0) {
        let currentY = absY + LayoutEngine.HEADER_HEIGHT + 10;
        children = original.children.map((child) => {
          const positioned = { ...child, x: absX + 10, y: currentY };
          currentY += (child.height ?? 50) + 10;
          if (child.children && child.children.length > 0)
            positioned.children = this.positionChildrenRecursively(child.children, absX + 20, currentY);
          return positioned;
        });
      }

      return {
        ...original,
        id: elkNode.id,
        x: absX,
        y: absY,
        width: elkNode.width ?? original.width ?? 100,
        height: elkNode.height ?? original.height ?? 50,
        children,
      };
    });
  }

  protected positionChildrenRecursively(children: Node[], parentX: number, parentY: number): Node[] {
    let currentY = parentY + 10;
    return children.map((child) => {
      const positioned = { ...child, x: parentX, y: currentY };
      currentY += (child.height ?? 50) + 10;
      if (child.children && child.children.length > 0)
        positioned.children = this.positionChildrenRecursively(child.children, parentX + 10, currentY);
      return positioned;
    });
  }
}

// Module-level worker instance shared across all WorkerLayoutEngine calls.
// Terminated and recreated whenever a new layout request arrives, so stale
// layouts never overwrite a newer result.
let sharedWorker: Worker | null = null;

export class WorkerLayoutEngine extends LayoutEngine {
  async applyHierarchicalDependencyLayout(
    nodes: Node[],
    edges: Edge[],
    hints?: PositionHints,
    skipEdgeProcessing: boolean = false,
  ): Promise<ElkGraph> {
    if (typeof Worker === 'undefined') {
      return super.applyHierarchicalDependencyLayout(nodes, edges, hints, skipEdgeProcessing);
    }

    // Terminate any in-flight layout so its result is never applied.
    if (sharedWorker) {
      sharedWorker.terminate();
      sharedWorker = null;
    }

    const processedEdges = skipEdgeProcessing ? edges : getSimpleEdges(edges, nodes, Infinity);

    const worker = new Worker(new URL('./layoutWorker.ts', import.meta.url), { type: 'module' });
    sharedWorker = worker;

    return new Promise<ElkGraph>((resolve, reject) => {
      worker.onmessage = (e: MessageEvent) => {
        const { type, payload, error } = e.data;
        if (type === 'LAYOUT_DONE') resolve(payload);
        else if (type === 'LAYOUT_ERROR') reject(new Error(error));
        // LAYOUT_PROGRESS messages are informational — ignored here
      };
      worker.onerror = (e: ErrorEvent) => reject(new Error(e.message));
      worker.postMessage({
        type: 'LAYOUT',
        payload: { nodes, edges: processedEdges, hints, settings: this.settings },
      });
    });
  }
}
