import { IconArrowsMaximize, IconArrowsMinimize } from "@tabler/icons-react";
import type { Viewport } from "@domains/graph/model/types";

interface SvgNodeContextMenuProps {
  nodeId: string;
  nodeX: number;
  nodeY: number;
  nodeWidth: number;
  hasChildren?: boolean;
  selectedNodeId: string | null;
  expandChildren: (id: string) => void;
  collapseChildren: (id: string) => void;
  viewport: Viewport;
}

export function SvgNodeContextMenu({
  nodeId,
  nodeX,
  nodeY,
  nodeWidth,
  hasChildren = false,
  selectedNodeId,
  expandChildren,
  collapseChildren,
  viewport,
}: SvgNodeContextMenuProps) {
  if (selectedNodeId !== nodeId || !hasChildren) return null;

  type ButtonDef = {
    key: string;
    icon: React.ReactNode;
    onClick: (e: React.MouseEvent) => void;
    danger?: boolean;
  };

  const buttons: ButtonDef[] = [
    { key: "expand", icon: <IconArrowsMaximize size={16} strokeWidth={1.75} className="text-gray-600 dark:text-gray-300" />, onClick: (e: React.MouseEvent) => { e.stopPropagation(); expandChildren(nodeId); } },
    { key: "collapse", icon: <IconArrowsMinimize size={16} strokeWidth={1.75} className="text-gray-600 dark:text-gray-300" />, onClick: (e: React.MouseEvent) => { e.stopPropagation(); collapseChildren(nodeId); } },
  ];

  const buttonSize = 32;
  const buttonSpacing = 4;
  const padding = 8;
  const menuWidth = buttonSize * buttons.length + buttonSpacing * (buttons.length - 1) + padding * 2;
  const menuHeight = buttonSize + padding * 2;

  const anchorX = nodeX + nodeWidth + 8;
  const anchorY = nodeY - menuHeight - 8;
  const scale = 1 / viewport.zoom;

  return (
    <g
      className="context-menu"
      style={{ pointerEvents: "auto" }}
      transform={`translate(${anchorX}, ${anchorY}) scale(${scale})`}
    >
      <rect
        x={0}
        y={0}
        width={menuWidth}
        height={menuHeight}
        rx={12}
        className="fill-cyan-200/70 dark:fill-cyan-900/70 stroke-cyan-300 dark:stroke-cyan-700"
        strokeWidth={1}
        style={{ filter: "drop-shadow(0 4px 6px rgb(0 0 0 / 0.1))" }}
      />

      {buttons.map((btn, i) => {
        const bx = padding + i * (buttonSize + buttonSpacing);
        const by = padding;
        return (
          <g key={btn.key} transform={`translate(${bx}, ${by})`}>
            <rect
              width={buttonSize}
              height={buttonSize}
              rx={6}
              className={
                btn.danger
                  ? "fill-red-500 dark:fill-red-600 cursor-pointer hover:fill-red-600 dark:hover:fill-red-700"
                  : "fill-white dark:fill-slate-800 stroke-gray-200 dark:stroke-slate-600 cursor-pointer hover:fill-gray-50 dark:hover:fill-slate-700"
              }
              strokeWidth={btn.danger ? 0 : 1}
              onClick={btn.onClick}
            />
            <foreignObject width={buttonSize} height={buttonSize} className="pointer-events-none">
              <div className="w-full h-full flex items-center justify-center">
                {btn.icon}
              </div>
            </foreignObject>
            <rect
              width={buttonSize}
              height={buttonSize}
              className="fill-transparent cursor-pointer"
              onClick={btn.onClick}
            />
          </g>
        );
      })}
    </g>
  );
}
