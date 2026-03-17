// Shared interaction props for SVG node components
export type NodeInteractionProps = {
  selectedNodeId: string | null;
  hoveredNodeId: string | null;
  onToggleCollapse: (id: string) => void;
  onDismissChange: (id: string) => void;
};
