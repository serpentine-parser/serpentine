import json

from fastmcp import FastMCP

from serpentine.domain import NotIngestedError, UnknownRepoError, get_catalog, get_stats
from serpentine.storage.base import GraphStore
from serpentine.vcs.manager import VcsManager


def register_resources(
    mcp: FastMCP,
    store: GraphStore,
    vcs_managers: dict[str, VcsManager],
) -> None:
    @mcp.resource("serpentine://{repo_id}/{ref}/catalog")
    def catalog_resource(repo_id: str, ref: str) -> str:
        if repo_id not in vcs_managers:
            raise UnknownRepoError(repo_id)
        vcs = vcs_managers[repo_id]
        commit_hash = vcs._backend.resolve_to_commit_hash(ref)
        graph_json = store.get(repo_id, commit_hash)
        if graph_json is None:
            raise NotIngestedError(repo_id, ref)
        graph_data = json.loads(graph_json)
        nodes = get_catalog(graph_data)
        return json.dumps(nodes, indent=2)

    @mcp.resource("serpentine://{repo_id}/{ref}/stats")
    def stats_resource(repo_id: str, ref: str) -> str:
        if repo_id not in vcs_managers:
            raise UnknownRepoError(repo_id)
        vcs = vcs_managers[repo_id]
        commit_hash = vcs._backend.resolve_to_commit_hash(ref)
        graph_json = store.get(repo_id, commit_hash)
        if graph_json is None:
            raise NotIngestedError(repo_id, ref)
        graph_data = json.loads(graph_json)
        return json.dumps(get_stats(graph_data), indent=2)
