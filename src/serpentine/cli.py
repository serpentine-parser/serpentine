"""
Serpentine CLI - Command-line interface for dependency graph analysis.

This module provides the main entry point for the serpentine tool.
Commands are organized by concern:
- `serve`: Start the web server with UI and optional file watching
- `analyze`: One-shot analysis of a project (outputs JSON)
"""

import fnmatch
import json
import threading
import time
import webbrowser
from pathlib import Path
from typing import Any

import click
import uvicorn

from serpentine.selector import GraphSelector, filter_by_state
from serpentine.server import create_app
from serpentine.state import GraphStateManager
from serpentine.watcher import FileWatcher


@click.group()
@click.version_option(version="0.1.0", prog_name="serpentine")
def main() -> None:
    """Serpentine: Fast dependency graph analysis for Python projects."""
    pass


@main.command()
@click.argument("path", type=click.Path(exists=True), default=".")
@click.option(
    "--port",
    "-p",
    type=int,
    default=8765,
    help="Port to run the server on (default: 8765)",
)
@click.option(
    "--host",
    "-h",
    type=str,
    default="127.0.0.1",
    help="Host to bind to (default: 127.0.0.1)",
)
@click.option(
    "--no-browser",
    is_flag=True,
    default=False,
    help="Don't automatically open browser",
)
@click.option(
    "--no-watch",
    is_flag=True,
    default=False,
    help="Disable file watching (static analysis only)",
)
def serve(
    path: str,
    port: int,
    host: str,
    no_browser: bool,
    no_watch: bool,
) -> None:
    """Start the serpentine web server with live dependency graph visualization.

    PATH is the directory to analyze (defaults to current directory).

    Examples:
        serpentine serve                    # Serve current directory
        serpentine serve ./my-project       # Serve specific project
        serpentine serve -p 9000            # Custom port
        serpentine serve --no-watch         # No live updates
    """
    project_path = Path(path).resolve()
    click.echo(f"📂 Analyzing: {project_path}")

    # Create the shared state manager (loads config from project)
    state_manager = GraphStateManager(project_path)

    # Initialize with first analysis
    state_manager.analyze_project(project_path)
    click.echo(
        f"✅ Found {state_manager.node_count} nodes, {state_manager.edge_count} edges"
    )

    # Create the web application
    app = create_app(state_manager, static_dir=_get_static_dir())

    # Set up file watcher if enabled
    watcher: FileWatcher | None = None
    if not no_watch:
        watcher = FileWatcher(
            path=project_path,
            on_change=lambda changed_files: state_manager.analyze_project(
                project_path, changed_files
            ),
            extensions=set(state_manager.config.extensions),
        )
        click.echo("👀 Watching for file changes...")

    url = f"http://{host}:{port}"
    click.echo(f"🚀 Server starting at {url}")

    if not no_browser:
        # Open browser after small delay to let server start
        def open_browser() -> None:
            time.sleep(0.5)
            webbrowser.open(url)

        threading.Thread(target=open_browser, daemon=True).start()

    # Run the server
    try:
        if watcher:
            watcher.start()
        uvicorn.run(
            app, host=host, port=port, log_level="warning", ws_per_message_deflate=False
        )
    finally:
        if watcher:
            watcher.stop()


