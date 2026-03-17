import { Edge, Node } from '../model/types';

function findDependencies(nodeId: string, edges: Edge[]): Set<string> {
  const dependencies = new Set<string>();
  edges.forEach((edge) => {
    if (edge.source === nodeId) dependencies.add(edge.target);
  });
  return dependencies;
}

function findDependents(nodeId: string, edges: Edge[]): Set<string> {
  const dependents = new Set<string>();
  edges.forEach((edge) => {
    if (edge.target === nodeId) dependents.add(edge.source);
  });
  return dependents;
}

export class SelectionAndHighlighting {
  private nodeUtils: any;

  constructor(nodeUtils: any) {
    this.nodeUtils = nodeUtils;
  }

  selectNode(
    nodeId: string | null,
    currentSelectedNodeId: string | null,
    nodes: Node[],
    allEdges: Edge[]
  ): { selectedNodeId: string | null; highlightedNodes: Node[] } {
    if (nodeId === currentSelectedNodeId) {
      return { selectedNodeId: null, highlightedNodes: this.clearHighlights(nodes) };
    }

    if (nodeId) {
      const getAllChildIds = (node: Node): string[] => {
        const childIds = [node.id];
        if (node.children) node.children.forEach((child) => childIds.push(...getAllChildIds(child)));
        return childIds;
      };

      const selectedNode = this.nodeUtils.findNodeById(nodeId, nodes);
      if (selectedNode) {
        const allRelatedIds = getAllChildIds(selectedNode);
        const allDependencies = new Set<string>();
        const allDependents = new Set<string>();
        allRelatedIds.forEach((id) => {
          findDependencies(id, allEdges).forEach((depId: string) => allDependencies.add(depId));
          findDependents(id, allEdges).forEach((depId: string) => allDependents.add(depId));
        });
        return {
          selectedNodeId: nodeId,
          highlightedNodes: this.highlightDependenciesCustom(nodeId, allDependencies, allDependents, nodes),
        };
      } else {
        return {
          selectedNodeId: nodeId,
          highlightedNodes: this.highlightDependencies(nodeId, "both", nodes, allEdges),
        };
      }
    } else {
      return { selectedNodeId: null, highlightedNodes: this.clearHighlights(nodes) };
    }
  }

  highlightDependencies(
    nodeId: string,
    direction: "upstream" | "downstream" | "both",
    nodes: Node[],
    allEdges: Edge[]
  ): Node[] {
    const dependencies = findDependencies(nodeId, allEdges);
    const dependents = findDependents(nodeId, allEdges);

    const updateNodeHighlighting = (nodeList: Node[]): Node[] => {
      return nodeList.map((node) => {
        let highlighted: "upstream" | "downstream" | "path" | null = null;
        if (node.id === nodeId) highlighted = "path";
        else if ((direction === "both" || direction === "upstream") && dependencies.has(node.id)) highlighted = "upstream";
        if ((direction === "both" || direction === "downstream") && dependents.has(node.id)) highlighted = "downstream";
        const processedChildren = node.children ? updateNodeHighlighting(node.children) : undefined;
        if (!highlighted && processedChildren) {
          const hasHighlightedChild = processedChildren.some((child) => child.highlighted);
          if (hasHighlightedChild) {
            const hasUpstream = processedChildren.some((child) => child.highlighted === "upstream");
            const hasDownstream = processedChildren.some((child) => child.highlighted === "downstream");
            const hasPath = processedChildren.some((child) => child.highlighted === "path");
            if (hasPath) highlighted = "path";
            else if (hasUpstream && hasDownstream) highlighted = "path";
            else if (hasUpstream) highlighted = "upstream";
            else if (hasDownstream) highlighted = "downstream";
          }
        }
        return { ...node, highlighted, children: processedChildren };
      });
    };
    return updateNodeHighlighting(nodes);
  }

  highlightDependenciesCustom(
    selectedNodeId: string,
    dependencies: Set<string>,
    dependents: Set<string>,
    nodes: Node[]
  ): Node[] {
    const updateNodeHighlighting = (nodeList: Node[]): Node[] => {
      return nodeList.map((node) => {
        let highlighted: "upstream" | "downstream" | "path" | null = null;
        if (node.id === selectedNodeId) highlighted = "path";
        else if (dependencies.has(node.id)) highlighted = "upstream";
        else if (dependents.has(node.id)) highlighted = "downstream";
        const processedChildren = node.children ? updateNodeHighlighting(node.children) : undefined;
        if (!highlighted && processedChildren) {
          const hasHighlightedChild = processedChildren.some((child) => child.highlighted);
          if (hasHighlightedChild) {
            const hasUpstream = processedChildren.some((child) => child.highlighted === "upstream");
            const hasDownstream = processedChildren.some((child) => child.highlighted === "downstream");
            const hasPath = processedChildren.some((child) => child.highlighted === "path");
            if (hasPath) highlighted = "path";
            else if (hasUpstream && hasDownstream) highlighted = "path";
            else if (hasUpstream) highlighted = "upstream";
            else if (hasDownstream) highlighted = "downstream";
          }
        }
        return { ...node, highlighted, children: processedChildren };
      });
    };
    return updateNodeHighlighting(nodes);
  }

  clearHighlights(nodes: Node[]): Node[] {
    const clearNodeHighlighting = (nodeList: Node[]): Node[] => {
      return nodeList.map((node) => ({
        ...node,
        highlighted: null,
        children: node.children ? clearNodeHighlighting(node.children) : undefined,
      }));
    };
    return clearNodeHighlighting(nodes);
  }
}
