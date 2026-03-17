import { Node } from "@domains/graph/model/types";
import * as d3 from "d3";
import { useEffect, useRef } from "react";
import Module from "./Module";
import type { NodeInteractionProps } from "./nodeInteraction";

interface NodeGroupProps {
  node: Node;
  moveChildWithConstraints: (id: string, parent: string | null, x: number, y: number) => boolean;
  layoutTransition: boolean;
  selectNode: (id: string | null) => void;
  setHoveredNode: (id: string | null) => void;
  getNodes: () => Node[];
  interaction: NodeInteractionProps;
}

export const NodeGroup = ({
  node,
  moveChildWithConstraints,
  layoutTransition,
  selectNode,
  setHoveredNode,
  getNodes,
  interaction,
}: NodeGroupProps) => {
  const ref = useRef<SVGGElement>(null);
  const prevPosition = useRef({ x: node.x, y: node.y });

  useEffect(() => {
    if (!node || !ref.current) return;
    const g = d3.select(ref.current);

    if (!node.parent) {
      const headerSelection = g.select<SVGRectElement>(
        `#${CSS.escape(node.id)}-header`,
      );
      const headerElement = headerSelection.node();

      if (headerElement) {
        let dragStartPosition = { x: 0, y: 0 };
        let accumulatedDelta = { dx: 0, dy: 0 };

        headerSelection.call(
          d3
            .drag<SVGRectElement, unknown>()
            .on("start", (event) => {
              const currentNodes = getNodes();
              const findNodeRecursively = (
                nodes: Node[],
                searchId: string,
              ): Node | null => {
                for (const n of nodes) {
                  if (n.id === searchId) return n;
                  if (n.children) {
                    const found = findNodeRecursively(n.children, searchId);
                    if (found) return found;
                  }
                }
                return null;
              };
              const currentNode = findNodeRecursively(currentNodes, node.id);
              if (currentNode) {
                dragStartPosition = { x: currentNode.x, y: currentNode.y };
                accumulatedDelta = { dx: 0, dy: 0 };
              }
            })
            .on("drag", (event) => {
              accumulatedDelta.dx += event.dx;
              accumulatedDelta.dy += event.dy;

              if (
                Math.abs(accumulatedDelta.dx) > 2 ||
                Math.abs(accumulatedDelta.dy) > 2
              ) {
                const newX = dragStartPosition.x + accumulatedDelta.dx;
                const newY = dragStartPosition.y + accumulatedDelta.dy;

                const success = moveChildWithConstraints(
                  node.id,
                  null,
                  newX,
                  newY,
                );
                if (success) {
                  dragStartPosition = { x: newX, y: newY };
                  accumulatedDelta = { dx: 0, dy: 0 };
                }
              }
            }),
        );

        headerSelection
          .on("click", (event) => {
            event.stopPropagation();
            selectNode(node.id);
          })
          .on("mouseenter", () => {
            setHoveredNode(node.id);
          })
          .on("mouseleave", () => {
            setHoveredNode(null);
          })
          .style("cursor", "pointer");
      }
    }

    const setupChildDragHandlers = (currentNode: Node) => {
      if (currentNode.children) {
        currentNode.children.forEach((child) => {
          const headerSelection = g.select<SVGRectElement>(
            `#${CSS.escape(child.id)}-header`,
          );
          const headerElement = headerSelection.node();

          if (headerElement && child.parent) {
            let dragStartPosition = { x: 0, y: 0 };
            let accumulatedDelta = { dx: 0, dy: 0 };

            headerSelection.call(
              d3
                .drag<SVGRectElement, unknown>()
                .on("start", (event) => {
                  const currentNodes = getNodes();
                  const findNodeRecursively = (
                    nodes: Node[],
                    searchId: string,
                  ): Node | null => {
                    for (const n of nodes) {
                      if (n.id === searchId) return n;
                      if (n.children) {
                        const found = findNodeRecursively(n.children, searchId);
                        if (found) return found;
                      }
                    }
                    return null;
                  };
                  const currentChild = findNodeRecursively(
                    currentNodes,
                    child.id,
                  );
                  if (currentChild) {
                    dragStartPosition = {
                      x: currentChild.x,
                      y: currentChild.y,
                    };
                    accumulatedDelta = { dx: 0, dy: 0 };
                  }
                })
                .on("drag", (event) => {
                  accumulatedDelta.dx += event.dx;
                  accumulatedDelta.dy += event.dy;

                  if (
                    Math.abs(accumulatedDelta.dx) > 2 ||
                    Math.abs(accumulatedDelta.dy) > 2
                  ) {
                    const newX = dragStartPosition.x + accumulatedDelta.dx;
                    const newY = dragStartPosition.y + accumulatedDelta.dy;

                    const success = moveChildWithConstraints(
                      child.id,
                      child.parent || null,
                      newX,
                      newY,
                    );
                    if (success) {
                      dragStartPosition = { x: newX, y: newY };
                      accumulatedDelta = { dx: 0, dy: 0 };
                    }
                  }
                }),
            );

            headerSelection
              .on("click", (event) => {
                event.stopPropagation();
                selectNode(child.id);
              })
              .on("mouseenter", () => {
                setHoveredNode(child.id);
              })
              .on("mouseleave", () => {
                setHoveredNode(null);
              })
              .style("cursor", "pointer");
          }
          setupChildDragHandlers(child);
        });
      }
    };
    setupChildDragHandlers(node);
  }, [node, moveChildWithConstraints, selectNode, setHoveredNode, getNodes]);

  useEffect(() => {
    if (!ref.current) return;

    const prev = prevPosition.current;
    const dx = prev.x - node.x;
    const dy = prev.y - node.y;
    prevPosition.current = { x: node.x, y: node.y };

    const positionChanged = dx !== 0 || dy !== 0;
    if (!layoutTransition || !positionChanged) {
      d3.select(ref.current).attr("transform", null);
      return;
    }

    const g = d3.select(ref.current);
    g.attr("transform", `translate(${dx}, ${dy})`);
    g.transition()
      .duration(300)
      .ease(d3.easeQuadInOut)
      .attr("transform", "translate(0, 0)")
      .on("end", () => g.attr("transform", null));
  }, [node.x, node.y, layoutTransition]);

  return (
    <g key={node.id} ref={ref}>
      <Module key={node.id} {...node} {...interaction} />
    </g>
  );
};

export default NodeGroup;