@main.command()
@click.argument("path", type=click.Path(exists=True), default=".")
@click.option(
    "--output",
    "-o",
    type=click.Path(),
    help="Output file path (default: stdout)",
)
@click.option(
    "--pretty",
    is_flag=True,
    default=False,
    help="Pretty-print JSON output",
)
@click.option(
    "--select",
    type=str,
    default=None,
    help="dbt-style selector to filter nodes (e.g. '+auth*', 'mod+', '@core')",
)
@click.option(
    "--exclude",
    type=str,
    default=None,
    help="Exclusion pattern (same selector syntax, e.g. 'test_*')",
)
@click.option(
    "--include-standard",
    is_flag=True,
    default=False,
    help="Include stdlib nodes in output (default: off)",
)
@click.option(
    "--include-third-party",
    is_flag=True,
    default=False,
    help="Include third-party nodes in output (default: off)",
)
@click.option(
    "--no-cfg",
    is_flag=True,
    default=False,
    help="Strip cfg field from all nodes to reduce output noise",
)
@click.option(
    "--edges-only",
    is_flag=True,
    default=False,
    help="Output only the edges array (compact, useful for cross-boundary analysis)",
)
@click.option(
    "--state",
    type=str,
    default=None,
    help="Comma-separated change states to include: modified,added,deleted",
)
def analyze(
    path: str,
    output: str | None,
    pretty: bool,
    select: str | None,
    exclude: str | None,
    include_standard: bool,
    include_third_party: bool,
    no_cfg: bool,
    edges_only: bool,
    state: str | None,
) -> None:
    """Analyze a project and output the dependency graph as JSON.

    PATH is the directory to analyze (defaults to current directory).

    Examples:
        serpentine analyze                  # Output to stdout
        serpentine analyze -o graph.json    # Output to file
        serpentine analyze --pretty         # Pretty-printed
        serpentine analyze --select "auth*" --exclude "test_*" --no-cfg --pretty
    """
    project_path = Path(path).resolve()
    click.echo(f"📂 Analyzing: {project_path}", err=True)

    state_manager = GraphStateManager(project_path)
    state_manager.analyze_project(project_path)

    click.echo(
        f"✅ Found {state_manager.node_count} nodes, {state_manager.edge_count} edges",
        err=True,
    )

    # Get the graph data as a dict for post-processing
    graph_data = json.loads(state_manager.get_graph_json())

    # Filter by origin (strip standard/third-party nodes by default)
    if not include_standard or not include_third_party:
        graph_data = _filter_by_origin(
            graph_data, include_standard, include_third_party
        )

    # Apply selector/exclude filtering
    if select or exclude:
        graph_data = GraphSelector.resolve(
            graph_data,
            select=select or "",
            exclude=exclude or "",
        )

    # Apply state filter
    if state:
        states = {s.strip() for s in state.split(",") if s.strip()}
        graph_data = filter_by_state(graph_data, states)

    # Strip cfg fields if requested
    if no_cfg:
        _strip_cfg(graph_data.get("nodes", []))

    output_data = graph_data.get("edges", []) if edges_only else graph_data
    graph_json = (
        json.dumps(output_data, indent=2) if pretty else json.dumps(output_data)
    )

    if output:
        Path(output).write_text(graph_json)
        click.echo(f"📄 Written to: {output}", err=True)
    else:
        click.echo(graph_json)


