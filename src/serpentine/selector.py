"""
dbt-style selector logic for filtering hierarchical dependency graphs.

Supports:
- pattern: Exact and glob matching on node id
- +pattern: Pattern + all upstream (dependencies)
- pattern+: Pattern + all downstream (dependents)
- +pattern+: Pattern + both directions
- @pattern: Pattern + complete connected component (all upstream and downstream transitively)
- Multiple patterns separated by commas (union)
- Exclude with --exclude pattern or via exclude parameter
"""

import copy
import fnmatch
import re
from typing import Any, Dict, List, Optional, Set


class GraphSelector:
    """dbt-style selector for filtering hierarchical dependency graphs."""

    def __init__(self):
        """Initialize the selector."""
        self.flat_nodes: Dict[str, dict] = {}  # id -> node info
        self.node_parents: Dict[str, Optional[str]] = {}  # id -> parent_id
        self.parent_children: Dict[str, Set[str]] = {}  # parent_id -> set(child_ids)
        self.edges_forward: Dict[str, Set[str]] = {}  # source -> set(targets)
        self.edges_backward: Dict[str, Set[str]] = {}  # target -> set(sources)

    @staticmethod
    def resolve(
        graph: dict, select: str = "", exclude: str = "", state: str = ""
    ) -> dict:
        """
        Resolve a selector expression against a graph and return filtered graph.

        Args:
            graph: Dict with "nodes" (hierarchical) and "edges" (list)
            select: Selector expression (e.g., "+pattern", "mod*.py+", "@core")
            exclude: Patterns to exclude (e.g., "test*,mock*")
            state: Comma-separated change states to include (e.g., "modified,added,deleted")

        Returns:
            Filtered graph with same structure as input
        """
        selector = GraphSelector()
        return selector._resolve(graph, select, exclude, state)

    def _resolve(
        self, graph: dict, select: str = "", exclude: str = "", state: str = ""
    ) -> dict:
        """Internal resolve implementation."""
        # Extract nodes and edges
        nodes = graph.get("nodes", [])
        edges = graph.get("edges", [])
        metadata = graph.get("metadata", {})

        # If no selector and no exclusion, skip graph traversal
        if not select and not exclude:
            if state:
                states = {s.strip() for s in state.split(",") if s.strip()}
                return filter_by_state(graph, states)
            return graph

        # Flatten the hierarchical nodes for easier traversal
        self._flatten_nodes(nodes)

        # Build edge lookup tables (bidirectional)
        self._build_edge_maps(edges)

        # Parse and expand the selector (if no select, select all nodes)
        if select:
            selected_ids = self._parse_and_expand_selector(select)
        else:
            # No select query means select everything
            selected_ids = set(self.flat_nodes.keys())

        # Always include parent nodes for hierarchy context
        selected_ids = self._include_parent_nodes(selected_ids)

        # Apply exclusions AFTER including parents/children
        # This ensures excluded nodes stay excluded even if they're children of selected parents
        if exclude:
            excluded_ids = self._parse_exclude_patterns(exclude)
            selected_ids -= excluded_ids

        # Filter nodes and reconstruct hierarchy
        filtered_nodes = self._reconstruct_hierarchy(nodes, selected_ids)

        # Filter edges - only keep edges between selected nodes
        filtered_edges = []
        for edge in edges:
            # Support both old format (source/target) and new format (caller/callee)
            source = edge.get("source") or edge.get("caller")
            target = edge.get("target") or edge.get("callee")
            if source in selected_ids and target in selected_ids:
                filtered_edges.append(edge)

        # Return filtered graph
        result = {
            "nodes": filtered_nodes,
            "edges": filtered_edges,
        }

        # Preserve metadata if present
        if metadata:
            result["metadata"] = metadata

        # Apply state filter as final step
        if state:
            states = {s.strip() for s in state.split(",") if s.strip()}
            result = filter_by_state(result, states)

        return result

    def _flatten_nodes(
        self, nodes: List[dict], parent_id: Optional[str] = None
    ) -> None:
        """Flatten hierarchical nodes into a flat lookup table."""
        for node in nodes:
            node_id = node.get("id")
            if node_id:
                self.flat_nodes[node_id] = copy.deepcopy(node)
                self.node_parents[node_id] = parent_id
                self.parent_children.setdefault(parent_id, set()).add(node_id)  # type: ignore

                # Recursively flatten children
                children = node.get("children", [])
                if children:
                    self._flatten_nodes(children, node_id)

    def _build_edge_maps(self, edges: List[dict]) -> None:
        """Build forward and backward edge lookup tables."""
        for edge in edges:
            # Support both old format (source/target) and new format (caller/callee)
            source = edge.get("source") or edge.get("caller")
            target = edge.get("target") or edge.get("callee")
            if source and target:
                self.edges_forward.setdefault(source, set()).add(target)
                self.edges_backward.setdefault(target, set()).add(source)

    def _parse_and_expand_selector(self, select: str) -> Set[str]:
        """Parse selector expression and expand to matching node ids.

        Supported forms (dbt-style):
          pattern         - exact/glob match
          +pattern        - pattern + all upstream (transitive dependencies)
          pattern+        - pattern + all downstream (transitive dependents)
          +pattern+       - pattern + both directions (unlimited)
          N+pattern       - pattern + N-hop upstream
          pattern+N       - pattern + N-hop downstream
          N+pattern+M     - pattern + N-hop upstream + M-hop downstream
          @pattern        - pattern + full connected component
        """
        selected: Set[str] = set()

        # Regex: optional leading depth+plus, base pattern, optional trailing plus+depth
        # Examples: "2+my_model", "my_model+3", "1+my_model+2", "+my_model+", "@my_model"
        TOKEN_RE = re.compile(
            r"^(?:(@)|(\d*)\+)?(.+?)(?:\+(\d*))?$"
        )

        for pattern in [p.strip() for p in select.split(",")]:
            if not pattern:
                continue

            m = TOKEN_RE.match(pattern)
            if not m:
                continue

            at_op, left_n, base_pattern, right_n = m.groups()

            if at_op:
                # @pattern — full connected component
                matched = self._match_pattern(base_pattern)
                selected.update(self._expand_component(matched))
                continue

            # left_n/right_n are None when the +/N+ prefix/suffix is absent entirely.
            # left_n/right_n == "" when a bare + was used (unlimited depth).
            # left_n/right_n == "N" when a numeric depth was given.
            do_upstream = left_n is not None
            do_downstream = right_n is not None
            up_depth = int(left_n) if left_n else None    # None = unlimited
            down_depth = int(right_n) if right_n else None  # None = unlimited

            matched = self._match_pattern(base_pattern)
            expanded = matched.copy()

            for node_id in matched:
                # Include node itself and all hierarchy descendants as traversal roots
                # so that +module selectors pick up edges from functions/classes within
                traversal_roots = {node_id} | self._get_all_descendants(node_id)
                if do_upstream:
                    for root in traversal_roots:
                        expanded.update(self._traverse(self.edges_forward, root, up_depth))
                if do_downstream:
                    for root in traversal_roots:
                        expanded.update(self._traverse(self.edges_backward, root, down_depth))

            selected.update(expanded)

        return selected

    def _traverse(
        self,
        edge_map: Dict[str, Set[str]],
        start: str,
        max_hops: Optional[int],
    ) -> Set[str]:
        """BFS traversal over edge_map from start, up to max_hops (None = unlimited)."""
        visited: Set[str] = set()
        queue = [(start, 0)]
        while queue:
            node_id, depth = queue.pop(0)
            if max_hops is not None and depth >= max_hops:
                continue
            for neighbor in edge_map.get(node_id, set()):
                if neighbor not in visited:
                    visited.add(neighbor)
                    queue.append((neighbor, depth + 1))
        return visited

    def _match_pattern(self, pattern: str) -> Set[str]:
        """Match pattern against flat node ids using glob matching."""
        # Normalize ** to * — in dotted IDs, * already matches dots
        normalized = pattern.replace("**", "*")
        matched: Set[str] = set()
        for node_id in self.flat_nodes.keys():
            if fnmatch.fnmatch(node_id, normalized):
                matched.add(node_id)
        return matched

    def _get_upstream(
        self, node_id: str, visited: Optional[Set[str]] = None
    ) -> Set[str]:
        """Get all upstream nodes (transitive dependencies)."""
        if visited is None:
            visited = set()

        if node_id in visited:
            return set()

        visited.add(node_id)
        upstream: Set[str] = set()

        # Get direct dependencies (targets of edges where this is source)
        for target in self.edges_forward.get(node_id, set()):
            upstream.add(target)
            upstream.update(self._get_upstream(target, visited))

        return upstream

    def _get_downstream(
        self, node_id: str, visited: Optional[Set[str]] = None
    ) -> Set[str]:
        """Get all downstream nodes (transitive dependents)."""
        if visited is None:
            visited = set()

        if node_id in visited:
            return set()

        visited.add(node_id)
        downstream: Set[str] = set()

        # Get direct dependents (sources of edges where this is target)
        for source in self.edges_backward.get(node_id, set()):
            downstream.add(source)
            downstream.update(self._get_downstream(source, visited))

        return downstream

    def _expand_component(self, node_ids: Set[str]) -> Set[str]:
        """Expand nodes to their complete connected component."""
        result: Set[str] = node_ids.copy()
        queue = list(node_ids)

        while queue:
            node_id = queue.pop(0)

            # Get upstream
            upstream = self._get_upstream(node_id)
            for node in upstream:
                if node not in result:
                    result.add(node)
                    queue.append(node)

            # Get downstream
            downstream = self._get_downstream(node_id)
            for node in downstream:
                if node not in result:
                    result.add(node)
                    queue.append(node)

        return result

    def _parse_exclude_patterns(self, exclude: str) -> Set[str]:
        """Parse exclude patterns and return matching node ids."""
        excluded: Set[str] = set()

        if not exclude:
            return excluded

        # Handle --exclude prefix
        if exclude.startswith("--exclude "):
            exclude = exclude[10:].strip()

        # Split by comma
        patterns = [p.strip() for p in exclude.split(",")]

        for pattern in patterns:
            if pattern:
                matched = self._match_pattern(pattern)
                excluded.update(matched)
                # Also exclude all descendants in the hierarchy
                for node_id in matched:
                    excluded.update(self._get_all_descendants(node_id))

        return excluded

    def _get_all_descendants(self, node_id: str) -> Set[str]:
        """Get all descendant nodes in the hierarchy (children, grandchildren, etc.)."""
        descendants: Set[str] = set()
        queue = [node_id]

        while queue:
            current = queue.pop(0)
            for child_id in self.parent_children.get(current, set()):
                if child_id not in descendants:
                    descendants.add(child_id)
                    queue.append(child_id)

        return descendants

    def _include_parent_nodes(self, selected_ids: Set[str]) -> Set[str]:
        """Add parent and children nodes to maintain hierarchy context."""
        result = selected_ids.copy()

        # Add all parent nodes (ancestors)
        for node_id in list(selected_ids):
            parent_id = self.node_parents.get(node_id)
            while parent_id is not None:
                result.add(parent_id)
                parent_id = self.node_parents.get(parent_id)

        # Add all child nodes (descendants in the hierarchy)
        queue = list(selected_ids)
        while queue:
            node_id = queue.pop(0)
            # Get all children of this node
            for child_id in self.parent_children.get(node_id, set()):
                if child_id not in result:
                    result.add(child_id)
                    queue.append(child_id)

        return result

    def _reconstruct_hierarchy(
        self, nodes: List[dict], selected_ids: Set[str]
    ) -> List[dict]:
        """Reconstruct hierarchical structure with only selected nodes."""
        result: List[dict] = []

        for node in nodes:
            filtered_node = self._filter_node_recursive(node, selected_ids)
            if filtered_node is not None:
                result.append(filtered_node)

        return result

    def _filter_node_recursive(
        self, node: dict, selected_ids: Set[str]
    ) -> Optional[dict]:
        """Recursively filter nodes, keeping only selected ones."""
        node_id = node.get("id")

        # If this node is not selected, skip it entirely
        if node_id not in selected_ids:
            return None

        # Make a deep copy of the node to preserve all fields
        filtered_node = copy.deepcopy(node)

        # Recursively filter children
        children = node.get("children", [])
        filtered_children: List[dict] = []

        for child in children:
            filtered_child = self._filter_node_recursive(child, selected_ids)
            if filtered_child is not None:
                filtered_children.append(filtered_child)

        filtered_node["children"] = filtered_children

        return filtered_node


