/**
 * Shared theme configuration for both dependency graph and CFG views.
 * Single source of truth for palette, mode-aware colors, and state classes.
 */

export type GraphMode = "view" | "edit";

export const PALETTE = {
  view: {
    primary: "emerald",
    secondary: "teal",
  },
  edit: {
    primary: "cyan",
    secondary: "sky",
  },
} as const;

export function getModeColors(mode: GraphMode) {
  return PALETTE[mode];
}

/** Background / border classes for chrome (toolbar, search bar) */
export function getChromeClasses(mode: GraphMode) {
  const p = PALETTE[mode];
  return {
    bg: `bg-${p.primary}-200/30 dark:bg-${p.primary}-900/30`,
    border: `border-${p.primary}-200/30 dark:border-${p.primary}-700`,
    button: `bg-${p.primary}-900 dark:bg-${p.primary}-800`,
    icon: `text-${p.primary}-400 dark:text-${p.primary}-300`,
  };
}

/** Highlight state shared across both views */
export type HighlightState = "upstream" | "downstream" | "path" | null;

export type EdgeColor = { light: string; dark: string };

// Shared edge colors for graph rendering (CFG + dependency graph)
export const EDGE_COLORS: Record<string, EdgeColor> = {
  true_branch: { light: "#16a34a", dark: "#22c55e" },
  false_branch: { light: "#dc2626", dark: "#ef4444" },
  control: { light: "#6b7280", dark: "#94a3b8" },
  fallthrough: { light: "#6b7280", dark: "#94a3b8" },
  data_flow: { light: "#6b7280", dark: "#94a3b8" },
  exception: { light: "#ea580c", dark: "#f97316" },
  return: { light: "#7c3aed", dark: "#8b5cf6" },
  cross_scope: { light: "#059669", dark: "#10b981" },
  binding: { light: "#0891b2", dark: "#06b6d4" },
  receiver: { light: "#0891b2", dark: "#06b6d4" },
  scope_exit: { light: "#6b7280", dark: "#94a3b8" },
  calls: { light: "#059669", dark: "#10b981" },
  uses: { light: "#0891b2", dark: "#06b6d4" },
  "is-a": { light: "#7c3aed", dark: "#8b5cf6" },
  "has-a": { light: "#0891b2", dark: "#06b6d4" },
  data: { light: "#0891b2", dark: "#06b6d4" },
};

// Marker arrow colors (need separate defs per theme)
export const MARKER_COLORS: Record<string, EdgeColor> = {
  default: { light: "#6b7280", dark: "#94a3b8" },
  green: { light: "#16a34a", dark: "#22c55e" },
  red: { light: "#dc2626", dark: "#ef4444" },
  highlighted: { light: "#059669", dark: "#10b981" },
  dimmed: { light: "#d1d5db", dark: "#475569" },
};

export const EDGE_DIMMED: EdgeColor = { light: "#d1d5db", dark: "#475569" };
export const EDGE_HIGHLIGHTED: EdgeColor = {
  light: "#059669",
  dark: "#10b981",
};