@main.command()
@click.argument("path", type=click.Path(exists=True), default=".")
@click.option(
    "--filter",
    "filters",
    type=str,
    multiple=True,
    help="Glob pattern matched against node id and name (multiple = union)",
)
@click.option(
    "--include-standard",
    is_flag=True,
    default=False,
    help="Include stdlib nodes (default: off)",
)
@click.option(
    "--include-third-party",
    is_flag=True,
    default=False,
    help="Include third-party nodes (default: off)",
)
@click.option(
    "--output",
    "-o",
    type=click.Path(),
    help="Output file path (default: stdout)",
)
@click.option(
    "--no-assignments",
    is_flag=True,
    default=False,
    help="Exclude assignment nodes (variables) — keeps modules, classes, functions only",
)
@click.option(
    "--pretty",
    is_flag=True,
    default=False,
    help="Pretty-print JSON output",
)
@click.option(
    "--state",
    type=str,
    default=None,
    help="Filter flat node list by change_status (modified,added,deleted)",
)
def catalog(
    path: str,
    filters: tuple[str, ...],
    include_standard: bool,
    include_third_party: bool,
    no_assignments: bool,
    output: str | None,
    pretty: bool,
    state: str | None,
) -> None:
    """Flat list of all nodes for agent discovery.

    Use this to find relevant node IDs before constructing selectors for analyze.

    PATH is the directory to analyze (defaults to current directory).

    Examples:
        serpentine catalog .
        serpentine catalog . --filter "auth*" --filter "login*"
        serpentine catalog . --no-assignments --filter "auth*"
        serpentine catalog . --include-third-party --pretty
    """
    project_path = Path(path).resolve()
    click.echo(f"📂 Analyzing: {project_path}", err=True)

    state_manager = GraphStateManager(project_path)
    state_manager.analyze_project(project_path)

    click.echo(
        f"✅ Found {state_manager.node_count} nodes, {state_manager.edge_count} edges",
        err=True,
    )

    graph_data = json.loads(state_manager.get_graph_json())

    # Flatten tree into catalog entries
    flat_nodes: list[dict[str, Any]] = []
    _flatten_nodes(graph_data.get("nodes", []), flat_nodes)

    # Filter by origin
    if not include_standard or not include_third_party:
        flat_nodes = [
            n
            for n in flat_nodes
            if not (n.get("origin") == "standard" and not include_standard)
            and not (n.get("origin") == "third-party" and not include_third_party)
        ]

    # Strip assignment nodes if requested
    if no_assignments:
        flat_nodes = [n for n in flat_nodes if n.get("type") != "assignment"]

    # Apply glob filters (union across all patterns, matched against id and name)
    if filters:
        flat_nodes = [
            n
            for n in flat_nodes
            if any(
                fnmatch.fnmatch(n.get("id", ""), pat)
                or fnmatch.fnmatch(n.get("name", ""), pat)
                for pat in filters
            )
        ]

    # Apply state filter
    if state:
        states = {s.strip() for s in state.split(",") if s.strip()}
        flat_nodes = [n for n in flat_nodes if n.get("change_status") in states]

    result = {
        "nodes": flat_nodes,
        "metadata": {"node_count": len(flat_nodes)},
    }

    catalog_json = json.dumps(result, indent=2) if pretty else json.dumps(result)

    if output:
        Path(output).write_text(catalog_json)
        click.echo(f"📄 Written to: {output}", err=True)
    else:
        click.echo(catalog_json)


@main.command()
@click.argument("path", type=click.Path(exists=True), default=".")
@click.option(
    "--include-standard",
    is_flag=True,
    default=False,
    help="Include stdlib nodes in counts (default: off)",
)
@click.option(
    "--include-third-party",
    is_flag=True,
    default=False,
    help="Include third-party nodes in counts (default: off)",
)
@click.option(
    "--pretty",
    is_flag=True,
    default=False,
    help="Pretty-print JSON output",
)
def stats(
    path: str,
    include_standard: bool,
    include_third_party: bool,
    pretty: bool,
) -> None:
    """Quick summary of project scale without full graph output.

    Useful as a first call to understand project scale before deeper analysis.

    PATH is the directory to analyze (defaults to current directory).

    Examples:
        serpentine stats .
        serpentine stats . --include-standard --include-third-party
        serpentine stats . --pretty
    """
    project_path = Path(path).resolve()
    click.echo(f"📂 Analyzing: {project_path}", err=True)

    state_manager = GraphStateManager(project_path)
    state_manager.analyze_project(project_path)

    graph_data = json.loads(state_manager.get_graph_json())

    # Flatten all nodes for counting
    all_nodes: list[dict[str, Any]] = []
    _flatten_nodes(graph_data.get("nodes", []), all_nodes)

    # Filter by origin
    if not include_standard or not include_third_party:
        all_nodes = [
            n
            for n in all_nodes
            if not (n.get("origin") == "standard" and not include_standard)
            and not (n.get("origin") == "third-party" and not include_third_party)
        ]

    # Count edges (filter to surviving node ids)
    surviving_ids = {n["id"] for n in all_nodes}
    edges = graph_data.get("edges", [])
    filtered_edges = [
        e
        for e in edges
        if (e.get("source") or e.get("caller")) in surviving_ids
        and (e.get("target") or e.get("callee")) in surviving_ids
    ]

    # by_type counts
    by_type: dict[str, int] = {}
    for node in all_nodes:
        t = node.get("type") or node.get("object_type") or "unknown"
        by_type[t] = by_type.get(t, 0) + 1

    # by_origin counts
    by_origin: dict[str, int] = {}
    for node in all_nodes:
        o = node.get("origin") or "local"
        by_origin[o] = by_origin.get(o, 0) + 1

    # top_level_modules: parent == null, type == "module", origin == "local"
    top_level_modules = [
        n["id"]
        for n in all_nodes
        if n.get("parent") is None
        and (n.get("type") or n.get("object_type")) == "module"
        and (n.get("origin") or "local") == "local"
    ]

    result = {
        "node_count": len(all_nodes),
        "edge_count": len(filtered_edges),
        "by_type": by_type,
        "by_origin": by_origin,
        "top_level_modules": top_level_modules,
    }

    click.echo(json.dumps(result, indent=2) if pretty else json.dumps(result))


