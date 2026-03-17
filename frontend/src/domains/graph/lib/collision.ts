import { Node } from '../model/types';

export class SpatialIndex {
  private cells: Map<string, Node[]>;
  private cellSize: number;

  constructor(nodes: Node[], cellSize: number) {
    this.cellSize = cellSize;
    this.cells = new Map();
    for (const node of nodes) {
      const minCX = Math.floor(node.x / cellSize);
      const maxCX = Math.floor((node.x + node.width) / cellSize);
      const minCY = Math.floor(node.y / cellSize);
      const maxCY = Math.floor((node.y + node.height) / cellSize);
      for (let cx = minCX; cx <= maxCX; cx++) {
        for (let cy = minCY; cy <= maxCY; cy++) {
          const key = `${cx},${cy}`;
          const cell = this.cells.get(key);
          if (cell) cell.push(node);
          else this.cells.set(key, [node]);
        }
      }
    }
  }

  candidates(node: Node): Node[] {
    const { cellSize } = this;
    const minCX = Math.floor(node.x / cellSize);
    const maxCX = Math.floor((node.x + node.width) / cellSize);
    const minCY = Math.floor(node.y / cellSize);
    const maxCY = Math.floor((node.y + node.height) / cellSize);
    const seen = new Set<string>();
    const result: Node[] = [];
    for (let cx = minCX; cx <= maxCX; cx++) {
      for (let cy = minCY; cy <= maxCY; cy++) {
        const cell = this.cells.get(`${cx},${cy}`);
        if (!cell) continue;
        for (const candidate of cell) {
          if (!seen.has(candidate.id)) {
            seen.add(candidate.id);
            result.push(candidate);
          }
        }
      }
    }
    return result;
  }
}

const BRUTE_FORCE_THRESHOLD = 8;

export class CollisionDetector {
  buildIndex(nodes: Node[]): SpatialIndex {
    const widths = nodes.map((n) => n.width).sort((a, b) => a - b);
    const heights = nodes.map((n) => n.height).sort((a, b) => a - b);
    const medianWidth = widths[Math.floor(widths.length / 2)] ?? 100;
    const medianHeight = heights[Math.floor(heights.length / 2)] ?? 100;
    const cellSize = Math.max(medianWidth, medianHeight) * 2;
    return new SpatialIndex(nodes, cellSize);
  }

  checkCollision(node: Node, index: SpatialIndex): boolean {
    for (const candidate of index.candidates(node)) {
      if (candidate.id === node.id) continue;
      if (
        node.x < candidate.x + candidate.width &&
        node.x + node.width > candidate.x &&
        node.y < candidate.y + candidate.height &&
        node.y + node.height > candidate.y
      ) return true;
    }
    return false;
  }

  checkNodeCollision(child: Node, siblings: Node[]): boolean {
    if (siblings.length < BRUTE_FORCE_THRESHOLD) {
      return siblings.some((sibling) => {
        if (sibling.id === child.id) return false;
        return (
          child.x < sibling.x + sibling.width &&
          child.x + child.width > sibling.x &&
          child.y < sibling.y + sibling.height &&
          child.y + child.height > sibling.y
        );
      });
    }
    const idx = this.buildIndex(siblings);
    return this.checkCollision(child, idx);
  }
}
