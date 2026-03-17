"""
Serpentine Server Package - Web server for dependency graph visualization.

This package provides a Starlette-based web server with:
- Static file serving for the frontend UI
- WebSocket support for real-time graph updates
- REST API for graph data
Architecture:
- `app.py`: Application factory and route composition
- `routes.py`: HTTP route handlers
- `websocket.py`: WebSocket connection management
"""

from serpentine.server.app import create_app

__all__ = ["create_app"]
