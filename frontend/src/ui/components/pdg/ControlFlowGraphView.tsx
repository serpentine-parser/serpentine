/**
 * ControlFlowGraphView — CFG visualization, props-in pattern
 */

import * as d3 from "d3";
import React, { useCallback, useEffect, useRef, useState } from "react";
import type { Node, Viewport } from "@domains/graph/model/types";
import type { CfgEdgeData } from "@domains/graph/model/types";
import PdgEdge from "./PdgEdge";
import PdgLeafNode from "./PdgLeafNode";
import PdgMarkerDefs from "./PdgMarkerDefs";
import PdgScopeNode from "./PdgScopeNode";

interface ControlFlowGraphViewProps {
  selectedNodeId: string | null;
  hoveredNodeId: string | null;
  viewport: Viewport;
  selectNode: (id: string | null) => void;
  selectPdgNode: (id: string) => void;
  setHoveredNode: (id: string | null) => void;
  clearHighlights: () => void;
  setViewport: (v: Viewport) => void;
  setSvgRef: (ref: React.RefObject<SVGSVGElement> | null) => void;
  visualizationMode: string;
  pdgFlatNodes: Node[];
  pdgEdges: CfgEdgeData[];
  isPdgLayoutLoading: boolean;
  togglePdgNodeCollapse: (id: string) => void;
  getPdgNodes: () => Node[];
  movePdgNodeWithConstraints: (id: string, parent: string | null, x: number, y: number) => void;
  edgeCurvature: number;
  edgeStrokeWidth: number;
}

