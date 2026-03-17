/**
 * PdgScopeNode — scope container (module, function, class).
 *
 * Rounded-rect with a tinted header band and dashed border.
 * Supports: collapse toggle, hover, highlight/dim state, mode-aware colors.
 */

import { Node } from "@domains/graph/model/types";
import { getNodeClasses } from "../viewNodes/nodeClasses";
import {
  DIM_OVERRIDES,
  HIGHLIGHT_OVERRIDES,
  SCOPE_CLASSES,
  SCOPE_SELECTED,
} from "./pdgStyles";

interface Props {
  node: Node;
  onClick: (id: string) => void;
  onHover: (id: string | null) => void;
  onToggleCollapse: (id: string) => void;
  selected: boolean;
  hovered: boolean;
  hasSelection: boolean;
}

export default function PdgScopeNode({
  node,
  onClick,
  onHover,
  onToggleCollapse,
  selected,
  hovered,
  hasSelection,
}: Props) {
  const d = 0; // PDG nodes don't use depth the same way - use constant
  const cls = SCOPE_CLASSES[d] ?? SCOPE_CLASSES[3];
  const headerH = 28;

  const nodeClasses = getNodeClasses();
  // Hover class from shared node classes
  const hoverClass = nodeClasses.containerHover || "";

  // Determine visual state
  const isHighlighted = node.highlighted === "path";
  const isDimmed = hasSelection && !selected && !isHighlighted;

  let containerCls = cls.container;
  let labelCls = cls.label;

  if (selected) {
    containerCls = SCOPE_SELECTED.container;
    labelCls = SCOPE_SELECTED.label;
  } else if (isDimmed) {
    containerCls = DIM_OVERRIDES.scope.container;
    labelCls = DIM_OVERRIDES.scope.label;
  } else if (isHighlighted) {
    containerCls = HIGHLIGHT_OVERRIDES.scope.container;
    labelCls = HIGHLIGHT_OVERRIDES.scope.label;
  }

  const isCollapsed = node.collapsed ?? false;

  return (
    <g
      onClick={(e) => {
        e.stopPropagation();
        onClick(node.id);
      }}
      onMouseEnter={() => onHover(node.id)}
      onMouseLeave={() => onHover(null)}
      style={{ cursor: "pointer" }}
      opacity={isDimmed ? 0.4 : 1}
    >
      {/* Container */}
      <rect
        x={node.x}
        y={node.y}
        width={node.width}
        height={node.height}
        rx={8}
        ry={8}
        className={`${containerCls} ${hovered && !selected && !isHighlighted && !isDimmed ? hoverClass : ""}`}
        strokeWidth={selected || hovered ? 1.8 : 1.2}
        strokeDasharray={selected ? undefined : "6 3"}
        style={selected ? { filter: "url(#shadow)" } : undefined}
      />

      {/* Header band */}
      <rect
        x={node.x}
        y={node.y}
        width={node.width}
        height={headerH}
        rx={8}
        ry={8}
        className={cls.header}
      />
      {/* Square-off the bottom corners of the header */}
      <rect
        x={node.x}
        y={node.y + headerH - 8}
        width={node.width}
        height={8}
        className={cls.header}
      />

      {/* Collapse toggle */}
      <rect
        x={node.x + node.width - 25}
        y={node.y + 5}
        width={20}
        height={20}
        className={nodeClasses.toggleButton}
        strokeWidth={1}
        rx={3}
        onClick={(e) => {
          e.stopPropagation();
          onToggleCollapse(node.id);
        }}
        style={{ cursor: "pointer" }}
      />
      <text
        x={node.x + node.width - 15}
        y={node.y + 18}
        textAnchor="middle"
        className={nodeClasses.toggleText}
        fontSize={12}
        fontWeight="bold"
        style={{ pointerEvents: "none" }}
      >
        {isCollapsed ? "+" : "−"}
      </text>

      {/* Label */}
      <text
        x={node.x + 10}
        y={node.y + 18}
        fontSize={12}
        fontWeight={600}
        fontFamily="'Fira Code', monospace"
        className={labelCls}
      >
        {node.label}
      </text>
    </g>
  );
}
