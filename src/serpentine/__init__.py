"""
Serpentine - Fast dependency graph analysis for Python projects.

This package provides tools for analyzing Python source code and
visualizing dependency graphs with real-time updates.

Main components:
- `cli`: Command-line interface (serpentine serve, serpentine analyze)
- `server`: Web server for UI and WebSocket updates
- `state`: Graph state management
- `watcher`: File system monitoring

Quick start:
    $ serpentine serve ./my-project

Or programmatically:
    from serpentine.state import GraphStateManager
    from serpentine.server import create_app

    state = GraphStateManager()
    state.analyze_project(Path("./my-project"))
    app = create_app(state)
"""

__version__ = "0.1.3"

from serpentine.server import create_app
from serpentine.state import GraphStateManager
from serpentine.watcher import FileWatcher

__all__ = [
    "GraphStateManager",
    "FileWatcher",
    "create_app",
    "__version__",
]
