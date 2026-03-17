import { MouseEvent } from "react";
import { Node } from "@domains/graph/model/types";
import RenderChild from "./RenderChild";
import { getNodeClasses } from "./nodeClasses";
import { getChangeStatusColor } from "@ui/lib/nodeStyles";
import type { NodeInteractionProps } from "./nodeInteraction";

type Props = Node & NodeInteractionProps & {
  parentId?: string;
};

const Class = (props: Props) => {
  const { x, y, width, height, children = [], parentId, selectedNodeId, hoveredNodeId, onToggleCollapse, onDismissChange } = props;
  const hasChildren = children && children.length > 0;
  const isCollapsed = props.collapsed;
  const isChildrenLoading = props.isChildrenLoading || false;
  const headerHeight = 28;
  const changeColor = getChangeStatusColor(props.changeStatus);

  const classes = getNodeClasses();

  const isSelected = selectedNodeId === props.id;
  const isHighlighted = props.highlighted;
  const isHovered = hoveredNodeId === props.id;
  const hasSelection = selectedNodeId !== null;

  let nodeState: "default" | "selected" | "highlighted" | "dimmed";
  if (isSelected) {
    nodeState = "selected";
  } else if (isHighlighted) {
    nodeState = "highlighted";
  } else if (hasSelection) {
    nodeState = "dimmed";
  } else {
    nodeState = "default";
  }

  const handleToggleCollapse = (e: MouseEvent<SVGRectElement>) => {
    e.stopPropagation();
    onToggleCollapse(props.id);
  };

  const handleDismiss = (e: MouseEvent) => {
    e.stopPropagation();
    onDismissChange(props.id);
  };

  const validChildren = children.filter((child) => {
    const isAtOrigin = child.x === 0 && child.y === 0;
    return !isAtOrigin || !props.isChildrenLoading;
  });

  const dismissX = hasChildren ? x + width - 48 : x + width - 25;

  const interaction: NodeInteractionProps = { selectedNodeId, hoveredNodeId, onToggleCollapse, onDismissChange };

  return (
    <g id={props.id} style={props.isGhost ? { opacity: 0.5 } : undefined}>
      <rect
        x={x}
        y={y}
        width={width}
        height={height}
        className={`${
          nodeState === "selected"
            ? classes.containerSelected
            : nodeState === "highlighted"
              ? classes.containerHighlighted
              : nodeState === "dimmed"
                ? classes.containerDimmed
                : classes.container
        } ${isHovered && nodeState === "default" ? classes.containerHover : ""}`}
        strokeWidth="1.2"
        rx="8"
        strokeDasharray={nodeState === "selected" ? undefined : "6 3"}
        style={{ pointerEvents: parentId ? "none" : "auto" }}
      />

      <rect
        id={`${props.id}-header`}
        x={x}
        y={y}
        width={width}
        height={headerHeight}
        className={`${
          nodeState === "selected"
            ? classes.headerSelected
            : nodeState === "highlighted"
              ? classes.headerHighlighted
              : nodeState === "dimmed"
                ? classes.headerDimmed
                : classes.header
        }`}
        rx="8"
        style={{ cursor: parentId ? "move" : "default", pointerEvents: "all" }}
      />
      {!isCollapsed && <rect
        x={x}
        y={y + headerHeight - 8}
        width={width}
        height="8"
        className={`${
          nodeState === "selected"
            ? classes.headerSelected
            : nodeState === "highlighted"
              ? classes.headerHighlighted
              : nodeState === "dimmed"
                ? classes.headerDimmed
                : classes.header
        }`}
        style={{ pointerEvents: "none" }}
      />}

      {changeColor && (
        <rect
          x={x} y={y} width={width} height={height}
          fill="none" stroke={changeColor} strokeWidth="2" rx="8"
          strokeDasharray={props.changeStatus === "deleted" ? "6 3" : undefined}
          style={{ pointerEvents: "none" }}
        />
      )}

      {changeColor && (
        <rect
          x={x} y={y} width={width} height={headerHeight}
          fill={changeColor} fillOpacity="0.15" rx="8"
          style={{ pointerEvents: "none" }}
        />
      )}

      {hasChildren && (
        <g>
          <rect
            x={x + width - 25}
            y={y + 5}
            width="20"
            height="20"
            className={classes.toggleButton}
            strokeWidth="1"
            rx="3"
            style={{ cursor: "pointer", pointerEvents: "all" }}
            onClick={handleToggleCollapse}
          />
          <text
            x={x + width - 15}
            y={y + 18}
            textAnchor="middle"
            className={classes.toggleText}
            fontSize="12"
            fontWeight="bold"
            style={{ cursor: "pointer", pointerEvents: "none" }}
          >
            {isCollapsed ? "+" : "−"}
          </text>
        </g>
      )}

      <text
        x={x + 10}
        y={y + 19}
        className={classes.titleText}
        fontSize="12"
        fontWeight="bold"
        fontFamily="'Fira Code', monospace"
        style={{ pointerEvents: "none" }}
      >
        {props.id.split(".").pop()}
      </text>

      {!isCollapsed && validChildren.map((child) => RenderChild(props, child, interaction))}

      {!isCollapsed && isChildrenLoading && (
        <g>
          <rect
            x={x + 8}
            y={y + headerHeight + 8}
            width={Math.max(0, width - 16)}
            height={Math.max(0, height - headerHeight - 16)}
            className="fill-white/80 dark:fill-gray-800/80"
            rx="4"
          />
          <circle
            cx={x + width / 2}
            cy={y + height / 2}
            r="6"
            fill="none"
            className={classes.loadingStroke}
            strokeWidth="2"
            strokeLinecap="round"
            strokeDasharray="4 4"
          >
            <animateTransform
              attributeName="transform"
              type="rotate"
              values="0;360"
              dur="1s"
              repeatCount="indefinite"
              transformOrigin={`${x + width / 2} ${y + height / 2}`}
            />
          </circle>
          <text
            x={x + width / 2}
            y={y + height / 2 + 18}
            textAnchor="middle"
            className={classes.loadingText}
            fontSize="9"
            style={{ pointerEvents: "none" }}
          >
            Positioning...
          </text>
        </g>
      )}
    </g>
  );
};

export default Class;
