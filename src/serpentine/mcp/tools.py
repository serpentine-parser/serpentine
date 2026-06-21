import json
import logging

from fastmcp import FastMCP

from serpentine.adapters import VcsSourceProvider
from serpentine.services import _validate_ref
from serpentine.domain import (
    MissingConfigError,
    NotIngestedError,
    UnknownRepoError,
    get_catalog,
    get_graph,
    get_stats,
    inject_source_on_demand,
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
        """List all repo IDs available on this server.

        Call this first to discover what repos you can query. Use the returned
        IDs in all other tools.
        """
        return json.dumps(sorted(vcs_managers.keys()), indent=2)

    @mcp.tool()
    def list_refs(repo_id: str) -> str:
        """List branches, tags, and recent commits for a repo.

        Use this to discover valid ref values before calling analyze or ingest_ref.
        """
        if repo_id not in vcs_managers:
            return _recovery_message(UnknownRepoError(repo_id))
        refs = vcs_managers[repo_id].list_refs()
        return json.dumps([{"id": r.id, "display": r.display, "kind": r.kind} for r in refs], indent=2)

    @mcp.tool()
    def catalog(repo_id: str, ref: str) -> str:
        """Get the flat node list for a repo at a ref.

        Call this before analyze to discover node IDs and build a selector.
        Returns every node with its id, name, object_type, and file_path.
        """
        if repo_id not in vcs_managers:
            return _recovery_message(UnknownRepoError(repo_id))
        try:
            vcs = vcs_managers[repo_id]
            _validate_ref(vcs, repo_id, ref)
            commit_hash = vcs._backend.resolve_to_commit_hash(ref)
            graph_json = store.get(repo_id, commit_hash)
            if graph_json is None:
                return _recovery_message(NotIngestedError(repo_id, ref))
            return json.dumps(get_catalog(json.loads(graph_json)), indent=2)
        except (NotIngestedError, UnknownRepoError) as e:
            return _recovery_message(e)

    @mcp.tool()
    def stats(repo_id: str, ref: str) -> str:
        """Get node and edge counts by type for a repo at a ref.

        Call this first to understand graph scale before fetching the catalog or
        running analyze. Returns node_count, edge_count, nodes_by_type, edges_by_type.
        """
        if repo_id not in vcs_managers:
            return _recovery_message(UnknownRepoError(repo_id))
        try:
            vcs = vcs_managers[repo_id]
            _validate_ref(vcs, repo_id, ref)
            commit_hash = vcs._backend.resolve_to_commit_hash(ref)
            graph_json = store.get(repo_id, commit_hash)
            if graph_json is None:
                return _recovery_message(NotIngestedError(repo_id, ref))
            import json as _json
            return _json.dumps(get_stats(_json.loads(graph_json)), indent=2)
        except (NotIngestedError, UnknownRepoError) as e:
            return _recovery_message(e)

    @mcp.tool()
    def analyze(
        repo_id: str,
        ref: str,
        select: str | None = None,
        exclude: str | None = None,
        source: bool = False,
    ) -> str:
        """Query the dependency graph for a repo at a ref.

        WORKFLOW — always follow this order:
        1. Call the `stats` tool to understand graph scale.
        2. Call the `catalog` tool to discover node IDs.
        3. Call this tool with a selector built from catalog IDs.

        If you get a "not ingested" error, call ingest_ref first, then retry.

        SELECTOR SYNTAX (dbt-style):
        - `*.ClassName`     — a specific symbol by name
        - `+*.Symbol`       — symbol + all its dependencies (upstream)
        - `*.Symbol+`       — symbol + everything that calls it (downstream)
        - `@*.Symbol`       — full connected component
        - `mod.sub.*`       — all nodes in a module
        - `*.A,*.B`         — union of multiple patterns

        Use source=true to inline the actual source code for each node in the result.
        Only use source=true after narrowing with a selector — it fetches files on demand.
        """
        if repo_id not in vcs_managers:
            return _recovery_message(UnknownRepoError(repo_id))
        try:
            vcs = vcs_managers[repo_id]
            graph_data = get_graph(store, vcs, repo_id, ref, select=select, exclude=exclude)
            if source:
                inject_source_on_demand(graph_data, VcsSourceProvider(vcs._backend, ref))
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
        """Analyze a repo at a ref and store the graph for querying.

        Run this before calling analyze on a ref that has not been ingested yet.
        The repo must have a .serpentine.toml in its root; pass ignore_config=true
        to skip that requirement and use default settings.

        After ingestion, call analyze to query the graph.
        """
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
