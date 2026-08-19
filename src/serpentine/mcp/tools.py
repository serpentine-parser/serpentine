import json
import logging

from fastmcp import FastMCP

from serpentine.adapters import VcsSourceProvider
from serpentine.domain import (
    MissingConfigError,
    NotIngestedError,
    UnknownRepoError,
    get_catalog,
    get_graph,
    get_stats,
    ingest_ref,
    inject_source_on_demand,
)
from serpentine.services import _validate_ref
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


def _ensure_ingested(vcs: VcsManager, store: GraphStore, repo_id: str, ref: str) -> str:
    """Resolve ref to commit hash, auto-ingest if not yet stored. Returns commit hash."""
    _validate_ref(vcs, repo_id, ref)
    commit_hash = vcs._backend.resolve_to_commit_hash(ref)
    if store.get(repo_id, commit_hash) is None:
        logger.info(f"[mcp] auto-ingesting {repo_id}/{ref} ({commit_hash[:7]})")
        ingest_ref(vcs, store, repo_id, ref, ignore_config=True)
    return commit_hash


def register_tools(
    mcp: FastMCP,
    store: GraphStore,
    vcs_managers: dict[str, VcsManager],
) -> None:
    @mcp.tool()
    def list_repos() -> str:
        """List all repo IDs available on this server.

        Call this first to see what's available, then call get_repo with a
        specific repo_id to see its refs and ingestion status.
        """
        return json.dumps(sorted(vcs_managers.keys()))

    @mcp.tool()
    def get_repo(repo_id: str, since: str | None = None, until: str | None = None) -> str:
        """Get available refs and ingestion status for a single repo.

        Returns each ref's ID, display name, kind, timestamp, and whether it's
        already ingested (ready to query immediately without waiting for ingestion).

        since/until optionally filter refs by commit timestamp, as ISO 8601 dates
        (e.g. "2026-08-01"). Refs with no timestamp (e.g. GitHub branch/tag refs,
        which aren't dated without an extra API call per ref) are never filtered out.
        """
        if repo_id not in vcs_managers:
            return _recovery_message(UnknownRepoError(repo_id))

        vcs = vcs_managers[repo_id]
        ingested = store.list_ingested(repo_id)
        ingested_by_hash = {r["commit_hash"]: r["ingested_at"] for r in ingested}

        try:
            fetched_refs = vcs.list_refs(since=since, until=until)
        except ValueError as e:
            return str(e)

        refs = []
        for r in fetched_refs:
            commit_hash = r.commit_hash
            if commit_hash is not None:
                refs.append(
                    {
                        "ref": r.id,
                        "display": r.display,
                        "kind": r.kind,
                        "commit_hash": commit_hash[:7],
                        "timestamp": r.timestamp,
                        "ingested": commit_hash in ingested_by_hash,
                        "ingested_at": ingested_by_hash.get(commit_hash),
                    }
                )
            else:
                refs.append(
                    {
                        "ref": r.id,
                        "display": r.display,
                        "kind": r.kind,
                        "timestamp": r.timestamp,
                        "ingested": False,
                    }
                )

        return json.dumps({"repo_id": repo_id, "refs": refs}, indent=2)

    @mcp.tool()
    def list_refs(repo_id: str, since: str | None = None, until: str | None = None) -> str:
        """List branches, tags, and recent commits for a repo.

        Use this to discover valid ref values before calling analyze or ingest_ref.

        since/until optionally filter refs by commit timestamp, as ISO 8601 dates
        (e.g. "2026-08-01"). Refs with no timestamp (e.g. GitHub branch/tag refs,
        which aren't dated without an extra API call per ref) are never filtered out.
        """
        if repo_id not in vcs_managers:
            return _recovery_message(UnknownRepoError(repo_id))
        try:
            refs = vcs_managers[repo_id].list_refs(since=since, until=until)
        except ValueError as e:
            return str(e)
        return json.dumps(
            [{"id": r.id, "display": r.display, "kind": r.kind, "timestamp": r.timestamp} for r in refs],
            indent=2,
        )

    @mcp.tool()
    def catalog(repo_id: str, ref: str, filter: str | None = None) -> str:
        """Get the flat node list for a repo at a ref.

        Call this before analyze to discover node IDs and build a selector.
        Returns every node with its id, name, object_type, and file_path.

        Always call stats first to understand graph scale. For large repos,
        pass a glob filter to narrow results (e.g. filter="*auth*", filter="serpentine.mcp.*").
        The repo is ingested automatically if needed.
        """
        if repo_id not in vcs_managers:
            return _recovery_message(UnknownRepoError(repo_id))
        try:
            vcs = vcs_managers[repo_id]
            commit_hash = _ensure_ingested(vcs, store, repo_id, ref)
            graph_json = store.get(repo_id, commit_hash)
            return json.dumps(
                get_catalog(json.loads(graph_json), filter_str=filter), indent=2
            )
        except (NotIngestedError, UnknownRepoError, MissingConfigError) as e:
            return _recovery_message(e)

    @mcp.tool()
    def stats(repo_id: str, ref: str) -> str:
        """Get node and edge counts by type for a repo at a ref.

        Call this first to understand graph scale before fetching the catalog or
        running analyze. Returns node_count, edge_count, nodes_by_type, edges_by_type.
        The repo is ingested automatically if needed.
        """
        if repo_id not in vcs_managers:
            return _recovery_message(UnknownRepoError(repo_id))
        try:
            vcs = vcs_managers[repo_id]
            commit_hash = _ensure_ingested(vcs, store, repo_id, ref)
            graph_json = store.get(repo_id, commit_hash)
            return json.dumps(get_stats(json.loads(graph_json)), indent=2)
        except (NotIngestedError, UnknownRepoError, MissingConfigError) as e:
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

        The repo is ingested automatically if needed — no manual ingest step required.

        WORKFLOW — always follow this order:
        1. Call the `stats` tool to understand graph scale.
        2. Call the `catalog` tool (with a filter) to discover node IDs.
        3. Call this tool with a selector built from catalog IDs.

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
            _ensure_ingested(vcs, store, repo_id, ref)
            graph_data = get_graph(
                store, vcs, repo_id, ref, select=select, exclude=exclude
            )
            if source:
                inject_source_on_demand(
                    graph_data, VcsSourceProvider(vcs._backend, ref)
                )
            else:
                _strip_code(graph_data.get("nodes", []))
            return json.dumps(graph_data, indent=2)
        except (NotIngestedError, UnknownRepoError, MissingConfigError) as e:
            return _recovery_message(e)

    @mcp.tool()
    def ingest_ref_tool(
        repo_id: str,
        ref: str,
        ignore_config: bool = False,
    ) -> str:
        """Force re-analysis of a repo at a ref and store the graph.

        Use this to explicitly refresh a ref that may have changed, or to ingest
        with a specific config setting. For normal querying, catalog/stats/analyze
        auto-ingest as needed — you don't have to call this manually first.
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