def _flatten_nodes(
    nodes: list[dict[str, Any]],
    result: list[dict[str, Any]],
    parent_id: str | None = None,
) -> None:
    """Recursively flatten nested node tree into a flat catalog list."""
    keep_keys = {"id", "name", "type", "object_type", "origin", "parent", "file_path", "change_status"}
    for node in nodes:
        entry = {k: v for k, v in node.items() if k in keep_keys}
        # Normalize type field
        if "object_type" in entry and "type" not in entry:
            entry["type"] = entry.pop("object_type")
        result.append(entry)
        _flatten_nodes(node.get("children", []), result, node.get("id"))


def _filter_by_origin(
    graph: dict[str, Any], include_standard: bool, include_third_party: bool
) -> dict[str, Any]:
    """Filter graph nodes by origin, removing standard/third-party as configured."""

    def _filter_nodes(nodes: list[dict[str, Any]]) -> list[dict[str, Any]]:
        result = []
        for node in nodes:
            origin = node.get("origin", "local")
            if origin == "standard" and not include_standard:
                continue
            if origin == "third-party" and not include_third_party:
                continue
            filtered = dict(node)
            filtered["children"] = _filter_nodes(node.get("children", []))
            result.append(filtered)
        return result

    edges = graph.get("edges", [])
    filtered_nodes = _filter_nodes(graph.get("nodes", []))

    # Collect surviving node ids to filter edges
    surviving_ids: set[str] = set()

    def _collect_ids(nodes: list[dict[str, Any]]) -> None:
        for node in nodes:
            surviving_ids.add(node["id"])
            _collect_ids(node.get("children", []))

    _collect_ids(filtered_nodes)

    filtered_edges = [
        e
        for e in edges
        if (e.get("source") or e.get("caller")) in surviving_ids
        and (e.get("target") or e.get("callee")) in surviving_ids
    ]

    result: dict[str, Any] = {"nodes": filtered_nodes, "edges": filtered_edges}
    if "metadata" in graph:
        result["metadata"] = graph["metadata"]
    return result


def _strip_cfg(nodes: list[dict[str, Any]]) -> None:
    """Recursively strip the cfg field from all nodes in-place."""
    for node in nodes:
        node.pop("cfg", None)
        _strip_cfg(node.get("children", []))


def _get_static_dir() -> Path:
    """Get the path to the bundled static files directory."""
    # In development, look for frontend/dist relative to the package
    # In production (pip install), it's bundled with the package
    package_dir = Path(__file__).parent

    # Check for development structure
    dev_static = package_dir.parent.parent / "frontend" / "dist"
    if dev_static.exists():
        return dev_static

    # Check for bundled static files
    bundled_static = package_dir / "static"
    if bundled_static.exists():
        return bundled_static

    # Fallback to a placeholder directory (will serve 404s until UI is built)
    return package_dir / "static"


if __name__ == "__main__":
    main()
