import { useMemo } from "react";
import { Node } from "@domains/graph/model/types";
import { generateBezierSVGPath } from "@domains/graph/lib/bezier";
import { getEdgeState } from "@ui/lib/nodeStyles";

export type EdgeData = {
  source: string;
  target: string;
  type: "calls" | "is-a" | "has-a";
  changeStatus?: "added" | "deleted" | null;
};

type Props = {
  dataKey: string;
  edge: EdgeData;
  nodes: Node[];
  selectedNodeId: string | null;
  edgeCurvature: number;
  edgeStrokeWidth: number;
};

type NodeOrClass = Node & { type: "module" | "class" | "function" };

function findAnyNodeById(nodes: Node[], id: string): NodeOrClass | null {
  const module = nodes.find((n) => n.id === id);
  if (module) {
    return { ...module, type: "module" };
  }

  const searchInChildren = (children: Node[]): NodeOrClass | null => {
    for (const child of children) {
      if (child.id === id) {
        return { ...child, type: child.type as "class" | "function" };
      }
      if (child.children && child.children.length > 0) {
        const found = searchInChildren(child.children);
        if (found) return found;
      }
    }
    return null;
  };

  for (const node of nodes) {
    if (node.children && node.children.length > 0) {
      const found = searchInChildren(node.children);
      if (found) return found;
    }
  }

  return null;
}

const Edge = ({ dataKey, edge, nodes, selectedNodeId, edgeCurvature, edgeStrokeWidth }: Props) => {
  const modeColorPrimary = "emerald";

  const pathData = useMemo(() => {
    const sourceNode = findAnyNodeById(nodes, edge.source);
    const targetNode = findAnyNodeById(nodes, edge.target);

    if (!sourceNode || !targetNode) {
      return "";
    }

    const sourceNodeForEdge = {
      id: sourceNode.id,
      type: sourceNode.type,
      x: sourceNode.x,
      y: sourceNode.y,
      width: sourceNode.width,
      height: sourceNode.height,
      children: sourceNode.children,
    };

    const targetNodeForEdge = {
      id: targetNode.id,
      type: targetNode.type,
      x: targetNode.x,
      y: targetNode.y,
      width: targetNode.width,
      height: targetNode.height,
      children: targetNode.children,
    };

    return generateBezierSVGPath(sourceNodeForEdge, targetNodeForEdge, edgeCurvature);
  }, [nodes, edge.source, edge.target, edgeCurvature]);

  const edgeState = useMemo(() => {
    const sourceNode = findAnyNodeById(nodes, edge.source);
    const targetNode = findAnyNodeById(nodes, edge.target);

    if (!sourceNode || !targetNode) {
      return "default";
    }

    const isNodeOrDescendantsHighlighted = (node: NodeOrClass): boolean => {
      if (node.highlighted) return true;
      if (node.children) {
        return node.children.some((child) =>
          isNodeOrDescendantsHighlighted(child as NodeOrClass),
        );
      }
      return false;
    };

    const sourceHighlighted =
      isNodeOrDescendantsHighlighted(sourceNode) ||
      sourceNode.id === selectedNodeId;
    const targetHighlighted =
      isNodeOrDescendantsHighlighted(targetNode) ||
      targetNode.id === selectedNodeId;
    const hasSelection = selectedNodeId !== null;

    return getEdgeState(sourceHighlighted, targetHighlighted, hasSelection);
  }, [nodes, edge.source, edge.target, selectedNodeId]);

  const changeEdgeColor = useMemo(() => {
    if (edge.changeStatus === "deleted") return "#ef4444";
    if (edge.changeStatus === "added") return "#0ea5e9";
    return null;
  }, [edge.changeStatus]);

  const getEdgeClasses = () => {
    let classes = "fill-none";

    if (changeEdgeColor) {
      if (edgeState === "highlighted") {
        classes += ` stroke-${modeColorPrimary}-800 dark:stroke-${modeColorPrimary}-500`;
      } else {
        classes += " opacity-80";
      }
    } else if (edgeState === "highlighted") {
      classes += ` stroke-${modeColorPrimary}-800 dark:stroke-${modeColorPrimary}-500`;
    } else if (edgeState === "dimmed") {
      classes += " stroke-gray-300 dark:stroke-slate-600 opacity-0 dark:opacity-30";
    } else {
      classes += " stroke-gray-500 dark:stroke-slate-400 opacity-75";
    }

    if (edge.type === "is-a") {
      classes += " stroke-dashed";
    } else if (edge.type === "has-a") {
      classes += " stroke-dotted";
    }

    return classes;
  };

  if (!pathData) return null;

  const getMarkers = () => {
    if (edge.changeStatus === "deleted") return { markerStart: "url(#start-circle-deleted)", markerEnd: "url(#arrow-deleted)" };
    if (edge.changeStatus === "added") return { markerStart: "url(#start-circle-added)", markerEnd: "url(#arrow-added)" };
    return {
      markerStart: `url(#start-circle-${edgeState})`,
      markerEnd: `url(#arrow-${edgeState})`,
    };
  };

  const markers = getMarkers();

  return (
    <path
      d={pathData}
      key={dataKey}
      data-key={dataKey}
      className={getEdgeClasses()}
      markerStart={markers.markerStart}
      markerEnd={markers.markerEnd}
      style={{
        strokeWidth: edgeStrokeWidth,
        strokeDasharray:
          edge.type === "is-a" ? "5,5" : edge.type === "has-a" ? "2,3" : "none",
        ...(changeEdgeColor && edgeState !== "highlighted"
          ? { stroke: changeEdgeColor }
          : {}),
      }}
    />
  );
};

export default Edge;