def filter_by_state(graph: dict, states: Set[str]) -> dict:
    """
    Filter graph to only include nodes with change_status in states.
    Parent nodes are kept as structural context if any descendant matches.
    Ghost (deleted) nodes are included when "deleted" is in states.
    """

    def _node_or_desc_matches(node: dict) -> bool:
        if node.get("change_status") in states:
            return True
        return any(_node_or_desc_matches(c) for c in node.get("children", []))

    def _filter_nodes(nodes: List[dict]) -> List[dict]:
        result = []
        for node in nodes:
            if _node_or_desc_matches(node):
                filtered = dict(node)
                filtered["children"] = _filter_nodes(node.get("children", []))
                result.append(filtered)
        return result

    def _collect_ids(nodes: List[dict]) -> Set[str]:
        ids: Set[str] = set()
        for node in nodes:
            if "id" in node:
                ids.add(node["id"])
            ids.update(_collect_ids(node.get("children", [])))
        return ids

    filtered_nodes = _filter_nodes(graph.get("nodes", []))
    surviving_ids = _collect_ids(filtered_nodes)
    filtered_edges = [
        e
        for e in graph.get("edges", [])
        if (e.get("caller") or e.get("source")) in surviving_ids
        and (e.get("callee") or e.get("target")) in surviving_ids
    ]
    result: Dict[str, Any] = {"nodes": filtered_nodes, "edges": filtered_edges}
    if "metadata" in graph:
        result["metadata"] = graph["metadata"]
    return result
