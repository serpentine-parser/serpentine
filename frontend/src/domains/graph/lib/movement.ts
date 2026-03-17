import { CollisionDetector, SpatialIndex } from './collision';
import { Node } from '../model/types';

export class NodeMovement {
  private collisionDetector: CollisionDetector;

  constructor(collisionDetector: CollisionDetector) {
    this.collisionDetector = collisionDetector;
  }

  moveChildWithConstraints(
    id: string,
    parentId: string | null,
    x: number,
    y: number,
    nodes: Node[],
    padding: number
  ): { success: boolean; updatedNodes: Node[] } {
    if (!parentId) {
      const rootIndex = nodes.findIndex((n: any) => n.id === id);
      if (rootIndex === -1) return { success: false, updatedNodes: nodes };
      const root = nodes[rootIndex] as any;
      const dx = x - root.x;
      const dy = y - root.y;
      const moveAllDescendants = (node: any): any => {
        const movedNode = { ...node, x: node.x + dx, y: node.y + dy };
        if (node.children && node.children.length > 0)
          movedNode.children = node.children.map((child: any) => moveAllDescendants(child));
        return movedNode;
      };
      const newRoot = moveAllDescendants(root);
      const otherRoots = nodes.filter((n: any) => n.id !== id);
      const otherRootsIndex = this.collisionDetector.buildIndex(otherRoots);
      if (this.collisionDetector.checkCollision(newRoot, otherRootsIndex))
        return { success: false, updatedNodes: nodes };
      const updatedNodes = nodes.map((n: any) => (n.id === id ? newRoot : n));
      return { success: true, updatedNodes };
    }

    const findParentById = (nodeList: any[], searchId: string): any => {
      for (const node of nodeList) {
        if (node.id === searchId) return node;
        if (node.children && node.children.length > 0) {
          const found = findParentById(node.children, searchId);
          if (found) return found;
        }
      }
      return null;
    };

    const parent = findParentById(nodes, parentId);
    if (!parent || !parent.children) return { success: false, updatedNodes: nodes };
    const child = parent.children.find((c: any) => c.id === id);
    if (!child) return { success: false, updatedNodes: nodes };

    const finalX = x;
    const finalY = y;
    const dx = finalX - child.x;
    const dy = finalY - child.y;

    const moveChildAndDescendants = (child: any, newX: number, newY: number, dx: number, dy: number): any => {
      const updatedChild = { ...child, x: newX, y: newY };
      if (child.children && child.children.length > 0)
        updatedChild.children = child.children.map((grandchild: any) =>
          moveChildAndDescendants(grandchild, grandchild.x + dx, grandchild.y + dy, dx, dy)
        );
      return updatedChild;
    };

    const newChild = moveChildAndDescendants(child, finalX, finalY, dx, dy);
    const originalSiblings = parent.children.filter((s: any) => s.id !== id);
    const siblingsIndex = this.collisionDetector.buildIndex(originalSiblings);
    if (this.collisionDetector.checkCollision(newChild, siblingsIndex))
      return { success: false, updatedNodes: nodes };

    const otherRootsIndex = this.collisionDetector.buildIndex(nodes);

    const checkAncestorCollisionChain = (checkParentId: string, movedChild: any, rootsIndex: SpatialIndex): boolean => {
      const checkParent = findParentById(nodes, checkParentId);
      if (!checkParent) return false;
      const headerHeight = (checkParent.type === "module" || checkParent.isScope) ? 30 : 25;
      const hypotheticalChildren = checkParent.children.map((c: any) =>
        c.id === movedChild.id ? movedChild : c
      );
      let hypotheticalParentBounds;
      if (hypotheticalChildren.length > 0) {
        const minX = Math.min(...hypotheticalChildren.map((c: any) => c.x));
        const maxX = Math.max(...hypotheticalChildren.map((c: any) => c.x + c.width));
        const minY = Math.min(...hypotheticalChildren.map((c: any) => c.y));
        const maxY = Math.max(...hypotheticalChildren.map((c: any) => c.y + c.height));
        hypotheticalParentBounds = {
          ...checkParent,
          x: minX - padding, y: minY - padding - headerHeight,
          width: maxX - minX + padding * 2, height: maxY - minY + padding * 2 + headerHeight,
        };
      } else {
        hypotheticalParentBounds = checkParent;
      }
      const findDirectParent = (nodeList: any[], targetId: string): any => {
        for (const node of nodeList) {
          if (node.children) {
            const found = node.children.find((c: any) => c.id === targetId);
            if (found) return node;
            const deepFound = findDirectParent(node.children, targetId);
            if (deepFound) return deepFound;
          }
        }
        return null;
      };
      const grandParent = findDirectParent(nodes, checkParentId);
      if (grandParent) {
        const parentSiblings = grandParent.children.filter((c: any) => c.id !== checkParentId);
        const parentSiblingsIndex = this.collisionDetector.buildIndex(parentSiblings);
        if (this.collisionDetector.checkCollision(hypotheticalParentBounds, parentSiblingsIndex)) return true;
        return checkAncestorCollisionChain(grandParent.id, hypotheticalParentBounds, rootsIndex);
      } else {
        return this.collisionDetector.checkCollision(hypotheticalParentBounds, rootsIndex);
      }
    };

    if (checkAncestorCollisionChain(parentId, newChild, otherRootsIndex)) return { success: false, updatedNodes: nodes };

    const updateChildInParent = (targetParent: any): any => {
      if (targetParent.id === parentId) {
        const updatedChildren = targetParent.children.map((c: any) => (c.id === id ? newChild : c));
        const headerHeight = (targetParent.type === "module" || targetParent.isScope) ? 30 : 25;
        if (updatedChildren.length > 0) {
          const minX = Math.min(...updatedChildren.map((c: any) => c.x));
          const maxX = Math.max(...updatedChildren.map((c: any) => c.x + c.width));
          const minY = Math.min(...updatedChildren.map((c: any) => c.y));
          const maxY = Math.max(...updatedChildren.map((c: any) => c.y + c.height));
          return {
            ...targetParent,
            x: minX - padding, y: minY - padding - headerHeight,
            width: maxX - minX + padding * 2, height: maxY - minY + padding * 2 + headerHeight,
            children: updatedChildren,
          };
        }
        return { ...targetParent, children: updatedChildren };
      }
      if (targetParent.children && targetParent.children.length > 0)
        return { ...targetParent, children: targetParent.children.map((child: any) => updateChildInParent(child)) };
      return targetParent;
    };

    const updatedNodes = nodes.map((node) => updateChildInParent(node));
    return { success: true, updatedNodes };
  }

