/**
 * PdgEdge — renders a control-flow edge as an SVG path.
 */

import { Node } from "@domains/graph/model/types";
import { EDGE_COLORS, EDGE_DIMMED, EDGE_HIGHLIGHTED } from "./pdgStyles";
import { generateBezierSVGPath } from "@domains/graph/lib/bezier";

interface Props {
  edge: {
    id: string;
    source: string;
    target: string;
    type: string;
    label?: string;
    highlighted?: boolean;
  };
  nodes: Node[];
  hasSelection: boolean;
  edgeCurvature: number;
  edgeStrokeWidth: number;
}

export default function PdgEdge({ edge, nodes, hasSelection, edgeCurvature, edgeStrokeWidth }: Props) {
  const src = nodes.find((n) => n.id === edge.source);
  const tgt = nodes.find((n) => n.id === edge.target);
  if (!src || !tgt) return null;

  // Generate bezier path with optimal connection points
  const path = generateBezierSVGPath(
    { id: src.id, type: "function", x: src.x, y: src.y, width: src.width, height: src.height, nodeShape: src.nodeShape },
    { id: tgt.id, type: "function", x: tgt.x, y: tgt.y, width: tgt.width, height: tgt.height, nodeShape: tgt.nodeShape },
    edgeCurvature,
  );

  const isHighlighted = edge.highlighted === true;
  const isDimmed = hasSelection && !isHighlighted;

  // Pick colors based on state
  let colors: { light: string; dark: string };
  if (isDimmed) {
    colors = EDGE_DIMMED;
  } else if (isHighlighted) {
    colors = EDGE_HIGHLIGHTED;
  } else {
    colors = EDGE_COLORS[edge.type] ?? EDGE_COLORS.fallthrough;
  }

  // Pick marker based on state
  let marker: string;
  if (isDimmed) {
    marker = "url(#cfg-arrow-dimmed)";
  } else if (isHighlighted) {
    marker = "url(#cfg-arrow-highlighted)";
  } else if (edge.type === "true_branch") {
    marker = "url(#cfg-arrow-green)";
  } else if (edge.type === "false_branch") {
    marker = "url(#cfg-arrow-red)";
  } else {
    marker = "url(#cfg-arrow)";
  }

  const isDashed =
    edge.type === "cross_scope" ||
    edge.type === "binding" ||
    edge.type === "receiver" ||
    edge.type === "data";

  const dashArray = edge.type === "data" ? "4,2" : "5,3";

  return (
    <g opacity={isDimmed ? 0.3 : 1}>
      {/* Light-mode path */}
      <path
        d={path}
        fill="none"
        stroke={colors.light}
        strokeWidth={isHighlighted ? edgeStrokeWidth * 1.5 : edgeStrokeWidth}
        strokeDasharray={isDashed ? dashArray : undefined}
        markerEnd={marker}
        opacity={1}
        className="dark:hidden"
      />
      {/* Dark-mode path */}
      <path
        d={path}
        fill="none"
        stroke={colors.dark}
        strokeWidth={isHighlighted ? edgeStrokeWidth * 1.5 : edgeStrokeWidth}
        strokeDasharray={isDashed ? dashArray : undefined}
        markerEnd={marker}
        opacity={1}
        className="hidden dark:block"
      />
    </g>
  );
}
