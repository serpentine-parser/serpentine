"""
Starlette application factory for the Serpentine server.

Responsibilities:
- Creating and configuring the Starlette application
- Composing routes, middleware, and static file serving
- Managing application lifecycle (startup/shutdown)

The factory pattern allows for clean dependency injection and
makes testing easier by enabling different configurations.
"""

import asyncio
import logging
from collections.abc import AsyncIterator
from contextlib import asynccontextmanager
from pathlib import Path
from typing import TYPE_CHECKING

from starlette.applications import Starlette
from starlette.middleware import Middleware
from starlette.middleware.cors import CORSMiddleware
from starlette.middleware.gzip import GZipMiddleware
from starlette.requests import Request
from starlette.responses import HTMLResponse
from starlette.routing import Mount, Route
from starlette.staticfiles import StaticFiles

from serpentine.server.routes import create_routes
from serpentine.server.websocket import ConnectionManager, set_connection_manager

if TYPE_CHECKING:
    from serpentine.state import GraphStateManager

logger = logging.getLogger(__name__)

# Store the event loop reference so file watcher threads can schedule work
_event_loop: asyncio.AbstractEventLoop | None = None


def create_app(
    state_manager: "GraphStateManager",
    static_dir: Path | None = None,
    debug: bool = False,
) -> Starlette:
    """
    Create and configure the Starlette application.

    This factory creates the web application with all routes,
    middleware, and static file serving configured.

    Args:
        state_manager: The graph state manager (shared with watcher)
        static_dir: Path to static files directory for frontend
        debug: Enable debug mode

    Returns:
        Configured Starlette application

    Example:
        state_manager = GraphStateManager()
        state_manager.analyze_project(Path("./my-project"))

        app = create_app(state_manager, static_dir=Path("./frontend/dist"))
        uvicorn.run(app, host="0.0.0.0", port=8765)
    """
    # Create the connection manager and make it globally accessible
    # This allows the file watcher to broadcast updates
    connection_manager = ConnectionManager()
    set_connection_manager(connection_manager)

    # Register the connection manager with the state manager
    # so it can broadcast on updates
    state_manager.set_broadcast_callback(
        lambda: _schedule_broadcast(connection_manager, state_manager)
    )

    @asynccontextmanager
    async def lifespan(app: Starlette) -> AsyncIterator[None]:
        """Application lifespan manager for startup/shutdown."""
        global _event_loop

        logger.info("Serpentine server starting up")

        # Store the event loop for use by file watcher threads
        _event_loop = asyncio.get_running_loop()

        yield

        logger.info("Serpentine server shutting down")
        _event_loop = None

    # Create API routes
    routes = create_routes(state_manager, connection_manager)

    # Add static file serving if directory has a built frontend
    if static_dir and (static_dir / "index.html").exists():
        routes.append(
            Mount(  # type: ignore
                "/",
                app=StaticFiles(directory=str(static_dir), html=True),
                name="static",
            )
        )
        logger.info(f"Serving static files from: {static_dir}")
    else:
        # Create a fallback route for development without frontend
        async def fallback_index(request: Request) -> HTMLResponse:
            return HTMLResponse(_get_fallback_html(state_manager))

        routes.append(Route("/", fallback_index, methods=["GET"]))
        logger.warning("No static directory found, serving fallback HTML")

    # Configure CORS for development (allowing local connections)
    middleware = [
        Middleware(GZipMiddleware, minimum_size=1000),
        Middleware(
            CORSMiddleware,
            allow_origins=["*"],
            allow_methods=["*"],
            allow_headers=["*"],
        ),
    ]

    return Starlette(
        debug=debug,
        routes=routes,
        middleware=middleware,
        lifespan=lifespan,
    )


def _schedule_broadcast(
    connection_manager: ConnectionManager,
    state_manager: "GraphStateManager",
) -> None:
    """
    Schedule a broadcast of the current graph state.

    This runs the async broadcast in a fire-and-forget manner,
    suitable for being called from synchronous file watcher callbacks
    running in background threads.
    """
    global _event_loop

    async def do_broadcast() -> None:
        try:
            await connection_manager.send_graph_update(state_manager.get_graph_json())
            logger.debug(
                f"Broadcast graph update to {connection_manager.connection_count} clients"
            )
        except Exception as e:
            logger.error(f"Failed to broadcast update: {e}")

    # Use the stored event loop to schedule the broadcast
    if _event_loop is not None and _event_loop.is_running():
        asyncio.run_coroutine_threadsafe(do_broadcast(), _event_loop)
    else:
        logger.warning("No event loop available for broadcast")


def _get_fallback_html(state_manager: "GraphStateManager") -> str:
    """Generate fallback HTML when no frontend is built."""
    return f"""
    <!DOCTYPE html>
    <html>
    <head>
        <title>Serpentine</title>
        <style>
            body {{
                font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
                max-width: 800px;
                margin: 50px auto;
                padding: 20px;
                background: #1a1a2e;
                color: #eee;
            }}
            h1 {{ color: #10b981; }}
            .stats {{
                background: #16213e;
                padding: 20px;
                border-radius: 8px;
                margin: 20px 0;
            }}
            .stat {{
                display: inline-block;
                margin-right: 30px;
            }}
            .stat-value {{
                font-size: 2em;
                color: #10b981;
            }}
            pre {{
                background: #0f0f23;
                padding: 15px;
                border-radius: 8px;
                overflow-x: auto;
                max-height: 400px;
            }}
            #ws-status {{
                padding: 5px 10px;
                border-radius: 4px;
                display: inline-block;
            }}
            .connected {{ background: #10b981; color: #fff; }}
            .disconnected {{ background: #ef4444; color: #fff; }}
        </style>
    </head>
    <body>
        <h1>🐍 Serpentine</h1>
        <p>WebSocket Status: <span id="ws-status" class="disconnected">Connecting...</span></p>

        <div class="stats">
            <div class="stat">
                <div class="stat-value" id="node-count">{state_manager.node_count}</div>
                <div>Nodes</div>
            </div>
            <div class="stat">
                <div class="stat-value" id="edge-count">{state_manager.edge_count}</div>
                <div>Edges</div>
            </div>
        </div>

        <h2>Graph Data</h2>
        <pre id="graph-data">Loading...</pre>

        <script>
            const ws = new WebSocket(`ws://${{location.host}}/ws`);
            const status = document.getElementById('ws-status');
            const graphData = document.getElementById('graph-data');
            const nodeCount = document.getElementById('node-count');
            const edgeCount = document.getElementById('edge-count');

            ws.onopen = () => {{
                status.textContent = 'Connected';
                status.className = 'connected';
            }};

            ws.onclose = () => {{
                status.textContent = 'Disconnected';
                status.className = 'disconnected';
            }};

            ws.onmessage = (event) => {{
                const msg = JSON.parse(event.data);
                if (msg.type === 'graph_update') {{
                    graphData.textContent = JSON.stringify(msg.data, null, 2);
                    if (msg.data.metadata) {{
                        nodeCount.textContent = msg.data.metadata.node_count || '?';
                        edgeCount.textContent = msg.data.metadata.edge_count || '?';
                    }}
                }}
            }};

            // Ping every 30s to keep connection alive
            setInterval(() => {{
                if (ws.readyState === WebSocket.OPEN) {{
                    ws.send(JSON.stringify({{type: 'ping'}}));
                }}
            }}, 30000);
        </script>
    </body>
    </html>
    """
