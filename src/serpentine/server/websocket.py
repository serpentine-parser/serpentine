"""
WebSocket connection management for real-time graph updates.

Responsibilities:
- Managing connected WebSocket clients
- Broadcasting graph updates to all clients
- Handling client connection lifecycle

This module is decoupled from the graph state - it only knows how to
broadcast messages to connected clients.
"""

import asyncio
import json
import logging
from typing import Any

from starlette.websockets import WebSocket

logger = logging.getLogger(__name__)


class ConnectionManager:
    """
    Manages WebSocket connections and broadcasts.

    Thread-safe management of connected clients with support for
    broadcasting messages to all connected clients.

    Usage:
        manager = ConnectionManager()

        # In WebSocket endpoint:
        await manager.connect(websocket)
        try:
            while True:
                data = await websocket.receive_text()
                # handle incoming messages
        except WebSocketDisconnect:
            manager.disconnect(websocket)

        # To broadcast to all clients:
        await manager.broadcast({"type": "update", "data": ...})
    """

    def __init__(self) -> None:
        self._connections: set[WebSocket] = set()
        self._lock = asyncio.Lock()

    async def connect(self, websocket: WebSocket) -> None:
        """Accept and register a new WebSocket connection."""
        await websocket.accept()
        async with self._lock:
            self._connections.add(websocket)
        logger.debug(f"Client connected. Total connections: {len(self._connections)}")

    def disconnect(self, websocket: WebSocket) -> None:
        """Remove a WebSocket connection from the manager."""
        self._connections.discard(websocket)
        logger.debug(
            f"Client disconnected. Total connections: {len(self._connections)}"
        )

    async def broadcast(self, message: dict[str, Any]) -> None:
        """
        Broadcast a message to all connected clients.

        Handles disconnected clients gracefully by removing them
        from the connection set.
        """
        if not self._connections:
            return

        payload = json.dumps(message)
        disconnected: set[WebSocket] = set()

        async with self._lock:
            for connection in self._connections:
                try:
                    await connection.send_text(payload)
                except Exception as e:
                    logger.warning(f"Failed to send to client: {e}")
                    disconnected.add(connection)

            # Clean up disconnected clients
            self._connections -= disconnected

    async def send_graph_update(self, graph_json: str) -> None:
        """
        Send a graph update to all connected clients.

        This is a convenience method that wraps the graph data
        in a standard message format.
        """
        await self.broadcast(
            {
                "type": "graph_update",
                "data": json.loads(graph_json),
            }
        )

    @property
    def connection_count(self) -> int:
        """Number of currently connected clients."""
        return len(self._connections)


# Module-level singleton for use across the application
# This allows the watcher to trigger broadcasts without tight coupling
_default_manager: ConnectionManager | None = None


def get_connection_manager() -> ConnectionManager:
    """Get the default connection manager instance."""
    global _default_manager
    if _default_manager is None:
        _default_manager = ConnectionManager()
    return _default_manager


def set_connection_manager(manager: ConnectionManager) -> None:
    """Set a custom connection manager (useful for testing)."""
    global _default_manager
    _default_manager = manager
