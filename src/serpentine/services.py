"""
Serpentine domain services.

Pure graph operations consumed by all entrypoints (CLI, HTTP API, MCP).
No I/O, no framework concerns. Entrypoints are thin adapters over these functions.
"""

import json
import logging
from typing import Any, Protocol

from serpentine.config import Config
from serpentine.selector import GraphSelector, filter_by_state
from serpentine.storage.base import GraphStore
from serpentine.vcs.manager import VcsManager

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Domain exceptions
# ---------------------------------------------------------------------------

class NotIngestedError(Exception):
    def __init__(self, repo_id: str, ref: str) -> None:
        self.repo_id = repo_id
        self.ref = ref
        super().__init__(f"{repo_id}/{ref} has not been ingested")


class MissingConfigError(Exception):
    def __init__(self, repo_id: str) -> None:
        self.repo_id = repo_id
        super().__init__(f"No .serpentine.toml found in {repo_id}")


class UnknownRepoError(Exception):
    def __init__(self, repo_id: str) -> None:
        self.repo_id = repo_id
        super().__init__(f"{repo_id} is not in the allowed repo list")


# ---------------------------------------------------------------------------
# SourceProvider protocol
# ---------------------------------------------------------------------------

class SourceProvider(Protocol):
    """Abstracts file content retrieval for source injection."""
    def get_file(self, path: str) -> str | None: ...


# ---------------------------------------------------------------------------
# Pure graph operations
# ---------------------------------------------------------------------------

def filter_by_origin(
    graph: dict[str, Any],
    include_standard: bool,
    include_third_party: bool,
) -> dict[str, Any]:
    """Filter graph nodes by origin. Excluded nodes and their children are dropped."""
    def _keep(node: dict[str, Any]) -> bool:
        origin = node.get("origin") or "local"
        if origin == "standard" and not include_standard:
            return False
        if origin == "third-party" and not include_third_party:
            return False
        return True

    def _filter_nodes(nodes: list[dict[str, Any]]) -> list[dict[str, Any]]:
        result = []
        for node in nodes:
            if not _keep(node):
                continue
            filtered = dict(node)
            filtered["children"] = _filter_nodes(node.get("children", []))
            result.append(filtered)
        return result

    filtered_nodes = _filter_nodes(graph.get("nodes", []))
    surviving: set[str] = set()

    def _collect(nodes: list[dict[str, Any]]) -> None:
        for n in nodes:
            surviving.add(n["id"])
            _collect(n.get("children", []))

    _collect(filtered_nodes)
    filtered_edges = [
        e for e in graph.get("edges", [])
        if (e.get("source") or e.get("caller")) in surviving
        and (e.get("target") or e.get("callee")) in surviving
    ]
    result: dict[str, Any] = {"nodes": filtered_nodes, "edges": filtered_edges}
    if "metadata" in graph:
        result["metadata"] = graph["metadata"]
    return result


def apply_filters(
    graph_data: dict[str, Any],
    *,
    select: str | None = None,
    exclude: str | None = None,
    state: str | None = None,
    include_standard: bool = False,
    include_third_party: bool = False,
) -> dict[str, Any]:
    """Apply origin filter, selector, and state filter to a graph dict."""
    if not include_standard or not include_third_party:
        graph_data = filter_by_origin(graph_data, include_standard, include_third_party)

    if select or exclude:
        graph_data = GraphSelector.resolve(
            graph_data,
            select=select or "",
            exclude=exclude or "",
        )

    if state:
        states = {s.strip() for s in state.split(",") if s.strip()}
        graph_data = filter_by_state(graph_data, states)

    return graph_data


def inject_source(graph_data: dict[str, Any], provider: SourceProvider) -> None:
    """Inject code_block into each node by fetching file contents via provider."""
    file_cache: dict[str, list[str]] = {}

    def _walk(nodes: list[dict[str, Any]]) -> None:
        for node in nodes:
            file_path = node.get("file_path")
            position = node.get("position")
            if file_path and position is not None:
                if file_path not in file_cache:
                    content = provider.get_file(file_path)
                    file_cache[file_path] = content.splitlines() if content else []
                lines = file_cache[file_path]
                if lines:
                    node["code_block"] = "\n".join(lines[position[0]:position[1]])
            _walk(node.get("children", []))

    _walk(graph_data.get("nodes", []))