  updateParentBounds(parentId: string, nodes: Node[], padding: number = 10): Node[] {
    const calculateMinWidth = (child: any): number => {
      const labelLength = child.id.split(".").pop()?.length || 0;
      switch (child.type) {
        case "module": {
          const textWidth = labelLength * 8;
          const p = 20;
          const iconSpace = child.children?.length ? 25 : 0;
          return Math.max(child.width, textWidth + p + iconSpace);
        }
        case "class": {
          const classTextWidth = labelLength * 8;
          const classPadding = 20;
          const classIconSpace = child.children?.length ? 25 : 0;
          return Math.max(child.width, classTextWidth + classPadding + classIconSpace);
        }
        case "function": {
          const funcTextWidth = labelLength * 8;
          const funcPadding = 20;
          const funcIconSpace = child.children?.length ? 25 : 0;
          return Math.max(child.width, funcTextWidth + funcPadding + funcIconSpace);
        }
        default:
          return child.width;
      }
    };
    const findNodeById = (nodeList: any[], searchId: string): any => {
      for (const node of nodeList) {
        if (node.id === searchId) return node;
        if (node.children && node.children.length > 0) {
          const found = findNodeById(node.children, searchId);
          if (found) return found;
        }
      }
      return null;
    };
    const node = findNodeById(nodes, parentId);
    if (!node || !node.children || node.children.length === 0) return nodes;
    const headerHeight = (node.type === "module" || node.isScope) ? 30 : 25;
    const children = node.children;
    const minX = Math.min(...children.map((c: any) => c.x));
    const maxX = Math.max(...children.map((c: any) => c.x + calculateMinWidth(c)));
    const minY = Math.min(...children.map((c: any) => c.y));
    const maxY = Math.max(...children.map((c: any) => c.y + c.height));
    const updatedNode = {
      ...node,
      x: minX - padding, y: minY - padding - headerHeight,
      width: maxX - minX + padding * 2, height: maxY - minY + padding * 2 + headerHeight,
    };
    const updateNodeInTree = (targetNode: any): any => {
      if (targetNode.id === parentId) return updatedNode;
      if (targetNode.children && targetNode.children.length > 0)
        return { ...targetNode, children: targetNode.children.map((child: any) => updateNodeInTree(child)) };
      return targetNode;
    };
    return nodes.map((node) => updateNodeInTree(node));
  }
}
