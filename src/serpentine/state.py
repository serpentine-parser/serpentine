"""
Graph state management module.

Responsibilities:
- Maintaining the current dependency graph state
- Coordinating with the Rust analyzer for parsing
- Notifying listeners when state changes

This module acts as the central source of truth for the graph data,
decoupled from both the file watcher (input) and the web server (output).
"""

import json
import logging
import threading
import time
from pathlib import Path
from typing import Any, Callable

from serpentine.cache import CacheManager
from serpentine.config import Config

logger = logging.getLogger(__name__)


class GraphStateManager:
    """
    Manages the dependency graph state and coordinates analysis.

    This class is thread-safe and can be shared between the file
    watcher (which triggers updates) and the web server (which
    reads state and broadcasts updates).

    Usage:
        state = GraphStateManager()
        state.set_broadcast_callback(lambda: broadcast_to_clients())
        state.analyze_project(Path("./my-project"))

        # Get current state
        graph_json = state.get_graph_json()
        graph_data = state.get_graph_data()

    The state manager handles all interaction with the Rust analyzer,
    keeping that complexity encapsulated.
    """

    def __init__(self, project_path: Path | None = None) -> None:
        self._graph_json: str = '{"nodes": [], "edges": [], "metadata": {}}'
        self._graph_data: dict[str, Any] = {"nodes": [], "edges": [], "metadata": {}}
        self._previous_graph_data: dict[str, Any] = {"nodes": [], "edges": []}
        self._ghost_nodes: dict[str, Any] = {}
        self._node_change_status: dict[str, str] = {}
        self._edge_change_status: dict[tuple[str, str], str] = {}
        self._deleted_edge_data: dict[tuple[str, str], dict[str, Any]] = {}
        self._lock = threading.RLock()
        self._broadcast_callback: Callable[[], None] | None = None
        self._analyzer: Any = None  # The Rust FileManager

        # Load configuration
        self._config = Config.load(project_path or Path.cwd())

    @property
    def config(self) -> Config:
        """The loaded configuration for this project."""
        return self._config

    def set_broadcast_callback(self, callback: Callable[[], None]) -> None:
        """
        Set a callback to be invoked when the graph state changes.

        This is typically used to trigger WebSocket broadcasts.
        """
        self._broadcast_callback = callback

    def analyze_project(self, project_path: Path, changed_files: dict[str, str] | None = None) -> None:
        """
        Analyze a project directory and update the graph state.

        This method:
        1. On initial analysis (changed_files=None): checks disk cache; returns immediately on hit.
        2. On file-watch re-analysis (changed_files provided) with a live analyzer: uses the
           incremental Rust API (update_file/close_file) to skip re-parsing unchanged files.
        3. Falls back to full analysis when no persistent analyzer exists.
        4. Saves the result to the disk cache so subsequent CLI invocations can skip analysis.

        Args:
            project_path: Path to the project root directory
            changed_files: Map of {path: event_type} from the file watcher, or None for initial analysis
        """
        with self._lock:
            try:
                self._previous_graph_data = self._graph_data.copy()

                # 2A: Check disk cache for initial (full) analysis
                if changed_files is None:
                    source_files = self._find_source_files(project_path)
                    cache = CacheManager(project_path, project_path / ".serpentine.toml")
                    fp = cache.compute_fingerprint(source_files)
                    cached_json = cache.load(fp)
                    if cached_json is not None:
                        self._update_state(cached_json)
                        logger.info(
                            f"Analysis complete (cached): {self.node_count} nodes, {self.edge_count} edges"
                        )
                        if self._broadcast_callback:
                            self._broadcast_callback()
                        return

                # 2B: Use incremental Rust API when analyzer is already loaded
                if changed_files is not None and self._analyzer is not None:
                    self._do_incremental_analysis(project_path, changed_files)
                else:
                    self._do_analysis(project_path)

                # Persist result to disk cache for future CLI invocations
                try:
                    source_files = self._find_source_files(project_path)
                    cache = CacheManager(project_path, project_path / ".serpentine.toml")
                    fp = cache.compute_fingerprint(source_files)
                    cache.save(fp, self._graph_json)
                except Exception as cache_err:
                    logger.warning(f"Cache save failed: {cache_err}")

                if changed_files is not None:
                    self._compute_change_status(changed_files)

                logger.info(
                    f"Analysis complete: {self.node_count} nodes, {self.edge_count} edges"
                )

                # Notify listeners
                if self._broadcast_callback:
                    self._broadcast_callback()

            except Exception as e:
                logger.error(f"Analysis failed: {e}")
                raise

    def _do_analysis(self, project_path: Path) -> None:
        """Internal method to perform the actual analysis."""
        try:
            from serpentine import _analyzer

            t0 = time.perf_counter()

            # Create a fresh analyzer for each analysis
            # This ensures clean state and avoids stale file issues
            self._analyzer = _analyzer.FileManager()

            # Find and load all source files
            source_files = self._find_source_files(project_path)
            logger.info(f"[perf] found {len(source_files)} source files ({time.perf_counter() - t0:.3f}s)")

            t_read = time.perf_counter()
            file_pairs: list[tuple[str, str]] = []
            for file_path in source_files:
                try:
                    source = file_path.read_text(encoding="utf-8")
                    file_pairs.append((str(file_path), source))
                except Exception as e:
                    logger.warning(f"Failed to read {file_path}: {e}")
            logger.info(f"[perf] file I/O: {time.perf_counter() - t_read:.3f}s")

            t_parse = time.perf_counter()
            try:
                self._analyzer.open_files_bulk(file_pairs)
            except Exception as e:
                logger.warning(f"Bulk open failed, falling back to serial: {e}")
                for path, source in file_pairs:
                    try:
                        self._analyzer.open_file(path, source)
                    except Exception as e2:
                        logger.warning(f"Failed to load {path}: {e2}")
            logger.info(f"[perf] parse (open_files_bulk): {time.perf_counter() - t_parse:.3f}s")

            # Build the dependency graph
            t_graph = time.perf_counter()
            graph_json = self._analyzer.build_dependency_graph()
            logger.info(f"[perf] build_dependency_graph: {time.perf_counter() - t_graph:.3f}s")

            t_state = time.perf_counter()
            self._update_state(graph_json)
            logger.info(f"[perf] update_state: {time.perf_counter() - t_state:.3f}s")
            logger.info(f"[perf] total analysis: {time.perf_counter() - t0:.3f}s")

        except ImportError:
            logger.warning(
                "Rust analyzer not available, using mock data. "
                "Run 'maturin develop' to build the analyzer."
            )
            self._update_state(self._get_mock_graph())

    def _do_incremental_analysis(self, project_path: Path, changed_files: dict[str, str]) -> None:
        """Apply incremental file updates to the persistent analyzer and rebuild the graph."""
        t0 = time.perf_counter()

        for path_str, event_type in changed_files.items():
            path = Path(path_str)
            if event_type == "deleted" or event_type == "moved":
                try:
                    self._analyzer.close_file(path_str)
                except Exception as e:
                    logger.warning(f"close_file failed for {path_str}: {e}")
            else:
                # created, modified, closed, or any other write event
                try:
                    content = path.read_text(encoding="utf-8")
                    if event_type == "created":
                        self._analyzer.open_file(path_str, content)
                    else:
                        try:
                            self._analyzer.update_file(path_str, content)
                        except KeyError:
                            # File not yet known to the analyzer (e.g. debouncer
                            # swallowed the "created" event); treat as new.
                            logger.debug(f"update_file: {path_str} not found, falling back to open_file")
                            self._analyzer.open_file(path_str, content)
                except Exception as e:
                    logger.warning(f"Incremental update failed for {path_str} ({event_type}): {e}")

        logger.info(f"[perf] incremental file updates: {time.perf_counter() - t0:.3f}s")

        t_graph = time.perf_counter()
        graph_json = self._analyzer.build_dependency_graph()
        logger.info(f"[perf] build_dependency_graph: {time.perf_counter() - t_graph:.3f}s")

        self._update_state(graph_json)

    def _flatten_nodes(self, nodes: list[Any], inherited_origin: str | None = None, parent_id: str | None = None) -> dict[str, Any]:
        """Recursively flatten hierarchical node list to {id: node_data}, propagating origin and parent to children."""
        result: dict[str, Any] = {}
        for node in nodes:
            node_id = node.get("id")
            node_origin = node.get("origin") or inherited_origin
            stored = dict(node)
            if node_origin and not node.get("origin"):
                stored["origin"] = node_origin
            if parent_id and not stored.get("parent"):
                stored["parent"] = parent_id
            if node_id:
                result[node_id] = stored
            result.update(self._flatten_nodes(node.get("children", []), inherited_origin=node_origin, parent_id=node_id))
        return result

    def _compute_change_status(self, changed_files: dict[str, str]) -> None:
        """Diff old vs new graph nodes and update change status and ghost nodes."""
        old = self._flatten_nodes(self._previous_graph_data.get("nodes", []))
        new = self._flatten_nodes(self._graph_data.get("nodes", []))

        # Only track changes for local nodes; stdlib/third-party IDs can fluctuate
        # between analyses producing false positives
        local_old = {k: v for k, v in old.items() if v.get("origin", "local") == "local"}
        local_new = {k: v for k, v in new.items() if v.get("origin", "local") == "local"}

        # Ghost cancel-out: nodes that reappear after being a ghost
        net_no_change: set[str] = set()
        for node_id in list(self._ghost_nodes.keys()):
            if node_id in local_new:
                ghost_hash = self._ghost_nodes[node_id].get("content_hash")
                new_hash = local_new[node_id].get("content_hash")
                if ghost_hash and new_hash:
                    is_exact = ghost_hash == new_hash
                else:
                    ghost_code = self._ghost_nodes[node_id].get("code_block")
                    new_code = local_new[node_id].get("code_block")
                    is_exact = ghost_code is not None and ghost_code == new_code
                if is_exact:
                    # Exact restoration — net no change
                    self._ghost_nodes.pop(node_id)
                    self._node_change_status.pop(node_id, None)
                    net_no_change.add(node_id)
                else:
                    # Re-added with different content — modified
                    self._ghost_nodes.pop(node_id)
                    self._node_change_status[node_id] = "modified"

        # Added: in new local, not in old local (and not already handled above)
        for node_id in local_new:
            if node_id not in local_old and node_id not in self._node_change_status and node_id not in net_no_change:
                self._node_change_status[node_id] = "added"

        # Deleted: in old local, not in new → becomes ghost
        for node_id, node_data in local_old.items():
            if node_id not in local_new:
                self._node_change_status[node_id] = "deleted"
                self._ghost_nodes[node_id] = {**node_data, "change_status": "deleted", "isGhost": True}

        # Modified: in both, content_hash differs (fall back to code_block if no hash)
        for node_id in local_new:
            if node_id in local_old and self._node_change_status.get(node_id) not in ("added", "deleted"):
                old_hash = local_old[node_id].get("content_hash")
                new_hash = local_new[node_id].get("content_hash")
                if old_hash and new_hash:
                    if old_hash != new_hash:
                        self._node_change_status[node_id] = "modified"
                else:
                    old_code = local_old[node_id].get("code_block")
                    new_code = local_new[node_id].get("code_block")
                    if old_code is not None and new_code is not None and old_code != new_code:
                        self._node_change_status[node_id] = "modified"

        # Edge diffing: compare old vs new edge sets
        old_edges = {(e["caller"], e["callee"]): e for e in self._previous_graph_data.get("edges", [])}
        new_edges = {(e["caller"], e["callee"]): e for e in self._graph_data.get("edges", [])}

        for edge_key in new_edges:
            if edge_key not in old_edges:
                self._edge_change_status[edge_key] = "added"

        for edge_key, edge_data in old_edges.items():
            if edge_key not in new_edges:
                self._edge_change_status[edge_key] = "deleted"
                self._deleted_edge_data[edge_key] = edge_data

        # Clear stale edge statuses for edges that are now stable (in both, not changed)
        for edge_key in list(self._edge_change_status.keys()):
            if edge_key in new_edges and edge_key in old_edges:
                self._edge_change_status.pop(edge_key, None)
                self._deleted_edge_data.pop(edge_key, None)

    def _inject_change_status(self, graph_data: dict[str, Any]) -> dict[str, Any]:
        """Attach change_status to nodes and append ghost nodes before returning graph data."""
        if not self._node_change_status and not self._ghost_nodes and not self._edge_change_status:
            return graph_data

        # Collect surviving node IDs so we know which parents are still alive
        surviving_ids: set[str] = set()

        def _collect_ids(nodes: list[Any]) -> None:
            for node in nodes:
                surviving_ids.add(node.get("id", ""))
                _collect_ids(node.get("children", []))

        _collect_ids(graph_data.get("nodes", []))

        # Group ghosts by parent: ghosts whose parent is alive or also a ghost go under
        # that parent; ghosts with no parent or an unknown parent go at the top level
        ghost_ids: set[str] = set(self._ghost_nodes.keys())
        ghosts_by_parent: dict[str, list[dict[str, Any]]] = {}
        top_level_ghosts: list[dict[str, Any]] = []
        for ghost in self._ghost_nodes.values():
            parent_id = ghost.get("parent")
            if parent_id and (parent_id in surviving_ids or parent_id in ghost_ids):
                ghosts_by_parent.setdefault(parent_id, []).append(ghost)
            else:
                top_level_ghosts.append(ghost)

        def _attach_ghost_children(ghost: dict[str, Any]) -> dict[str, Any]:
            ghost_id = ghost.get("id")
            if ghost_id and ghost_id in ghosts_by_parent:
                ghost = dict(ghost)
                ghost["children"] = ghost.get("children", []) + [
                    _attach_ghost_children(c) for c in ghosts_by_parent[ghost_id]
                ]
            return ghost

        def _annotate_nodes(nodes: list[Any]) -> list[Any]:
            result = []
            for node in nodes:
                node_id = node.get("id")
                annotated = dict(node)
                if node_id and node_id in self._node_change_status:
                    annotated["change_status"] = self._node_change_status[node_id]
                annotated["children"] = _annotate_nodes(node.get("children", []))
                if node_id and node_id in ghosts_by_parent:
                    annotated["children"] = annotated["children"] + [
                        _attach_ghost_children(g) for g in ghosts_by_parent[node_id]
                    ]
                # Bubble up: if any child has a change_status, mark this node modified
                if not annotated.get("change_status"):
                    if any(c.get("change_status") for c in annotated["children"]):
                        annotated["change_status"] = "modified"
                result.append(annotated)
            return result

        enriched = dict(graph_data)
        enriched["nodes"] = _annotate_nodes(graph_data.get("nodes", []))
        enriched["nodes"] = enriched["nodes"] + [
            _attach_ghost_children(g) for g in top_level_ghosts
        ]

        # Annotate surviving edges with their change_status
        annotated_edges = []
        surviving_edge_keys = {(e["caller"], e["callee"]) for e in enriched.get("edges", [])}

        for edge in enriched.get("edges", []):
            edge_key = (edge["caller"], edge["callee"])
            annotated = dict(edge)
            if edge_key in self._edge_change_status:
                annotated["change_status"] = self._edge_change_status[edge_key]
            annotated_edges.append(annotated)

        # Inject deleted edges (they no longer appear in the live graph)
        for edge_key, edge_data in self._deleted_edge_data.items():
            if edge_key not in surviving_edge_keys:
                annotated_edges.append({**edge_data, "change_status": "deleted"})

        enriched["edges"] = annotated_edges
        return enriched

    def dismiss_change(self, node_id: str) -> None:
        """Remove change status and ghost entry for a single node."""
        self._node_change_status.pop(node_id, None)
        self._ghost_nodes.pop(node_id, None)
        for edge_key in list(self._edge_change_status.keys()):
            if node_id in edge_key:
                self._edge_change_status.pop(edge_key, None)
                self._deleted_edge_data.pop(edge_key, None)

    def dismiss_all_changes(self) -> None:
        """Clear all change statuses and ghost nodes."""
        self._node_change_status.clear()
        self._ghost_nodes.clear()
        self._edge_change_status.clear()
        self._deleted_edge_data.clear()

    def _find_source_files(self, project_path: Path) -> list[Path]:
        """Find all supported source files in a project."""
        files: list[Path] = []

        for ext in self._config.extensions:
            for file_path in project_path.rglob(f"*{ext}"):
                # Skip excluded directories
                if any(
                    part.startswith(".") or part in self._config.exclude_dirs
                    for part in file_path.parts
                ):
                    continue
                files.append(file_path)

        return sorted(files)

    def _update_state(self, graph_json: str) -> None:
        """Update the internal state with new graph data."""
        self._graph_json = graph_json
        try:
            self._graph_data = json.loads(graph_json)
        except json.JSONDecodeError:
            logger.error("Failed to parse graph JSON")
            self._graph_data = {"nodes": [], "edges": [], "metadata": {}}

    def get_graph_json(self) -> str:
        """Get the current graph state as a JSON string."""
        with self._lock:
            return json.dumps(self._inject_change_status(self._graph_data))

    def get_graph_data(self) -> dict[str, Any]:
        """Get the current graph state as a Python dict."""
        with self._lock:
            return self._inject_change_status(self._graph_data.copy())

    @property
    def node_count(self) -> int:
        """Number of nodes in the current graph."""
        metadata = self._graph_data.get("metadata", {})
        return metadata.get("node_count", len(self._graph_data.get("nodes", [])))

    @property
    def edge_count(self) -> int:
        """Number of edges in the current graph."""
        metadata = self._graph_data.get("metadata", {})
        return metadata.get("edge_count", len(self._graph_data.get("edges", [])))

    def _get_mock_graph(self) -> str:
        """Return mock graph data for development without the Rust analyzer."""
        return json.dumps(
            {
                "nodes": [
                    {
                        "id": "mock_module",
                        "name": "mock_module",
                        "object_type": "module",
                        "origin": "local",
                        "children": [
                            {
                                "id": "mock_module.MockClass",
                                "name": "MockClass",
                                "object_type": "class",
                                "children": [
                                    {
                                        "id": "mock_module.MockClass.method",
                                        "name": "method",
                                        "object_type": "function",
                                        "children": [],
                                    }
                                ],
                            },
                            {
                                "id": "mock_module.main",
                                "name": "main",
                                "object_type": "function",
                                "children": [],
                            },
                        ],
                    }
                ],
                "edges": [
                    {
                        "caller": "mock_module.main",
                        "callee": "mock_module.MockClass",
                        "type": "calls",
                        "filename": "mock_module.py",
                    }
                ],
                "metadata": {
                    "node_count": 4,
                    "edge_count": 1,
                    "node_types": {"module": 1, "class": 1, "function": 2},
                },
            }
        )
