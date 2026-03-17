import * as d3 from "d3";
import { useEffect, useMemo, useRef } from "react";
import type { Node, Edge as EdgeData, Viewport, ZoomBounds } from "@domains/graph/model/types";
import Edge from "./viewNodes/Edge";
import NodeGroup from "./viewNodes/NodeGroup";
import { SvgNodeContextMenu } from "./viewNodes/SvgNodeContextMenu";
import type { NodeInteractionProps } from "./viewNodes/nodeInteraction";

interface GraphProps {
  nodes: Node[];
  edges: EdgeData[];
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

export function Graph({
  nodes,
  edges,
  selectedNodeId,
  hoveredNodeId,
  findNodeById,
  graphWidth,
  graphHeight,
  graphBounds,
  viewport,
  zoomBounds,
  setViewport,
  setSvgRef,
  clearHighlights,
  moveChildWithConstraints,
  layoutTransition,
  selectNode,
  setHoveredNode,
  toggleNodeCollapse,
  dismissChange,
  expandChildren,
  collapseChildren,
  edgeCurvature,
  edgeStrokeWidth,
  getNodes,
}: GraphProps) {
  const ref = useRef<SVGSVGElement>(null);
  const gRef = useRef<SVGGElement>(null);
  const zoomRef = useRef<d3.ZoomBehavior<Element, unknown> | null>(null);

  const selectedNode = selectedNodeId ? findNodeById(selectedNodeId) : null;

  const { visibleNodes, bgRect } = useMemo(() => {
    const { x: tx, y: ty, zoom: k } = viewport;
    const svgW = ref.current?.clientWidth ?? 1200;
    const svgH = ref.current?.clientHeight ?? 800;
    const left = -tx / k;
    const top = -ty / k;
    const right = left + svgW / k;
    const bottom = top + svgH / k;

    const filtered = !graphBounds
      ? nodes
      : nodes.filter(
          (n) =>
            n.x + (n.width ?? 200) >= left &&
            n.x <= right &&
            n.y + (n.height ?? 50) >= top &&
            n.y <= bottom
        );

    // Cover union of graph bounds and current viewport so dots fill everywhere the user can see
    const pad = 2000;
    const minX = Math.min(graphBounds ? graphBounds.minX : 0, left) - pad;
    const minY = Math.min(graphBounds ? graphBounds.minY : 0, top) - pad;
    const maxX = Math.max(graphBounds ? graphBounds.maxX : 0, right) + pad;
    const maxY = Math.max(graphBounds ? graphBounds.maxY : 0, bottom) + pad;

    return {
      visibleNodes: filtered,
      bgRect: { x: minX, y: minY, width: maxX - minX, height: maxY - minY },
    };
  }, [nodes, viewport, graphBounds]);

  const interaction: NodeInteractionProps = {
    selectedNodeId,
    hoveredNodeId,
    onToggleCollapse: toggleNodeCollapse,
    onDismissChange: dismissChange,
  };

  useEffect(() => {
    setSvgRef(ref);
    return () => setSvgRef(null);
  }, [setSvgRef]);

  useEffect(() => {
    if (!ref.current || !gRef.current) return;

    const svg = d3.select(ref.current);
    const g = d3.select(gRef.current);

    const zoom = d3
      .zoom()
      .scaleExtent([zoomBounds.minZoom, zoomBounds.maxZoom])
      .on("zoom", (event) => {
        g.attr("transform", event.transform);
        setViewport({
          x: event.transform.x,
          y: event.transform.y,
          zoom: event.transform.k,
        });
      });

    svg.call(zoom as any);
    zoomRef.current = zoom as any;

    const storedTransform = d3.zoomIdentity
      .translate(viewport.x, viewport.y)
      .scale(viewport.zoom);

    const isInitialLoad =
      viewport.x === 0 && viewport.y === 0 && viewport.zoom === 1;

    if (isInitialLoad && graphBounds) {
      const svgWidth = ref.current.clientWidth;
      const svgHeight = ref.current.clientHeight;

      const scaleX = svgWidth / graphWidth;
      const scaleY = svgHeight / graphHeight;
      const scale = Math.min(scaleX, scaleY) * 0.8;

      const centerX =
        (svgWidth - graphWidth * scale) / 2 - graphBounds.minX * scale;
      const centerY =
        (svgHeight - graphHeight * scale) / 2 - graphBounds.minY * scale;

      const initialTransform = d3.zoomIdentity
        .translate(centerX, centerY)
        .scale(scale);

      svg.call(zoom.transform as any, initialTransform);
      setViewport({ x: centerX, y: centerY, zoom: scale });
    } else {
      svg.call(zoom.transform as any, storedTransform);
    }

    return () => {
      svg.on(".zoom", null);
    };
  }, [
    graphWidth,
    graphHeight,
    zoomBounds.minZoom,
    zoomBounds.maxZoom,
    setViewport,
  ]);

  useEffect(() => {
    if (!ref.current || !gRef.current) return;
    const g = d3.select(gRef.current);
    const storedTransform = d3.zoomIdentity
      .translate(viewport.x, viewport.y)
      .scale(viewport.zoom);
    g.attr("transform", storedTransform.toString());
  }, [viewport.x, viewport.y, viewport.zoom]);

  useEffect(() => {
    if (!selectedNodeId || !ref.current || !zoomRef.current) return;
    const node = findNodeById(selectedNodeId);
    if (!node) return;
    const el = ref.current;
    const vw = el.clientWidth || 800;
    const vh = el.clientHeight || 600;
    const pad = 80;
    const fitZoom = Math.min(
      vw / (node.width + pad * 2),
      vh / (node.height + pad * 2),
      0.8,
    );
    const tx = vw / 2 - (node.x + node.width / 2) * fitZoom;
    const ty = vh / 2 - (node.y + node.height / 2) * fitZoom;
    const t = d3.zoomIdentity.translate(tx, ty).scale(fitZoom);
    d3.select(ref.current).transition().duration(500).call(zoomRef.current.transform as any, t);
  }, [selectedNodeId]);

  return (
    <svg
      ref={ref}
      width="100%"
      height="100%"
      className="cursor-grab"
      style={{ minWidth: "800px", minHeight: "600px" }}
      onClick={() => clearHighlights()}
    >
      <g ref={gRef}>
        <rect
          x={bgRect.x}
          y={bgRect.y}
          width={bgRect.width}
          height={bgRect.height}
          fill="url(#dots)"
          className="pointer-events-none"
        />

        <g className="nodes">
          {visibleNodes.map((node) => (
            <NodeGroup
              key={node.id}
              node={node}
              moveChildWithConstraints={moveChildWithConstraints}
              layoutTransition={layoutTransition}
              selectNode={selectNode}
              setHoveredNode={setHoveredNode}
              getNodes={getNodes}
              interaction={interaction}
            />
          ))}
        </g>

        <defs>
          <pattern
            id="dots"
            x="0"
            y="0"
            width="20"
            height="20"
            patternUnits="userSpaceOnUse"
            patternTransform={`scale(${Math.pow(3, Math.max(0, Math.ceil(Math.log(0.75 / viewport.zoom) / Math.log(3))))})`}
          >
            <circle
              cx="10"
              cy="10"
              r="1.5"
              className="fill-gray-300 dark:fill-slate-600"
              opacity="0.4"
            />
          </pattern>

          <filter id="shadow" x="-20%" y="-20%" width="140%" height="140%">
            <feDropShadow
              dx="0"
              dy="1"
              stdDeviation="1"
              floodColor="rgba(16, 185, 129, 0.06)"
            />
          </filter>

          <marker id="arrow-default" viewBox="0 -5 10 10" refX="9" refY="0" markerWidth="6" markerHeight="6" orient="auto">
            <path d="M0,-5L10,0L0,5" className="fill-gray-500 dark:fill-slate-400" />
          </marker>
          <marker id="start-circle-default" viewBox="0 0 10 10" refX="5" refY="5" markerWidth="4" markerHeight="4" orient="auto">
            <circle cx="5" cy="5" r="3" className="fill-gray-500 dark:fill-slate-400" />
          </marker>

          <marker id="arrow-highlighted" viewBox="0 -5 10 10" refX="9" refY="0" markerWidth="6" markerHeight="6" orient="auto">
            <path d="M0,-5L10,0L0,5" className="fill-emerald-800 dark:fill-emerald-500" />
          </marker>
          <marker id="start-circle-highlighted" viewBox="0 0 10 10" refX="5" refY="5" markerWidth="4" markerHeight="4" orient="auto">
            <circle cx="5" cy="5" r="3" className="fill-emerald-800 dark:fill-emerald-500" />
          </marker>

          <marker id="arrow-dimmed" viewBox="0 -5 10 10" refX="9" refY="0" markerWidth="6" markerHeight="6" orient="auto">
            <path d="M0,-5L10,0L0,5" className="fill-gray-300 dark:fill-slate-600" />
          </marker>
          <marker id="start-circle-dimmed" viewBox="0 0 10 10" refX="5" refY="5" markerWidth="4" markerHeight="4" orient="auto">
            <circle cx="5" cy="5" r="3" className="fill-gray-300 dark:fill-slate-600" />
          </marker>

          <marker id="arrow-deleted" viewBox="0 -5 10 10" refX="9" refY="0" markerWidth="6" markerHeight="6" orient="auto">
            <path d="M0,-5L10,0L0,5" fill="#ef4444" />
          </marker>
          <marker id="start-circle-deleted" viewBox="0 0 10 10" refX="5" refY="5" markerWidth="4" markerHeight="4" orient="auto">
            <circle cx="5" cy="5" r="3" fill="#ef4444" />
          </marker>

          <marker id="arrow-added" viewBox="0 -5 10 10" refX="9" refY="0" markerWidth="6" markerHeight="6" orient="auto">
            <path d="M0,-5L10,0L0,5" fill="#0ea5e9" />
          </marker>
          <marker id="start-circle-added" viewBox="0 0 10 10" refX="5" refY="5" markerWidth="4" markerHeight="4" orient="auto">
            <circle cx="5" cy="5" r="3" fill="#0ea5e9" />
          </marker>

          <marker id="edit-arrow-default" viewBox="0 -5 10 10" refX="9" refY="0" markerWidth="6" markerHeight="6" orient="auto">
            <path d="M0,-5L10,0L0,5" className="fill-slate-500 dark:fill-slate-400" />
          </marker>
          <marker id="start-circle-edit-default" viewBox="0 0 10 10" refX="5" refY="5" markerWidth="4" markerHeight="4" orient="auto">
            <circle cx="5" cy="5" r="3" className="fill-slate-500 dark:fill-slate-400" />
          </marker>

          <marker id="edit-arrow-highlighted" viewBox="0 -5 10 10" refX="9" refY="0" markerWidth="6" markerHeight="6" orient="auto">
            <path d="M0,-5L10,0L0,5" className="fill-emerald-700 dark:fill-emerald-500" />
          </marker>
          <marker id="start-circle-edit-highlighted" viewBox="0 0 10 10" refX="5" refY="5" markerWidth="4" markerHeight="4" orient="auto">
            <circle cx="5" cy="5" r="3" className="fill-emerald-700 dark:fill-emerald-500" />
          </marker>

          <marker id="edit-arrow-dimmed" viewBox="0 -5 10 10" refX="9" refY="0" markerWidth="6" markerHeight="6" orient="auto">
            <path d="M0,-5L10,0L0,5" className="fill-slate-300 dark:fill-slate-600" />
          </marker>
          <marker id="start-circle-edit-dimmed" viewBox="0 0 10 10" refX="5" refY="5" markerWidth="4" markerHeight="4" orient="auto">
            <circle cx="5" cy="5" r="3" className="fill-slate-300 dark:fill-slate-600" />
          </marker>
        </defs>

        <g className="edges">
          {edges.map((edge) => (
            <Edge
              key={`${edge.source}-${edge.target}`}
              dataKey={`${edge.source}-${edge.target}`}
              edge={edge}
              nodes={nodes}
              selectedNodeId={selectedNodeId}
              edgeCurvature={edgeCurvature}
              edgeStrokeWidth={edgeStrokeWidth}
            />
          ))}
        </g>

        <g className="context-menus">
          {selectedNode && (
            <SvgNodeContextMenu
              nodeId={selectedNode.id}
              nodeX={selectedNode.x}
              nodeY={selectedNode.y}
              nodeWidth={selectedNode.width}
              hasChildren={!!(selectedNode.children && selectedNode.children.length > 0)}
              selectedNodeId={selectedNodeId}
              expandChildren={expandChildren}
              collapseChildren={collapseChildren}
              viewport={viewport}
            />
          )}
        </g>
      </g>
    </svg>
  );
}
