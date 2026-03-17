/**
 * Shared edge rendering utilities for both D3/SVG and React Flow
 * Provides consistent bezier curves, stroke width, and arrow styling
 */

export interface EdgePoint {
  x: number;
  y: number;
  side: "top" | "right" | "bottom" | "left";
}

export interface EdgeBezierConfig {
  curvature: number;
  strokeWidth: number;
  arrowSize: number;
}

export const EDGE_BEZIER_CONFIG: EdgeBezierConfig = {
  curvature: 40,
  strokeWidth: 2,
  arrowSize: 8,
};

export function getNodeSideCenters(node: {
  x: number;
  y: number;
  width: number;
  height: number;
}): EdgePoint[] {
  return [
    { x: node.x + node.width / 2, y: node.y, side: "top" },
    { x: node.x + node.width, y: node.y + node.height / 2, side: "right" },
    { x: node.x + node.width / 2, y: node.y + node.height, side: "bottom" },
    { x: node.x, y: node.y + node.height / 2, side: "left" },
  ];
}

export function getNodeSideCentersForShape(
  node: { x: number; y: number; width: number; height: number },
  nodeShape?: string
): EdgePoint[] {
  const allSides = getNodeSideCenters(node);
  if (nodeShape === "condition") {
    return [allSides[0], allSides[2]];
  }
  return allSides;
}

export function getOptimalConnectionPoints(
  sourceNode: { x: number; y: number; width: number; height: number },
  targetNode: { x: number; y: number; width: number; height: number },
  sourceShape?: string,
  targetShape?: string
): { source: EdgePoint; target: EdgePoint } {
  let best: { source: EdgePoint; target: EdgePoint } | null = null;
  let minDistance = Infinity;

  const sourceCenters = getNodeSideCentersForShape(sourceNode, sourceShape);
  const targetCenters = getNodeSideCentersForShape(targetNode, targetShape);

  sourceCenters.forEach((sourcePort) => {
    targetCenters.forEach((targetPort) => {
      const distance =
        (targetPort.x - sourcePort.x) ** 2 + (targetPort.y - sourcePort.y) ** 2;
      if (distance < minDistance) {
        minDistance = distance;
        best = { source: sourcePort, target: targetPort };
      }
    });
  });

  return best!;
}

export function calculateBezierControlPoints(
  source: EdgePoint,
  target: EdgePoint,
  curvature: number = EDGE_BEZIER_CONFIG.curvature
): { c1: { x: number; y: number }; c2: { x: number; y: number } } {
  const c1 = { ...source };
  const c2 = { ...target };

  const adjustControlPoint = (pt: { x: number; y: number }, side: string) => {
    if (side === "left") pt.x -= curvature;
    if (side === "right") pt.x += curvature;
    if (side === "top") pt.y -= curvature;
    if (side === "bottom") pt.y += curvature;
  };

  adjustControlPoint(c1, source.side);
  adjustControlPoint(c2, target.side);

  return { c1, c2 };
}

export function generateBezierSVGPath(
  sourceNode: {
    id: string;
    type?: string;
    x: number;
    y: number;
    width: number;
    height: number;
    children?: any[];
    nodeShape?: string;
  },
  targetNode: {
    id: string;
    type?: string;
    x: number;
    y: number;
    width: number;
    height: number;
    children?: any[];
    nodeShape?: string;
  },
  curvature: number = EDGE_BEZIER_CONFIG.curvature
): string {
  const { source, target } = getOptimalConnectionPoints(
    sourceNode,
    targetNode,
    sourceNode.nodeShape,
    targetNode.nodeShape
  );
  const { c1, c2 } = calculateBezierControlPoints(source, target, curvature);

  return `M${source.x},${source.y} C${c1.x},${c1.y} ${c2.x},${c2.y} ${target.x},${target.y}`;
}
