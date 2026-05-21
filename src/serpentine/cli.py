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
from importlib.resources import files as _res_files
from pathlib import Path
from typing import Any

import click
import uvicorn

from serpentine import __version__
from serpentine.selector import GraphSelector, filter_by_state
from serpentine.server import create_app
from serpentine.state import GraphStateManager
from serpentine.watcher import FileWatcher

_DEFAULT_CONFIG_YAML = """\
analysis:
  extensions:
    - .py
    - .js
    - .jsx
    - .ts
    - .tsx
    - .rs
  exclude_dirs:
    - __pycache__
    - .git
    - .venv
    - venv
    - node_modules
    - .mypy_cache
    - .pytest_cache
    - .tox
    - dist
    - build
    - .next
    - .nuxt
    - coverage
  exclude_patterns: []
"""

_CLAUDE_MD_SECTION = """\
## Serpentine

**Never use `grep`, `find`, `rg`, or the Read tool for code navigation.** Serpentine is the replacement.

| Instead of | Use |
|---|---|
| `grep -r "ClassName" .` | `/code-analysis ClassName` |
| `find . -name "*.py" \\| xargs grep X` | `/code-analysis X` |
| `ls src/module/` | `/code-analysis` |
| `cat file.py` or Read tool | `/code-analysis SymbolName` |

- `/code-analysis <target>` — find where a symbol is defined, read its source, trace callers/callees, and check blast radius. Also handles structural questions ("what's in module X?").
"""


