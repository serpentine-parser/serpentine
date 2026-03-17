import type { Node, Edge, Viewport, ZoomBounds } from "@domains/graph/model/types";
import { Graph } from "./Graph";
import { LoadingOverlay } from "./LoadingOverlay";

export interface GraphContainerProps {
  loadingPhase: "data" | "layout" | null;
  loadingNodeCount: number;
  nodes: Node[];
  edges: Edge[];
  searchQuery: string;
  excludeQuery: string;
  selectedNodeId: string | null;
  hoveredNodeId: string | null;
  findNodeById: (id: string) => Node | null;
  graphWidth: number;
  graphHeight: number;
  graphBounds: { minX: number; minY: number; maxX: number; maxY: number } | null;
  viewport: Viewport;
  zoomBounds: ZoomBounds;
  setViewport: (v: Viewport) => void;
  setSvgRef: (ref: React.RefObject<SVGSVGElement> | null) => void;
  clearHighlights: () => void;
  moveChildWithConstraints: (id: string, parent: string | null, x: number, y: number) => boolean;
  layoutTransition: boolean;
  selectNode: (id: string | null) => void;
  setHoveredNode: (id: string | null) => void;
  toggleNodeCollapse: (id: string) => void;
  dismissChange: (id: string) => void;
  expandChildren: (id: string) => void;
  collapseChildren: (id: string) => void;
  edgeCurvature: number;
  edgeStrokeWidth: number;
  getNodes: () => Node[];
}

export function GraphContainer({
  loadingPhase,
  loadingNodeCount,
  nodes,
  edges,
  searchQuery,
  excludeQuery,
  ...graphProps
}: GraphContainerProps) {
  if (nodes.length === 0 && !loadingPhase && (searchQuery.trim() || excludeQuery.trim())) {
    return (
      <div className="w-full h-full flex items-center justify-center">
        <div className="text-center p-6 bg-white/90 dark:bg-gray-800/90 rounded shadow">
          <div className="mb-2 font-semibold text-gray-900 dark:text-white">
            No matching nodes
          </div>
          <div className="text-sm text-gray-600 dark:text-gray-300">
            Try adjusting your selection or exclude filters.
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="relative w-full h-full">
      {loadingPhase !== "layout" && <Graph nodes={nodes} edges={edges} {...graphProps} />}
      {loadingPhase && (
        <LoadingOverlay phase={loadingPhase} nodeCount={loadingNodeCount} />
      )}
    </div>
  );
}
