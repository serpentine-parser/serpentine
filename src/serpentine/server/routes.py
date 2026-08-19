"""
HTTP route handlers for the Serpentine server.

Responsibilities:
- Serving the graph data via REST API
- Handling WebSocket connections
- Health check endpoint

Routes are defined as functions that can be composed into the
Starlette application by the app factory.
"""

import json
import logging
from typing import TYPE_CHECKING, Any

from starlette.requests import Request
from starlette.responses import JSONResponse, Response
from starlette.routing import Route, WebSocketRoute
from starlette.websockets import WebSocket, WebSocketDisconnect

from serpentine.domain import apply_filters, get_catalog, inject_source
from serpentine.server.websocket import ConnectionManager
from serpentine.vcs.manager import VcsManager

if TYPE_CHECKING:
    from serpentine.state import GraphStateManager

logger = logging.getLogger(__name__)


def create_routes(
    state_manager: "GraphStateManager",
    connection_manager: ConnectionManager,
    vcs_manager: VcsManager | None = None,
) -> list[Route | WebSocketRoute]:
    """
    Create all HTTP and WebSocket routes for the application.

    This function creates route handlers with the provided dependencies
    injected via closure, keeping handlers pure and testable.

    Args:
        state_manager: The graph state manager for data access
        connection_manager: The WebSocket connection manager

    Returns:
        List of Starlette Route objects
    """

    async def health(request: Request) -> JSONResponse:
        """Health check endpoint."""
        return JSONResponse(
            {
                "status": "healthy",
                "connections": connection_manager.connection_count,
                "nodes": state_manager.node_count,
                "edges": state_manager.edge_count,
            }
        )

    async def get_graph(request: Request) -> Response:
        """
        Return the current dependency graph as JSON.

        Query Parameters:
            select (str, optional): dbt-style selector pattern
                - pattern: Exact match or glob pattern (e.g., "mod*.py", "test_*")
                - +pattern: Pattern + all upstream dependencies
                - pattern+: Pattern + all downstream dependents
                - +pattern+: Pattern + both directions
                - @pattern: Pattern + complete connected component
                - Multiple patterns separated by commas for union

            exclude (str, optional): Patterns to exclude from results
                - Glob patterns separated by commas (e.g., "mock_*,test_*")

        Examples:
            GET /api/graph                           # Full graph
            GET /api/graph?select=core*              # Matches patterns
            GET /api/graph?select=+parser            # Parser + dependencies
            GET /api/graph?select=analyzer+          # Analyzer + dependents
            GET /api/graph?select=@test&exclude=mock # Component without mocks
        """
        select = request.query_params.get("select", "").strip()
        exclude = request.query_params.get("exclude", "").strip()
        state = request.query_params.get("state", "").strip()

        graph_data = apply_filters(
            state_manager.get_graph_data(),
            select=select or None,
            exclude=exclude or None,
            state=state or None,
        )

        return Response(
            content=json.dumps(graph_data),
            media_type="application/json",
        )

    async def get_catalog(request: Request) -> Response:
        """
        Return the full node catalog for search autocomplete and object explorer.

        Unlike /api/graph, this endpoint is never filtered by selector patterns —
        only by package origin. This ensures search and navigation always have
        access to the complete node set.

        Query Parameters:
            include_standard (bool, default true): Include stdlib nodes
            include_third_party (bool, default true): Include third-party nodes
        """
        include_standard = (
            request.query_params.get("include_standard", "true").lower() != "false"
        )
        include_third_party = (
            request.query_params.get("include_third_party", "true").lower() != "false"
        )
        state = request.query_params.get("state", "").strip()

        flat_nodes = get_catalog(
            state_manager.get_graph_data(),
            include_standard=include_standard,
            include_third_party=include_third_party,
            state=state or None,
        )

        return Response(
            content=json.dumps(
                {"nodes": flat_nodes, "metadata": {"node_count": len(flat_nodes)}}
            ),
            media_type="application/json",
        )

    async def get_vcs_refs(request: Request) -> Response:
        """Return available VCS refs for the ref picker."""
        if vcs_manager is None:
            return Response(
                content=json.dumps({"available": False, "refs": []}),
                media_type="application/json",
            )
        refs = [{"id": r.id, "display": r.display, "kind": r.kind} for r in vcs_manager.list_refs()]
        return Response(
            content=json.dumps({"available": True, "refs": refs}),
            media_type="application/json",
        )

    async def get_vcs_refs(request: Request) -> Response:
        """Return available VCS refs for the ref picker."""
        if vcs_manager is None:
            return Response(
                content=json.dumps({"available": False, "refs": []}),
                media_type="application/json",
            )
        refs = [{"id": r.id, "display": r.display, "kind": r.kind} for r in vcs_manager.list_refs()]
        return Response(
            content=json.dumps({"available": True, "refs": refs}),
            media_type="application/json",
        )

    async def websocket_endpoint(websocket: WebSocket) -> None:
        """
        WebSocket endpoint for real-time graph updates.

        Protocol:
        - On connect: Client receives current graph state
        - On file change: Client receives updated graph
        - Client can send: {"type": "ping"} for keepalive
        """
        await connection_manager.connect(websocket)

        try:
            # Send initial graph state
            await websocket.send_json(
                {
                    "type": "graph_update",
                    "data": state_manager.get_graph_data(),
                }
            )

            # Listen for client messages (ping/pong, future commands)
            while True:
                data = await websocket.receive_json()

                if data.get("type") == "ping":
                    await websocket.send_json({"type": "pong"})
                elif data.get("type") == "request_graph":
                    # Client explicitly requests current state
                    await websocket.send_json(
                        {
                            "type": "graph_update",
                            "data": state_manager.get_graph_data(),
                        }
                    )
                elif data.get("action") == "dismiss_change":
                    node_id = data.get("data", {}).get("node_id", "")
                    if node_id:
                        state_manager.dismiss_change(node_id)
                        await connection_manager.send_graph_update(
                            state_manager.get_graph_json()
                        )
                elif data.get("action") == "dismiss_all_changes":
                    state_manager.dismiss_all_changes()
                    await connection_manager.send_graph_update(
                        state_manager.get_graph_json()
                    )
                elif data.get("action") == "get_node_code":
                    qualname = data.get("data", {}).get("qualname", "")
                    graph_data = state_manager.get_graph_data()

                    def _find_node(nodes: list[dict[str, Any]], qn: str) -> dict[str, Any] | None:
                        for n in nodes:
                            if n.get("id") == qn:
                                return n
                            found = _find_node(n.get("children", []), qn)
                            if found:
                                return found
                        return None

                    node = _find_node(graph_data.get("nodes", []), qualname)
                    code: str | None = None
                    if node:
                        inject_source({"nodes": [node], "edges": []}, state_manager._analyzer)
                        code = node.get("code_block")
                    await websocket.send_json({
                        "type": "node_code",
                        "data": {"qualname": qualname, "code": code},
                    })
                elif data.get("action") == "set_vcs_comparison":
                    action_data = data.get("data", {})
                    from_ref: str = action_data.get("from", "@start")
                    to_ref: str = action_data.get("to", "@current")

                    try:
                        # Resolve from_graph_json
                        if from_ref == "@current":
                            from_graph_json = json.dumps(state_manager._graph_data)
                        elif from_ref == "@start":
                            from_graph_json = json.dumps(state_manager._start_graph_data)
                        elif vcs_manager is None:
                            await websocket.send_json({"type": "error", "data": {"message": "VCS not available"}})
                            continue
                        else:
                            valid_ids = {r.id for r in vcs_manager.list_refs()}
                            if from_ref not in valid_ids:
                                await websocket.send_json({"type": "error", "data": {"message": f"Unknown ref: {from_ref}"}})
                                continue
                            from_graph_json = vcs_manager.get_graph_at(from_ref)

                        # Resolve to_graph_json
                        to_graph_json: str | None
                        if to_ref == "@current":
                            to_graph_json = None
                        elif to_ref == "@start":
                            to_graph_json = json.dumps(state_manager._start_graph_data)
                        elif vcs_manager is None:
                            await websocket.send_json({"type": "error", "data": {"message": "VCS not available"}})
                            continue
                        else:
                            valid_ids = {r.id for r in vcs_manager.list_refs()}
                            if to_ref not in valid_ids:
                                await websocket.send_json({"type": "error", "data": {"message": f"Unknown ref: {to_ref}"}})
                                continue
                            to_graph_json = vcs_manager.get_graph_at(to_ref)

                        state_manager.set_vcs_comparison(from_graph_json, to_graph_json)
                        await connection_manager.send_graph_update(state_manager.get_graph_json())
                    except Exception as e:
                        logger.error(f"[vcs] set_vcs_comparison failed: {e}", exc_info=True)
                        await websocket.send_json({"type": "error", "data": {"message": f"Comparison failed: {e}"}})
                elif data.get("action") == "clear_vcs_comparison":
                    state_manager.clear_vcs_comparison()
                    await connection_manager.send_graph_update(state_manager.get_graph_json())
                elif data.get("action") == "update_start":
                    state_manager.update_start()
                    await connection_manager.send_graph_update(state_manager.get_graph_json())
                else:
                    logger.debug(f"Unknown message type: {data.get('type')}")

        except WebSocketDisconnect:
            connection_manager.disconnect(websocket)
        except Exception as e:
            logger.error(f"WebSocket error: {e}")
            connection_manager.disconnect(websocket)

    return [
        Route("/api/health", health, methods=["GET"]),
        Route("/api/graph", get_graph, methods=["GET"]),
        Route("/api/catalog", get_catalog, methods=["GET"]),
        Route("/api/vcs/refs", get_vcs_refs, methods=["GET"]),
        WebSocketRoute("/ws", websocket_endpoint),
    ]
