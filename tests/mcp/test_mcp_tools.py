"""FastMCP in-process client tests for tools and resources."""

import asyncio
import json

import pytest
from fastmcp.client import Client, FastMCPTransport

from serpentine.mcp.server import create_mcp_app
from serpentine.services import ingest_ref


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _call(mcp_app, tool: str, args: dict):
    async def _run():
        async with Client(FastMCPTransport(mcp_app)) as c:
            return await c.call_tool(tool, args)
    return asyncio.run(_run())


def _resource(mcp_app, uri: str):
    async def _run():
        async with Client(FastMCPTransport(mcp_app)) as c:
            return await c.read_resource(uri)
    return asyncio.run(_run())


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------

@pytest.fixture
def app_no_data(store, vcs, git_repo):
    """MCP app with a repo registered but nothing ingested yet."""
    return create_mcp_app(store, {"myrepo": vcs}, auth=None)


@pytest.fixture
def app_with_data(store, vcs, git_repo):
    """MCP app with myrepo already ingested at main."""
    ingest_ref(vcs, store, "myrepo", "main", ignore_config=True)
    return create_mcp_app(store, {"myrepo": vcs}, auth=None)


# ---------------------------------------------------------------------------
# list_repos
# ---------------------------------------------------------------------------

def test_list_repos(app_no_data):
    result = _call(app_no_data, "list_repos", {})
    repo_ids = json.loads(result.data)
    assert "myrepo" in repo_ids


# ---------------------------------------------------------------------------
# list_refs
# ---------------------------------------------------------------------------

def test_list_refs_known_repo(app_no_data):
    result = _call(app_no_data, "list_refs", {"repo_id": "myrepo"})
    refs = json.loads(result.data)
    assert isinstance(refs, list)
    assert len(refs) > 0
    assert any("main" in r["display"] for r in refs)


def test_list_refs_unknown_repo(app_no_data):
    result = _call(app_no_data, "list_refs", {"repo_id": "../../etc/passwd"})
    assert "not in the allowed repo list" in result.data


# ---------------------------------------------------------------------------
# analyze
# ---------------------------------------------------------------------------

def test_analyze_auto_ingests_when_not_yet_ingested(app_no_data):
    # Auto-ingest: calling analyze on an un-ingested ref should succeed, not error
    result = _call(app_no_data, "analyze", {"repo_id": "myrepo", "ref": "main"})
    graph = json.loads(result.data)
    assert "nodes" in graph


def test_analyze_after_ingest_returns_graph(app_with_data):
    result = _call(app_with_data, "analyze", {"repo_id": "myrepo", "ref": "main"})
    graph = json.loads(result.data)
    assert "nodes" in graph
    assert len(graph["nodes"]) > 0


def test_analyze_unknown_repo(app_no_data):
    result = _call(app_no_data, "analyze", {"repo_id": "unknown", "ref": "main"})
    assert "not in the allowed repo list" in result.data


# ---------------------------------------------------------------------------
# ingest_ref tool
# ---------------------------------------------------------------------------

def test_ingest_ref_tool_stores_graph(app_no_data, store, vcs):
    result = _call(app_no_data, "ingest_ref_tool", {
        "repo_id": "myrepo",
        "ref": "main",
        "ignore_config": True,
    })
    assert "Ingested" in result.data

    # Subsequent analyze should now succeed
    app2 = create_mcp_app(store, {"myrepo": vcs}, auth=None)
    result2 = _call(app2, "analyze", {"repo_id": "myrepo", "ref": "main"})
    graph = json.loads(result2.data)
    assert "nodes" in graph


def test_ingest_ref_tool_unknown_repo(app_no_data):
    result = _call(app_no_data, "ingest_ref_tool", {
        "repo_id": "ghost",
        "ref": "main",
    })
    assert "not in the allowed repo list" in result.data


# ---------------------------------------------------------------------------
# Resources: catalog and stats
# ---------------------------------------------------------------------------

def test_catalog_resource(app_with_data):
    contents = _resource(app_with_data, "serpentine://myrepo/main/catalog")
    nodes = json.loads(contents[0].text)
    assert isinstance(nodes, list)
    assert len(nodes) > 0


def test_stats_resource(app_with_data):
    contents = _resource(app_with_data, "serpentine://myrepo/main/stats")
    stats = json.loads(contents[0].text)
    assert "node_count" in stats
    assert stats["node_count"] > 0
    assert "edge_count" in stats
