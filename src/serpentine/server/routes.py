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
from collections.abc import Iterator
from typing import TYPE_CHECKING, Any

from starlette.requests import Request
from starlette.responses import JSONResponse, Response
from starlette.routing import Route, WebSocketRoute
from starlette.websockets import WebSocket, WebSocketDisconnect

from serpentine.selector import GraphSelector, filter_by_state
from serpentine.server.websocket import ConnectionManager

if TYPE_CHECKING:
    from serpentine.state import GraphStateManager

logger = logging.getLogger(__name__)


def create_routes(
    state_manager: "GraphStateManager",
    connection_manager: ConnectionManager,
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
        graph_data = state_manager.get_graph_data()

        # Extract query parameters
        select = request.query_params.get("select", "").strip()
        exclude = request.query_params.get("exclude", "").strip()
        state = request.query_params.get("state", "").strip()

        # Apply filtering if select or exclude parameter is provided
        if select or exclude:
            logger.info(f"Applying selector: select={select} exclude={exclude}")
            logger.info(
                f"Before filter: {len(graph_data.get('nodes', []))} nodes, {len(graph_data.get('edges', []))} edges"
            )
            graph_data = GraphSelector.resolve(
                graph_data, select=select, exclude=exclude
            )
            logger.info(
                f"After filter: {len(graph_data.get('nodes', []))} nodes, {len(graph_data.get('edges', []))} edges"
            )

        # Apply state filter
        if state:
            states = {s.strip() for s in state.split(",") if s.strip()}
            graph_data = filter_by_state(graph_data, states)

        # Return as JSON
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
        graph_data = state_manager.get_graph_data()

        include_standard = (
            request.query_params.get("include_standard", "true").lower() != "false"
        )
        include_third_party = (
            request.query_params.get("include_third_party", "true").lower() != "false"
        )
        state = request.query_params.get("state", "").strip()

        # Apply state filter before building catalog
        if state:
            states = {s.strip() for s in state.split(",") if s.strip()}
            graph_data = filter_by_state(graph_data, states)

        def _strip_and_filter(nodes: list[dict[str, Any]]) -> list[dict[str, Any]]:
            result = []
            for node in nodes:
                origin = node.get("origin", "local")
                if origin == "standard" and not include_standard:
                    continue
                if origin == "third-party" and not include_third_party:
                    continue
                catalog_node = {
                    "id": node.get("id"),
                    "name": node.get("name"),
                    "label": node.get("label") or node.get("name"),
                    "type": node.get("type") or node.get("object_type"),
                    "origin": origin,
                    "parent": node.get("parent"),
                    "children": _strip_and_filter(node.get("children", [])),
                }
                result.append(catalog_node)
            return result

        catalog_nodes = _strip_and_filter(graph_data.get("nodes", []))

        return Response(
            content=json.dumps(
                {
                    "nodes": catalog_nodes,
                    "metadata": {
                        "node_count": sum(1 for _ in _count_nodes(catalog_nodes))
                    },
                }
            ),
            media_type="application/json",
        )

    def _count_nodes(nodes: list[dict[str, Any]]) -> Iterator[dict[str, Any]]:
        for node in nodes:
            yield node
            yield from _count_nodes(node.get("children", []))

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
        WebSocketRoute("/ws", websocket_endpoint),
    ]
