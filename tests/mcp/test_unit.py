"""Unit tests for pure-function domain logic (no I/O)."""

import pytest

from serpentine.cache import NullCache
from serpentine.services import (
    MissingConfigError,
    NotIngestedError,
    UnknownRepoError,
    get_catalog,
    get_stats,
)


# ---------------------------------------------------------------------------
# NullCache
# ---------------------------------------------------------------------------

def test_null_cache_get_always_none():
    c = NullCache()
    assert c.get("abc123", "config") is None


def test_null_cache_put_is_noop():
    c = NullCache()
    c.put("abc123", "config", '{"nodes": []}')  # must not raise


# ---------------------------------------------------------------------------
# Domain exceptions
# ---------------------------------------------------------------------------

def test_not_ingested_error_message():
    exc = NotIngestedError("org/repo", "main")
    assert "org/repo" in str(exc)
    assert "main" in str(exc)
    assert exc.repo_id == "org/repo"
    assert exc.ref == "main"


def test_missing_config_error_message():
    exc = MissingConfigError("myrepo")
    assert "myrepo" in str(exc)
    assert exc.repo_id == "myrepo"


def test_unknown_repo_error_message():
    exc = UnknownRepoError("evil/repo")
    assert "evil/repo" in str(exc)
    assert exc.repo_id == "evil/repo"


# ---------------------------------------------------------------------------
# get_stats
# ---------------------------------------------------------------------------

EMPTY_GRAPH: dict = {"nodes": [], "edges": []}

SIMPLE_GRAPH: dict = {
    "nodes": [
        {
            "id": "mod.A",
            "name": "A",
            "object_type": "class",
            "origin": "local",
            "children": [
                {
                    "id": "mod.A.method",
                    "name": "method",
                    "object_type": "function",
                    "origin": "local",
                    "children": [],
                }
            ],
        }
    ],
    "edges": [
        {"source": "mod.A.method", "target": "mod.A", "type": "calls"},
        {"source": "mod.A.method", "target": "mod.A", "type": "calls"},
    ],
}


def test_get_stats_empty():
    stats = get_stats(EMPTY_GRAPH)
    assert stats["node_count"] == 0
    assert stats["edge_count"] == 0
    assert stats["nodes_by_type"] == {}
    assert stats["edges_by_type"] == {}


def test_get_stats_counts_nested():
    stats = get_stats(SIMPLE_GRAPH)
    assert stats["node_count"] == 2
    assert stats["nodes_by_type"]["class"] == 1
    assert stats["nodes_by_type"]["function"] == 1


def test_get_stats_edge_counts():
    stats = get_stats(SIMPLE_GRAPH)
    assert stats["edge_count"] == 2
    assert stats["edges_by_type"]["calls"] == 2


# ---------------------------------------------------------------------------
# get_catalog
# ---------------------------------------------------------------------------

CATALOG_GRAPH: dict = {
    "nodes": [
        {
            "id": "mod.A",
            "name": "A",
            "label": "A",
            "object_type": "class",
            "origin": "local",
            "parent": None,
            "file_path": "mod.py",
            "children": [
                {
                    "id": "mod.A.x",
                    "name": "x",
                    "label": "x",
                    "object_type": "assignment",
                    "origin": "local",
                    "parent": "mod.A",
                    "file_path": "mod.py",
                    "children": [],
                },
                {
                    "id": "mod.A.method",
                    "name": "method",
                    "label": "method",
                    "object_type": "function",
                    "origin": "local",
                    "parent": "mod.A",
                    "file_path": "mod.py",
                    "children": [],
                },
            ],
        },
        {
            "id": "os.path",
            "name": "path",
            "label": "path",
            "object_type": "module",
            "origin": "standard",
            "parent": None,
            "file_path": "",
            "children": [],
        },
    ],
    "edges": [],
}


def test_catalog_empty_graph():
    result = get_catalog(EMPTY_GRAPH)
    assert result == []


def test_catalog_excludes_assignments_by_default():
    result = get_catalog(CATALOG_GRAPH, include_standard=True, include_third_party=True)
    ids = [n["id"] for n in result]
    assert "mod.A.x" not in ids
    assert "mod.A" in ids
    assert "mod.A.method" in ids


def test_catalog_includes_assignments_when_asked():
    result = get_catalog(
        CATALOG_GRAPH,
        include_assignments=True,
        include_standard=True,
        include_third_party=True,
    )
    ids = [n["id"] for n in result]
    assert "mod.A.x" in ids


def test_catalog_includes_standard_by_default():
    result = get_catalog(CATALOG_GRAPH)
    ids = [n["id"] for n in result]
    assert "os.path" in ids


def test_catalog_excludes_standard_when_asked():
    result = get_catalog(CATALOG_GRAPH, include_standard=False)
    ids = [n["id"] for n in result]
    assert "os.path" not in ids


def test_catalog_filter_str():
    result = get_catalog(CATALOG_GRAPH, filter_str="mod.A.*", include_standard=True)
    ids = [n["id"] for n in result]
    assert "mod.A.method" in ids
    assert "mod.A" not in ids