@click.group()
@click.version_option(version=__version__, prog_name="serpentine")
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
@click.option(
    "--format",
    "fmt",
    type=click.Choice(["json", "text"]),
    default="text",
    help="Output format: text (default) or json",
)
@click.option(
    "--source",
    is_flag=True,
    default=False,
    help="Include source code blocks in text output (only with --format text)",
)
@click.option(
    "--include-assignments",
    is_flag=True,
    default=False,
    help="Include assignment nodes in text output (default: excluded, only with --format text)",
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
    fmt: str,
    source: bool,
    include_assignments: bool,
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
    state_manager = GraphStateManager(project_path)
    state_manager.analyze_project(project_path)

    # Get the graph data as a dict for post-processing
    graph_data = state_manager.get_graph_data()

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

    if fmt == "text":
        lines: list[str] = []
        if source:
            node_index = _build_node_index(graph_data.get("nodes", []))
            project_path_obj = Path(path).resolve()
            edges_by_caller: dict[str, list[dict[str, Any]]] = {}
            for e in graph_data.get("edges", []):
                edges_by_caller.setdefault(e.get("caller", ""), []).append(e)
            for node_id, node in node_index.items():
                object_type = node.get("object_type", "")
                if not include_assignments and object_type in {"assignment", "unknown"}:
                    continue
                pos = node.get("position", [0, 0])
                rel = _rel_path(node.get("file_path", ""), project_path_obj)
                lines.append(f"## {node_id}  [{object_type}]  {rel}:{pos[0]}-{pos[1]}")
                if object_type in {"function", "class"}:
                    code = node.get("code_block", "")
                    if code:
                        lines.append(code)
                for e in edges_by_caller.get(node_id, []):
                    lines.append(
                        f"  --{e.get('type', 'calls')}--> {e.get('callee', '')}"
                    )
                lines.append("")
        else:
            lines = [
                f"{e.get('caller', '')} --{e.get('type', 'calls')}--> {e.get('callee', '')}"
                for e in graph_data.get("edges", [])
            ]
        text_out = "\n".join(lines)
        if output:
            Path(output).write_text(text_out)
            click.echo(f"📄 Written to: {output}", err=True)
        else:
            click.echo(text_out)
        return

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
    "--include-assignments",
    is_flag=True,
    default=False,
    help="Include assignment nodes (variables) in output (default: excluded)",
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
@click.option(
    "--format",
    "fmt",
    type=click.Choice(["json", "text"]),
    default="text",
    help="Output format: text (default) or json",
)
def catalog(
    path: str,
    filters: tuple[str, ...],
    include_standard: bool,
    include_third_party: bool,
    include_assignments: bool,
    output: str | None,
    pretty: bool,
    state: str | None,
    fmt: str,
) -> None:
    """Flat list of all nodes for agent discovery.

    Use this to find relevant node IDs before constructing selectors for analyze.

    PATH is the directory to analyze (defaults to current directory).

    Examples:
        serpentine catalog .
        serpentine catalog . --filter "auth*" --filter "login*"
        serpentine catalog . --include-assignments --filter "auth*"
        serpentine catalog . --include-third-party --pretty
    """
    project_path = Path(path).resolve()
    state_manager = GraphStateManager(project_path)
    state_manager.analyze_project(project_path)

    graph_data = state_manager.get_graph_data()

    if fmt == "text":
        if not include_standard or not include_third_party:
            graph_data = _filter_by_origin(
                graph_data, include_standard, include_third_party
            )
        project_path_obj = Path(path).resolve()
        state_filter = (
            {s.strip() for s in state.split(",") if s.strip()} if state else None
        )
        lines = _render_catalog_text(
            graph_data.get("nodes", []),
            project_path_obj,
            filters,
            include_assignments,
            state_filter,
        )
        text_out = "\n".join(lines)
        if output:
            Path(output).write_text(text_out)
            click.echo(f"📄 Written to: {output}", err=True)
        else:
            click.echo(text_out)
        return

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

    # Strip assignment nodes unless included
    if not include_assignments:
        flat_nodes = [
            n for n in flat_nodes if n.get("type") in ("module", "class", "function")
        ]

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

    graph_data = state_manager.get_graph_data()

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


@main.command()
@click.argument("path", type=click.Path(exists=True), default=".")
def init(path: str) -> None:
    """Initialize a project for use with serpentine.

    Generates .serpentine.yml, updates .gitignore, installs Claude Code skills,
    and appends a serpentine section to CLAUDE.md.

    PATH is the directory to initialize (defaults to current directory).

    Examples:
        serpentine init             # Initialize current directory
        serpentine init ./my-project
    """
    project_path = Path(path).resolve()

    if not project_path.is_dir():
        click.echo(f"Error: {path} must be a directory, not a file.", err=True)
        raise SystemExit(1)

    results: list[tuple[str, str]] = []

    # Step 1: .serpentine.yml
    config_path = project_path / ".serpentine.yml"
    if config_path.is_dir():
        results.append(
            ("✗", ".serpentine.yml — path is a directory, cannot write config")
        )
    elif config_path.exists():
        results.append(("⚠", ".serpentine.yml — already exists, skipped"))
    else:
        try:
            config_path.write_text(_DEFAULT_CONFIG_YAML)
            results.append(("✓", ".serpentine.yml created"))
        except (PermissionError, OSError) as e:
            results.append(("✗", f".serpentine.yml — {e}"))

    # Step 2: .gitignore
    gitignore_path = project_path / ".gitignore"
    try:
        if gitignore_path.exists():
            content = gitignore_path.read_text()
            if any(
                line.strip() in {".serpentine", ".serpentine/"}
                for line in content.splitlines()
            ):
                results.append(("✓", ".gitignore — .serpentine already present"))
            else:
                sep = "" if content.endswith("\n") else "\n"
                gitignore_path.write_text(f"{content}{sep}.serpentine\n")
                results.append(("✓", ".gitignore updated"))
        else:
            gitignore_path.write_text(".serpentine\n")
            results.append(("✓", ".gitignore created"))
    except (PermissionError, OSError) as e:
        results.append(("✗", f".gitignore — {e}"))

    # Step 3: Skill files
    claude_dir = project_path / ".claude"
    if claude_dir.exists() and not claude_dir.is_dir():
        results.append(("✗", ".claude exists as a file — cannot install skills"))
    else:
        for skill_name in ("code-analysis",):
            skill_target = claude_dir / "skills" / skill_name / "SKILL.md"
            rel = str(skill_target.relative_to(project_path))
            if skill_target.exists():
                results.append(("⚠", f"{rel} — already exists, skipped"))
                continue
            try:
                bundled = _res_files("serpentine.skills").joinpath(f"{skill_name}.md")
                skill_content = bundled.read_text(encoding="utf-8")
            except FileNotFoundError:
                results.append(
                    (
                        "✗",
                        f"{rel} — skill not found in package, try reinstalling serpentine",
                    )
                )
                continue
            try:
                skill_target.parent.mkdir(parents=True, exist_ok=True)
                skill_target.write_text(skill_content)
                results.append(("✓", f"{rel} installed"))
            except (PermissionError, OSError) as e:
                results.append(("✗", f"{rel} — {e}"))

    # Step 4: CLAUDE.md
    claude_md_path = project_path / "CLAUDE.md"
    try:
        if claude_md_path.exists():
            content = claude_md_path.read_text()
            if any(line.strip() == "## Serpentine" for line in content.splitlines()):
                results.append(("✓", "CLAUDE.md — serpentine section already present"))
            else:
                sep = "" if content.endswith("\n") else "\n"
                claude_md_path.write_text(f"{content}{sep}\n{_CLAUDE_MD_SECTION}")
                results.append(("✓", "CLAUDE.md updated"))
        else:
            claude_md_path.write_text(_CLAUDE_MD_SECTION)
            results.append(("✓", "CLAUDE.md created"))
    except (PermissionError, OSError) as e:
        results.append(("✗", f"CLAUDE.md — {e}"))

    click.echo("\nserpentine init complete:")
    for symbol, message in results:
        click.echo(f"  {symbol} {message}")


def _build_node_index(
    nodes: list[dict[str, Any]],
    index: dict[str, dict[str, Any]] | None = None,
) -> dict[str, dict[str, Any]]:
    """Build an ordered dict of node_id → node in DFS traversal order."""
    if index is None:
        index = {}
    for node in nodes:
        index[node["id"]] = node
        _build_node_index(node.get("children", []), index)
    return index


def _render_catalog_text(
    nodes: list[dict[str, Any]],
    project_path: Path,
    filters: tuple[str, ...],
    include_assignments: bool,
    state_filter: set[str] | None,
    depth: int = 1,
    seen_files: set[str] | None = None,
    lines: list[str] | None = None,
) -> list[str]:
    """Render a node tree as a file-grouped, indented text catalog."""
    if lines is None:
        lines = []
    if seen_files is None:
        seen_files = set()

    for node in nodes:
        object_type = node.get("object_type", "")
        name = node.get("name", "")
        node_id = node.get("id", "")

        should_show = True
        if not include_assignments and object_type in ["assignment", "unknown"]:
            should_show = False
        if state_filter and node.get("change_status") not in state_filter:
            should_show = False
        if filters and not any(
            fnmatch.fnmatch(node_id, pat) or fnmatch.fnmatch(name, pat)
            for pat in filters
        ):
            should_show = False

        if should_show:
            file_path = node.get("file_path", "")
            if file_path and file_path not in seen_files:
                lines.append(_rel_path(file_path, project_path))
                seen_files.add(file_path)
            lines.append(f"{'  ' * depth}{name}  [{object_type}]")

        _render_catalog_text(
            node.get("children", []),
            project_path,
            filters,
            include_assignments,
            state_filter,
            depth + 1,
            seen_files,
            lines,
        )

    return lines


def _rel_path(file_path: str, base: Path) -> str:
    """Return file_path relative to base, or the original if not under base."""
    if not file_path:
        return ""
    try:
        return str(Path(file_path).relative_to(base))
    except ValueError:
        return file_path


def _flatten_nodes(
    nodes: list[dict[str, Any]],
    result: list[dict[str, Any]],
    parent_id: str | None = None,
) -> None:
    """Recursively flatten nested node tree into a flat catalog list."""
    keep_keys = {
        "id",
        "name",
        "type",
        "object_type",
        "origin",
        "parent",
        "file_path",
        "change_status",
    }
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
