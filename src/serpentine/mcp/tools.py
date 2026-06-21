import json
import logging

from fastmcp import FastMCP

from serpentine.adapters import VcsSourceProvider
from serpentine.domain import (
    MissingConfigError,
    NotIngestedError,
    UnknownRepoError,
    get_graph,
    inject_source,
    ingest_ref,
)
from serpentine.storage.base import GraphStore
from serpentine.vcs.manager import VcsManager

logger = logging.getLogger(__name__)


def _recovery_message(exc: Exception) -> str:
    if isinstance(exc, NotIngestedError):
        return (
            f"{exc.repo_id}/{exc.ref} has not been ingested. "
            f"Run the ingest_ref tool or 'serpentine mcp ingest {exc.repo_id} {exc.ref}' "
            "in CI to populate the graph, then retry."
        )
    if isinstance(exc, MissingConfigError):
        return (
            f"No .serpentine.toml found in {exc.repo_id}. "
            "Add one to configure ingestion, or re-run with --ignore-config to use defaults."
        )
    if isinstance(exc, UnknownRepoError):
        return (
            f"{exc.repo_id} is not in the allowed repo list. "
            "Check SERPENTINE_ALLOWED_REPOS or SERPENTINE_REPOS_DIR."
        )
    return str(exc)


def register_tools(
    mcp: FastMCP,
    store: GraphStore,
    vcs_managers: dict[str, VcsManager],
) -> None:
    @mcp.tool()
    def list_repos() -> str:
        """List all available repo IDs that can be queried or ingested."""
        return json.dumps(sorted(vcs_managers.keys()), indent=2)

    @mcp.tool()
    def list_refs(repo_id: str) -> str:
        """List VCS refs (branches, tags, recent commits) for a repo."""
        if repo_id not in vcs_managers:
            return _recovery_message(UnknownRepoError(repo_id))
        refs = vcs_managers[repo_id].list_refs()
        return json.dumps([{"id": r.id, "display": r.display, "kind": r.kind} for r in refs], indent=2)

    @mcp.tool()
    def analyze(
        repo_id: str,
        ref: str,
        select: str | None = None,
        exclude: str | None = None,
        source: bool = False,
    ) -> str:
        """Return filtered graph data for a repo at a ref. Use source=true to inline source code for each node."""
        if repo_id not in vcs_managers:
            return _recovery_message(UnknownRepoError(repo_id))
        try:
            vcs = vcs_managers[repo_id]
            graph_data = get_graph(store, vcs, repo_id, ref, select=select, exclude=exclude)
            if source:
                inject_source(graph_data, VcsSourceProvider(vcs._backend, ref))
            else:
                _strip_code(graph_data.get("nodes", []))
            return json.dumps(graph_data, indent=2)
        except (NotIngestedError, UnknownRepoError) as e:
            return _recovery_message(e)

    @mcp.tool()
    def ingest_ref_tool(
        repo_id: str,
        ref: str,
        ignore_config: bool = False,
    ) -> str:
        """Trigger analysis of a repo at a ref and persist the graph to the store."""
        if repo_id not in vcs_managers:
            return _recovery_message(UnknownRepoError(repo_id))
        try:
            commit_hash = ingest_ref(
                vcs_managers[repo_id],
                store,
                repo_id,
                ref,
                ignore_config=ignore_config,
            )
            return f"Ingested {repo_id}/{ref} at commit {commit_hash[:7]}."
        except (MissingConfigError, UnknownRepoError) as e:
            return _recovery_message(e)


def _strip_code(nodes: list) -> None:
    for node in nodes:
        node.pop("code_block", None)
        _strip_code(node.get("children", []))
