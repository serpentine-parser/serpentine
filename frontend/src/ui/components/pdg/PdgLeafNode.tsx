/**
 * PdgLeafNode — non-scope PDG nodes:
 *   condition  → parallelogram (emerald border)
 *   call       → rounded rect (cyan accent)
 *   variable   → rounded rect (slate)
 *   literal    → rounded rect (cyan accent)
 *   statement  → rounded rect (slate)
 *   merge      → tiny dot
 *
 * Supports: hover, highlight/dim state, mode-aware colors.
 */

import { Node } from "@domains/graph/model/types";
import { getNodeClasses } from "../viewNodes/nodeClasses";
import { LEAF_STYLES } from "./pdgStyles";

interface Props {
  node: Node;
  onClick: (id: string) => void;
  onHover: (id: string | null) => void;
  selected: boolean;
  hovered: boolean;
  hasSelection: boolean;
}

export default function PdgLeafNode({
  node,
  onClick,
  onHover,
  selected,
  hovered,
  hasSelection,
}: Props) {
  const style =
    LEAF_STYLES[node.nodeShape || "statement"] ?? LEAF_STYLES.statement;
  const isHighlighted = node.highlighted === "path";
  const isDimmed = hasSelection && !selected && !isHighlighted;

  // Centralized classes
  const classes = getNodeClasses();

  // Base uses the centralized leaf class; append shape-specific stroke/text accents
  // so the overall palette stays consistent while preserving shape identity.
  let fill = classes.leaf; // keep base fill from centralized class
  let stroke = classes.leaf;
  let text = classes.leafText;

  // Only use shape-specific stroke and text; avoid shape fill to keep base palette
  if (style.stroke) stroke = `${stroke} ${style.stroke}`;
  if (style.text) text = `${text} ${style.text}`;

  if (selected) {
    fill = classes.leafSelected;
    stroke = classes.leafSelected;
    text = classes.leafText;
  } else if (isDimmed) {
    fill = classes.leafDimmed;
    stroke = classes.leafDimmed;
    text = classes.leafText;
  } else if (isHighlighted) {
    fill = classes.leafHighlighted;
    stroke = classes.leafHighlighted;
    text = classes.leafText;
  } else if (hovered) {
    // Slight hover tint using the shared hover class
    fill = `${fill} ${classes.containerHover}`;
  }

  // ── Merge (tiny dot) ──
  if (node.nodeShape === "merge") {
    return (
      <circle
        cx={node.x + node.width / 2}
        cy={node.y + node.height / 2}
        r={3}
        className={selected ? style.selectedFill : style.fill}
        opacity={isDimmed ? 0.3 : 0.5}
      />
    );
  }

  // ── Condition (parallelogram) ──
  if (node.nodeShape === "condition") {
    return (
      <ConditionNode
        node={node}
        onClick={onClick}
        onHover={onHover}
        selected={selected}
        hovered={hovered}
        fill={fill}
        stroke={stroke}
        text={text}
        isDimmed={isDimmed}
      />
    );
  }

  // ── Everything else: rounded rect ──
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
      <rect
        x={node.x}
        y={node.y}
        width={node.width}
        height={node.height}
        rx={6}
        ry={6}
        className={`${fill} ${stroke}`}
        strokeWidth={selected || hovered ? 1.8 : 1.2}
        style={selected ? { filter: "url(#shadow)" } : undefined}
      />
      <text
        x={node.x + node.width / 2}
        y={node.y + node.height / 2 + 4}
        textAnchor="middle"
        fontSize={11}
        fontFamily="'Fira Code', monospace"
        className={text}
      >
        {node.label}
      </text>
    </g>
  );
}

// ── Parallelogram for conditions ────────────────────────

function ConditionNode({
  node,
  onClick,
  onHover,
  selected,
  hovered,
  fill,
  stroke,
  text,
  isDimmed,
}: {
  node: Node;
  onClick: (id: string) => void;
  onHover: (id: string | null) => void;
  selected: boolean;
  hovered: boolean;
  fill: string;
  stroke: string;
  text: string;
  isDimmed: boolean;
}) {
  const { x, y, width: w, height: h } = node;
  const skew = h * 0.35;
  const pts = [
    `${x + skew},${y}`,
    `${x + w},${y}`,
    `${x + w - skew},${y + h}`,
    `${x},${y + h}`,
  ].join(" ");

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
      <polygon
        points={pts}
        className={`${fill} ${stroke}`}
        strokeWidth={selected || hovered ? 1.8 : 1.2}
      />
      <text
        x={x + w / 2}
        y={y + h / 2 + 4}
        textAnchor="middle"
        fontSize={11}
        fontFamily="'Fira Code', monospace"
        className={text}
      >
        {node.label}
      </text>
    </g>
  );
}