def get_catalog(
    graph_data: dict[str, Any],
    *,
    filter_str: str | None = None,
    include_assignments: bool = False,
    include_standard: bool = True,
    include_third_party: bool = True,
    state: str | None = None,
) -> list[dict[str, Any]]:
    """Flat node list from graph_data, optionally filtered."""
    import fnmatch

    if state:
        states = {s.strip() for s in state.split(",") if s.strip()}
        graph_data = filter_by_state(graph_data, states)

    nodes: list[dict[str, Any]] = []

    def _walk(node_list: list[dict[str, Any]]) -> None:
        for node in node_list:
            origin = node.get("origin", "local")
            if origin == "standard" and not include_standard:
                _walk(node.get("children", []))
                continue
            if origin == "third-party" and not include_third_party:
                _walk(node.get("children", []))
                continue
            object_type = node.get("object_type", "")
            if not include_assignments and object_type in {"assignment", "unknown"}:
                _walk(node.get("children", []))
                continue
            entry = {
                "id": node.get("id", ""),
                "name": node.get("name", ""),
                "label": node.get("label") or node.get("name", ""),
                "object_type": object_type,
                "type": node.get("type") or object_type,
                "origin": origin,
                "parent": node.get("parent"),
                "file_path": node.get("file_path", ""),
            }
            if filter_str is None or fnmatch.fnmatch(entry["id"], filter_str):
                nodes.append(entry)
            _walk(node.get("children", []))

    _walk(graph_data.get("nodes", []))
    return nodes


def get_stats(graph_data: dict[str, Any]) -> dict[str, Any]:
    """Node/edge counts by type."""
    type_counts: dict[str, int] = {}

    def _walk(node_list: list[dict[str, Any]]) -> None:
        for node in node_list:
            t = node.get("object_type", "unknown")
            type_counts[t] = type_counts.get(t, 0) + 1
            _walk(node.get("children", []))

    _walk(graph_data.get("nodes", []))
    edges = graph_data.get("edges", [])
    edge_type_counts: dict[str, int] = {}
    for e in edges:
        t = e.get("type", "unknown")
        edge_type_counts[t] = edge_type_counts.get(t, 0) + 1

    return {
        "node_count": sum(type_counts.values()),
        "edge_count": len(edges),
        "nodes_by_type": type_counts,
        "edges_by_type": edge_type_counts,
    }


# ---------------------------------------------------------------------------
# MCP-specific: store-backed graph retrieval and ingestion
# ---------------------------------------------------------------------------

def get_graph(
    store: GraphStore,
    vcs: VcsManager,
    repo_id: str,
    ref: str,
    *,
    select: str | None = None,
    exclude: str | None = None,
    include_standard: bool = False,
    include_third_party: bool = False,
) -> dict[str, Any]:
    """Resolve ref → commit hash via vcs, load graph from store, apply filters."""
    commit_hash = vcs._backend.resolve_to_commit_hash(ref)
    graph_json = store.get(repo_id, commit_hash)
    if graph_json is None:
        raise NotIngestedError(repo_id, ref)

    graph_data: dict[str, Any] = json.loads(graph_json)
    return apply_filters(
        graph_data,
        select=select,
        exclude=exclude,
        include_standard=include_standard,
        include_third_party=include_third_party,
    )


def _config_from_toml_bytes(data: bytes) -> Config:
    try:
        import tomllib
    except ImportError:
        import tomli as tomllib  # type: ignore[no-redef]
    return Config(tomllib.loads(data.decode("utf-8")))


def ingest_ref(
    vcs: VcsManager,
    store: GraphStore,
    repo_id: str,
    ref: str,
    *,
    ignore_config: bool = False,
) -> str:
    """Fetch config from repo, analyze at ref, persist graph to store. Returns commit hash."""
    config_bytes = vcs._backend.get_config_file(ref)
    if config_bytes is None and not ignore_config:
        raise MissingConfigError(repo_id)
    if config_bytes is not None:
        vcs._config = _config_from_toml_bytes(config_bytes)

    commit_hash = vcs._backend.resolve_to_commit_hash(ref)
    logger.info(f"[mcp] ingesting {repo_id} at {ref} ({commit_hash[:7]})")
    graph_json = vcs.get_graph_at(ref)
    store.put(repo_id, commit_hash, graph_json)
    logger.info(f"[mcp] stored {repo_id}/{commit_hash[:7]}")
    return commit_hash
