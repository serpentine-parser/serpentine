/**
 * PdgMarkerDefs — SVG <defs> for PDG arrow markers.
 *
 * Two sets: one for light mode, one for dark mode, toggled via
 * Tailwind's dark: variant on a wrapping <g>.  Both are always in
 * the DOM so url(#cfg-arrow) always resolves — the hidden set just
 * has zero-opacity fill that won't render.
 */

import { MARKER_COLORS } from "./pdgStyles";

export default function PdgMarkerDefs() {
  return (
    <defs>
      {/* Default — gray/slate */}
      <marker
        id="cfg-arrow"
        viewBox="0 0 10 10"
        refX="9"
        refY="5"
        markerWidth="7"
        markerHeight="7"
        orient="auto-start-reverse"
      >
        <path d="M 0 0 L 10 5 L 0 10 z" fill={MARKER_COLORS.default.dark} />
      </marker>
      <marker
        id="cfg-arrow-light"
        viewBox="0 0 10 10"
        refX="9"
        refY="5"
        markerWidth="7"
        markerHeight="7"
        orient="auto-start-reverse"
      >
        <path d="M 0 0 L 10 5 L 0 10 z" fill={MARKER_COLORS.default.light} />
      </marker>

      {/* True branch — green */}
      <marker
        id="cfg-arrow-green"
        viewBox="0 0 10 10"
        refX="9"
        refY="5"
        markerWidth="7"
        markerHeight="7"
        orient="auto-start-reverse"
      >
        <path d="M 0 0 L 10 5 L 0 10 z" fill={MARKER_COLORS.green.dark} />
      </marker>

      {/* False branch — red */}
      <marker
        id="cfg-arrow-red"
        viewBox="0 0 10 10"
        refX="9"
        refY="5"
        markerWidth="7"
        markerHeight="7"
        orient="auto-start-reverse"
      >
        <path d="M 0 0 L 10 5 L 0 10 z" fill={MARKER_COLORS.red.dark} />
      </marker>

      {/* Highlighted — emerald */}
      <marker
        id="cfg-arrow-highlighted"
        viewBox="0 0 10 10"
        refX="9"
        refY="5"
        markerWidth="7"
        markerHeight="7"
        orient="auto-start-reverse"
      >
        <path
          d="M 0 0 L 10 5 L 0 10 z"
          fill={MARKER_COLORS.highlighted.dark}
        />
      </marker>

      {/* Dimmed */}
      <marker
        id="cfg-arrow-dimmed"
        viewBox="0 0 10 10"
        refX="9"
        refY="5"
        markerWidth="7"
        markerHeight="7"
        orient="auto-start-reverse"
      >
        <path d="M 0 0 L 10 5 L 0 10 z" fill={MARKER_COLORS.dimmed.dark} />
      </marker>
    </defs>
  );
}
