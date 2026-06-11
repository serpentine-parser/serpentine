import type { Edge, Node } from '@domains/graph/model/types';

function safeId(id: string): string {
  return id.replaceAll('.', '_').replaceAll('/', '_').replaceAll('-', '_');
}

function emitNode(node: Node, lines: string[], indent: string): void {
  const visibleChildren = node.children?.filter((c) => !c.isGhost) ?? [];
  const hasVisibleChildren = !node.collapsed && visibleChildren.length > 0;
  const sid = safeId(node.id);
  const label = node.label ?? node.id.split('.').pop() ?? node.id;

  if (hasVisibleChildren) {
    lines.push(`${indent}subgraph ${sid}["${label}"]`);
    for (const child of visibleChildren) {
      emitNode(child, lines, indent + '  ');
    }
    lines.push(`${indent}end`);
  } else {
    lines.push(`${indent}${sid}["${label}"]`);
  }
}

function collectVisibleIds(nodes: Node[], out: Set<string>): void {
  for (const n of nodes) {
    out.add(n.id);
    if (!n.collapsed && n.children) collectVisibleIds(n.children, out);
  }
}

export function buildMermaidDiagram(nodes: Node[], edges: Edge[]): string {
  const lines: string[] = ['graph TD'];

  for (const node of nodes) {
    emitNode(node, lines, '  ');
  }

  const visibleIds = new Set<string>();
  collectVisibleIds(nodes, visibleIds);

  for (const edge of edges) {
    if (!visibleIds.has(edge.source) || !visibleIds.has(edge.target)) continue;
    const src = safeId(edge.source);
    const tgt = safeId(edge.target);
    lines.push(`  ${src} -->|${edge.type}| ${tgt}`);
  }

  return lines.join('\n') + '\n';
}
