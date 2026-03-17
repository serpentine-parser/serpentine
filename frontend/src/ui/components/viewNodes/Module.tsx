import { MouseEvent } from "react";
import { Node } from "@domains/graph/model/types";
import RenderChild from "./RenderChild";
import { getNodeClasses } from "./nodeClasses";
import { getChangeStatusColor } from "@ui/lib/nodeStyles";
import type { NodeInteractionProps } from "./nodeInteraction";

type Props = Node & NodeInteractionProps & {
  parentId?: string;
};

const Module = (props: Props) => {
  const { selectedNodeId, hoveredNodeId, onToggleCollapse, onDismissChange } = props;
  const hasChildren = props.children && props.children.length > 0;
  const isCollapsed = props.collapsed;
  const isChildrenLoading = props.isChildrenLoading || false;
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

  const validChildren = (props.children || []).filter((child) => {
    const isAtOrigin = child.x === 0 && child.y === 0;
    return !isAtOrigin || !props.isChildrenLoading;
  });

  const handleToggleCollapse = (e: MouseEvent) => {
    e.stopPropagation();
    onToggleCollapse(props.id);
  };

  const handleDismiss = (e: MouseEvent) => {
    e.stopPropagation();
    onDismissChange(props.id);
  };

  const dismissX = hasChildren
    ? props.x + props.width - 48
    : props.x + props.width - 25;

  const interaction: NodeInteractionProps = { selectedNodeId, hoveredNodeId, onToggleCollapse, onDismissChange };

  return (
    <g id={props.id} style={props.isGhost ? { opacity: 0.5 } : undefined}>
      <rect
        x={props.x}
        y={props.y}
        width={props.width}
        height={props.height}
        className={`${
          nodeState === "selected"
            ? classes.containerSelected
            : nodeState === "highlighted"
              ? classes.containerHighlighted
              : nodeState === "dimmed"
                ? classes.containerDimmed
                : classes.container
        } ${isHovered ? classes.containerHover : ""}`}
        strokeWidth="1.2"
        rx="8"
        strokeDasharray={nodeState === "selected" ? undefined : "6 3"}
        style={nodeState === "selected" ? { filter: "url(#shadow)" } : {}}
      />

      <rect
        id={`${props.id}-header`}
        x={props.x}
        y={props.y}
        width={props.width}
        height="28"
        className={`${
          nodeState === "selected"
            ? classes.headerSelected
            : nodeState === "highlighted"
              ? classes.headerHighlighted
              : nodeState === "dimmed"
                ? classes.headerDimmed
                : classes.header
        }`}
        rx="5"
        style={{ cursor: "move", pointerEvents: "all" }}
      />

      {!isCollapsed && <rect
        x={props.x}
        y={props.y + 20}
        width={props.width}
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
        style={{ cursor: "move", pointerEvents: "all" }}
      />}

      {changeColor && (
        <rect
          x={props.x}
          y={props.y}
          width={props.width}
          height={props.height}
          fill="none"
          stroke={changeColor}
          strokeWidth="2"
          rx="8"
          strokeDasharray={props.changeStatus === "deleted" ? "6 3" : undefined}
          style={{ pointerEvents: "none" }}
        />
      )}

      {changeColor && (
        <rect
          x={props.x}
          y={props.y}
          width={props.width}
          height="28"
          fill={changeColor}
          fillOpacity="0.15"
          rx="5"
          style={{ pointerEvents: "none" }}
        />
      )}

      {hasChildren && (
        <g>
          <rect
            x={props.x + props.width - 25}
            y={props.y + 5}
            width="20"
            height="20"
            className={classes.toggleButton}
            strokeWidth="1"
            rx="3"
            style={{ cursor: "pointer", pointerEvents: "all" }}
            onClick={handleToggleCollapse}
          />
          <text
            x={props.x + props.width - 15}
            y={props.y + 18}
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
        x={props.x + 10}
        y={props.y + 20}
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
            x={props.x + 5}
            y={props.y + 35}
            width={Math.max(0, props.width - 10)}
            height={Math.max(0, props.height - 40)}
            className="fill-white/80 dark:fill-slate-800/80"
            rx="4"
          />
          <circle
            cx={props.x + props.width / 2}
            cy={props.y + props.height / 2}
            r="8"
            fill="none"
            className={classes.loadingStroke}
            strokeWidth="2"
            strokeLinecap="round"
            strokeDasharray="6 6"
          >
            <animateTransform
              attributeName="transform"
              type="rotate"
              values="0;360"
              dur="1s"
              repeatCount="indefinite"
              transformOrigin={`${props.x + props.width / 2} ${
                props.y + props.height / 2
              }`}
            />
          </circle>
          <text
            x={props.x + props.width / 2}
            y={props.y + props.height / 2 + 25}
            textAnchor="middle"
            className={classes.loadingText}
            fontSize="10"
            style={{ pointerEvents: "none" }}
          >
            Positioning...
          </text>
        </g>
      )}
    </g>
  );
};

export default Module;
