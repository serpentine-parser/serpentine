/**
 * PDG visual styling — Tailwind className strings with light/dark support.
 *
 * Matches the dependency graph's monochrome emerald palette:
 *   light → emerald/white backgrounds, dark borders
 *   dark  → slate-800/900 backgrounds, emerald borders
 *
 * Every value is a Tailwind className string, not a hex color.
 */

import {
  EDGE_COLORS,
  EDGE_DIMMED,
  EDGE_HIGHLIGHTED,
  MARKER_COLORS,
} from "@ui/lib/graphTheme";
import { getNodeClasses } from "../viewNodes/nodeClasses";

export { EDGE_COLORS, EDGE_DIMMED, EDGE_HIGHLIGHTED, MARKER_COLORS };

// Derive scope container classes from the centralized node classes so PDG and
// dependency graph visuals stay consistent. This centralizes color decisions
// in one place (`nodeClasses.getNodeClasses`) making future adjustments easy.
const NODE_CLASSES = getNodeClasses();

// ── Scope containers (by nesting depth) ─────────────────
// For now we keep the same classes across depths, but this is a single place
// to add depth-based variants later if desired.
export const SCOPE_CLASSES: Record<
  number,
  { container: string; header: string; label: string }
> = {
  0: {
    container: NODE_CLASSES.container,
    header: NODE_CLASSES.header,
    label: NODE_CLASSES.titleText,
  },
  1: {
    container: NODE_CLASSES.container,
    header: NODE_CLASSES.header,
    label: NODE_CLASSES.titleText,
  },
  2: {
    container: NODE_CLASSES.container,
    header: NODE_CLASSES.header,
    label: NODE_CLASSES.titleText,
  },
  3: {
    container: NODE_CLASSES.container,
    header: NODE_CLASSES.header,
    label: NODE_CLASSES.titleText,
  },
};

export const SCOPE_SELECTED = {
  container: NODE_CLASSES.containerSelected,
  label: NODE_CLASSES.titleText,
};

// ── Leaf node shapes ────────────────────────────────────

interface LeafStyle {
  fill: string;
  stroke: string;
  text: string;
  selectedFill: string;
  selectedStroke: string;
}

export const LEAF_STYLES: Record<string, LeafStyle> = {
  condition: {
    fill: NODE_CLASSES.leaf,
    stroke: "",
    text: "",
    selectedFill: NODE_CLASSES.leafSelected,
    selectedStroke: "",
  },
  call: {
    fill: NODE_CLASSES.leaf,
    stroke: "",
    text: "",
    selectedFill: NODE_CLASSES.leafSelected,
    selectedStroke: "",
  },
  statement: {
    fill: NODE_CLASSES.leaf,
    stroke: "",
    text: "",
    selectedFill: NODE_CLASSES.leafSelected,
    selectedStroke: "",
  },
  variable: {
    fill: NODE_CLASSES.leaf,
    stroke: "",
    text: "",
    selectedFill: NODE_CLASSES.leafSelected,
    selectedStroke: "",
  },
  literal: {
    fill: NODE_CLASSES.leaf,
    stroke: "stroke-cyan-400 dark:stroke-cyan-500",
    text: "fill-cyan-200",
    selectedFill: NODE_CLASSES.leafSelected,
    selectedStroke: "",
  },
  merge: {
    fill: "fill-emerald-400 dark:fill-emerald-600",
    stroke: "",
    text: "",
    selectedFill: NODE_CLASSES.leafHighlighted,
    selectedStroke: "",
  },
};

// ── Highlight / dim state overrides ─────────────────────

export const HIGHLIGHT_OVERRIDES = {
  scope: {
    container: NODE_CLASSES.containerHighlighted,
    label: NODE_CLASSES.titleText,
  },
  leaf: {
    fill: NODE_CLASSES.containerHighlighted,
    stroke: NODE_CLASSES.containerHighlighted,
    text: NODE_CLASSES.titleText,
  },
};

export const DIM_OVERRIDES = {
  scope: {
    container: NODE_CLASSES.containerDimmed,
    label: NODE_CLASSES.titleText,
  },
  leaf: {
    fill: NODE_CLASSES.containerDimmed,
    stroke: NODE_CLASSES.containerDimmed,
    text: NODE_CLASSES.titleText,
  },
};

// ── Dimmed edge colors ──────────────────────────────────

// Edge theme values are now sourced from graphTheme.