export default function ControlFlowGraphView({
  selectedNodeId,
  hoveredNodeId,
  viewport,
  selectNode,
  selectPdgNode,
  setHoveredNode,
  clearHighlights,
  setViewport,
  setSvgRef,
  visualizationMode,
  pdgFlatNodes,
  pdgEdges,
  isPdgLayoutLoading,
  togglePdgNodeCollapse,
  getPdgNodes,
  movePdgNodeWithConstraints,
  edgeCurvature,
  edgeStrokeWidth,
}: ControlFlowGraphViewProps) {
  const svgRef = useRef<SVGSVGElement>(null);
  const gRef = useRef<SVGGElement>(null);
  const zoomRef = useRef<d3.ZoomBehavior<SVGSVGElement, unknown> | null>(null);
  const [layoutApplied, setLayoutApplied] = useState(false);

  useEffect(() => {
    if (!isPdgLayoutLoading) {
      setLayoutApplied(true);
    }
  }, [isPdgLayoutLoading]);

  useEffect(() => {
    setSvgRef(svgRef as React.RefObject<SVGSVGElement>);
    return () => setSvgRef(null);
  }, [setSvgRef]);

  useEffect(() => {
    if (!svgRef.current || !gRef.current) return;
    const svg = d3.select(svgRef.current);
    const g = d3.select(gRef.current);

    const zoom = d3
      .zoom<SVGSVGElement, unknown>()
      .scaleExtent([0.1, 3])
      .on("zoom", (e) => {
        g.attr("transform", e.transform);
        setViewport({
          x: e.transform.x,
          y: e.transform.y,
          zoom: e.transform.k,
        });
      });
    svg.call(zoom);
    zoomRef.current = zoom;

    const isInitialLoad =
      viewport.x === 0 && viewport.y === 0 && viewport.zoom === 1;

    if (isInitialLoad && pdgFlatNodes.length > 0) {
      const pad = 60;
      let x0 = Infinity,
        y0 = Infinity,
        x1 = -Infinity,
        y1 = -Infinity;
      for (const n of pdgFlatNodes) {
        x0 = Math.min(x0, n.x);
        y0 = Math.min(y0, n.y);
        x1 = Math.max(x1, n.x + n.width);
        y1 = Math.max(y1, n.y + n.height);
      }
      const bw = x1 - x0 + pad * 2;
      const bh = y1 - y0 + pad * 2;
      const el = svgRef.current;
      const vw = el.clientWidth || 800;
      const vh = el.clientHeight || 600;
      const sc = Math.min(vw / bw, vh / bh, 1);
      const tx = (vw - bw * sc) / 2 - x0 * sc + pad * sc;
      const ty = (vh - bh * sc) / 2 - y0 * sc + pad * sc;
      svg.call(zoom.transform, d3.zoomIdentity.translate(tx, ty).scale(sc));
    } else if (!isInitialLoad) {
      const t = d3.zoomIdentity
        .translate(viewport.x, viewport.y)
        .scale(viewport.zoom);
      svg.call(zoom.transform, t);
    }

    return () => {
      svg.on(".zoom", null);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pdgFlatNodes]);

  useEffect(() => {
    if (!selectedNodeId || isPdgLayoutLoading || !svgRef.current || !zoomRef.current) return;

    const node = pdgFlatNodes.find((n) => n.id === selectedNodeId);
    if (!node) return;

    const el = svgRef.current;
    const vw = el.clientWidth || 800;
    const vh = el.clientHeight || 600;
    const svg = d3.select(svgRef.current);
    const zoom = zoomRef.current;

    const pad = 80;
    const fitZoom = Math.min(
      vw / (node.width + pad * 2),
      vh / (node.height + pad * 2),
      1,
    );
    const tx = vw / 2 - (node.x + node.width / 2) * fitZoom;
    const ty = vh / 2 - (node.y + node.height / 2) * fitZoom;
    const t = d3.zoomIdentity.translate(tx, ty).scale(fitZoom);

    svg.transition().duration(500).call(zoom.transform, t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedNodeId, isPdgLayoutLoading]);

  useEffect(() => {
    if (!gRef.current) return;
    const t = d3.zoomIdentity
      .translate(viewport.x, viewport.y)
      .scale(viewport.zoom);
    d3.select(gRef.current).attr("transform", t.toString());
  }, [viewport.x, viewport.y, viewport.zoom]);

  const handleNodeClick = useCallback(
    (id: string) => {
      if (visualizationMode === "control-flow") selectPdgNode(id);
      else selectNode(selectedNodeId === id ? null : id);
    },
    [selectNode, selectPdgNode, selectedNodeId, visualizationMode],
  );

  const handleNodeHover = useCallback(
    (id: string | null) => setHoveredNode(id),
    [setHoveredNode],
  );

  const handleToggleCollapse = useCallback(
    (id: string) => {
      togglePdgNodeCollapse(id);
    },
    [togglePdgNodeCollapse],
  );

  useEffect(() => {
    if (!gRef.current) return;

    const drag = d3
      .drag<SVGGElement, unknown>()
      .clickDistance(4)
      .on("start", function () {
        const g = d3.select(this);
        g.style("opacity", "0.8");
      })
      .on("drag", function (e) {
        const g = d3.select(this);
        const nodeId = g.attr("data-node-id");
        if (!nodeId) return;

        const currentFlatNodes = getPdgNodes();
        const draggedNode = currentFlatNodes.find((n) => n.id === nodeId);
        if (!draggedNode) return;

        movePdgNodeWithConstraints(
          nodeId,
          draggedNode.parent || null,
          draggedNode.x + e.dx,
          draggedNode.y + e.dy,
        );
      })
      .on("end", function () {
        const g = d3.select(this);
        g.style("opacity", "1");
      });

    d3.select(gRef.current)
      .selectAll<SVGGElement, unknown>("g[data-node-id]")
      .call(drag as any);

    return () => {
      if (gRef.current) {
        d3.select(gRef.current)
          .selectAll<SVGGElement, unknown>("g[data-node-id]")
          .on(".drag", null);
      }
    };
  }, [pdgFlatNodes, getPdgNodes, movePdgNodeWithConstraints]);

  const visibleNodes = pdgFlatNodes;

  if (visualizationMode === "control-flow" && !selectedNodeId) {
    return (
      <div className="flex h-full items-center justify-center bg-gray-50 dark:bg-gray-950 text-gray-500 dark:text-gray-400">
        <div className="text-center">
          <p className="text-sm">Select a node in the Object Explorer to view its control flow.</p>
        </div>
      </div>
    );
  }

  if (isPdgLayoutLoading || !layoutApplied) {
    return (
      <div className="flex h-full items-center justify-center bg-gray-50 dark:bg-gray-950 text-gray-500 dark:text-gray-400">
        <div className="animate-pulse text-lg">
          Building control-flow graph…
        </div>
      </div>
    );
  }

  if (pdgFlatNodes.length === 0) {
    return (
      <div className="flex h-full items-center justify-center bg-gray-50 dark:bg-gray-950 text-gray-500 dark:text-gray-400">
        No control-flow data available.
      </div>
    );
  }

  const hasSelection = visualizationMode === "control-flow"
    ? pdgFlatNodes.some((n) => n.highlighted === "path")
    : selectedNodeId !== null;
  const visibleIds = new Set(visibleNodes.map((n) => n.id));
  const visibleEdges = pdgEdges.filter(
    (e) => visibleIds.has(e.source) && visibleIds.has(e.target),
  );

  return (
    <div className="h-full w-full bg-gray-50 dark:bg-gray-950">
      <svg
        ref={svgRef}
        className="h-full w-full"
        style={{ cursor: "grab" }}
        onClick={() => clearHighlights()}
      >
        <PdgMarkerDefs />

        <g ref={gRef}>
          <defs>
            <pattern
              id="cfg-dots"
              x="0"
              y="0"
              width="20"
              height="20"
              patternUnits="userSpaceOnUse"
            >
              <circle
                cx="10"
                cy="10"
                r="1.5"
                className="fill-gray-300 dark:fill-slate-600"
                opacity="0.4"
              />
            </pattern>
          </defs>
          <rect
            x="-10000"
            y="-10000"
            width="20000"
            height="20000"
            fill="url(#cfg-dots)"
            className="pointer-events-none"
          />

          <filter id="shadow" x="-20%" y="-20%" width="140%" height="140%">
            <feDropShadow
              dx="0"
              dy="1"
              stdDeviation="1"
              floodColor="rgba(16, 185, 129, 0.06)"
            />
          </filter>

          {visibleNodes
            .filter((n) => n.isScope)
            .map((n) => (
              <g key={n.id} data-node-id={n.id}>
                <PdgScopeNode
                  node={n}
                  onClick={handleNodeClick}
                  onHover={handleNodeHover}
                  onToggleCollapse={handleToggleCollapse}
                  selected={selectedNodeId === n.id}
                  hovered={hoveredNodeId === n.id}
                  hasSelection={hasSelection}
                />
              </g>
            ))}

          {visibleNodes
            .filter((n) => !n.isScope)
            .map((n) => (
              <g key={n.id} data-node-id={n.id}>
                <PdgLeafNode
                  node={n}
                  onClick={handleNodeClick}
                  onHover={handleNodeHover}
                  selected={selectedNodeId === n.id}
                  hovered={hoveredNodeId === n.id}
                  hasSelection={hasSelection}
                />
              </g>
            ))}

          {visibleEdges.map((e) => (
            <PdgEdge
              key={e.id}
              edge={e}
              nodes={visibleNodes}
              hasSelection={hasSelection}
              edgeCurvature={edgeCurvature}
              edgeStrokeWidth={edgeStrokeWidth}
            />
          ))}
        </g>
      </svg>
    </div>
  );
}
