from fastmcp import FastMCP

from serpentine.mcp.prompts import register_prompts
from serpentine.mcp.resources import register_resources
from serpentine.mcp.tools import register_tools
from serpentine.storage.base import GraphStore
from serpentine.vcs.manager import VcsManager


def create_mcp_app(
    store: GraphStore,
    vcs_managers: dict[str, VcsManager],
    auth: object | None = None,
) -> FastMCP:
    mcp = FastMCP(name="serpentine")

    register_tools(mcp, store, vcs_managers)
    register_resources(mcp, store, vcs_managers)
    register_prompts(mcp)

    return mcp
