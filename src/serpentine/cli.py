"""
Serpentine CLI - Command-line interface for code reference graph analysis.

This module provides the main entry point for the serpentine tool.
Commands are organized by concern:
- `init`: Initialize a project — writes config, updates .gitignore, installs harness instructions
- `stats`: Quick project scale summary (node/edge counts, top-level modules)
- `catalog`: Flat node list for discovery; supports glob filtering
- `analyze`: Full graph query with selector syntax; outputs JSON or annotated source text
- `serve`: Start the web server with live code reference graph UI and optional file watching
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
from serpentine.adapters import DiskSourceProvider
from serpentine.cache import VcsRefCacheManager
from serpentine.domain import (
    apply_filters,
    filter_by_origin,
    get_catalog,
    get_stats,
    inject_source_on_demand,
)
from serpentine.server import create_app
from serpentine.state import GraphStateManager
from serpentine.vcs.backend import detect_backend
from serpentine.vcs.manager import VcsManager
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
    - .tf
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
    """Serpentine: Fast code reference graph analysis for Python, JS/TS, and Rust projects."""
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
    """Start the serpentine web server with live code reference graph visualization.

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

    # Set up VCS manager if a backend is available
    vcs_manager: VcsManager | None = None
    backend = detect_backend(project_path)
    if backend is not None:
        vcs_manager = VcsManager(
            backend, VcsRefCacheManager(project_path), state_manager.config
        )
        click.echo("🔀 VCS integration enabled")

    # Create the web application
    app = create_app(
        state_manager, static_dir=_get_static_dir(), vcs_manager=vcs_manager
    )

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
    help="Node selector pattern (e.g. '*.Symbol', '+*.Symbol', '*.Symbol+', '@*.Symbol')",
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
    "--edge-type",
    "edge_types",
    type=str,
    default=None,
    help="Comma-separated edge types to include: calls,is-a,has-a,references,imports",
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
@click.option(
    "--compare",
    "compare_ref",
    type=str,
    default=None,
    help="VCS ref to compare against (branch, tag, or commit hash). Shows added/removed/modified nodes.",
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
    edge_types: str | None,
    fmt: str,
    source: bool,
    include_assignments: bool,
    compare_ref: str | None,
) -> None:
    """Analyze a project and output the code reference graph.

    PATH is the directory to analyze (defaults to current directory).

    Examples:
        serpentine analyze                                        # Text edges to stdout
        serpentine analyze --select "*.AuthService" --source      # Source + edges for a symbol
        serpentine analyze --select "+*.AuthService" --source     # Include upstream dependencies
        serpentine analyze --format json --pretty                 # Full graph as pretty JSON
        serpentine analyze --format json --select "auth*" --no-cfg --pretty
        serpentine analyze --edge-type calls,is-a                 # Only call and inheritance edges
    """
    project_path = Path(path).resolve()
    state_manager = GraphStateManager(project_path)
    state_manager.analyze_project(project_path, force_fresh=source)

    if compare_ref is not None:
        backend = detect_backend(project_path)
        if backend is None:
            raise click.ClickException(
                "No VCS backend found. Is this a git repo? "
                "Install git support with: pip install serpentine[git]"
            )
        try:
            vcs_manager = VcsManager(
                backend, VcsRefCacheManager(project_path), state_manager.config
            )
            from_graph_json = vcs_manager.get_graph_at(compare_ref)
        except Exception as e:
            raise click.ClickException(
                f"Could not resolve ref '{compare_ref}': {e}"
            ) from e
        state_manager.set_vcs_comparison(from_graph_json, to_graph_json=None)

    graph_data = apply_filters(
        state_manager.get_graph_data(),
        select=select,
        exclude=exclude,
        state=state,
        include_standard=include_standard,
        include_third_party=include_third_party,
    )

    if edge_types:
        allowed = {t.strip() for t in edge_types.split(",") if t.strip()}
        graph_data["edges"] = [
            e for e in graph_data.get("edges", []) if e.get("type") in allowed
        ]

    if no_cfg:
        _strip_cfg(graph_data.get("nodes", []))

    if fmt == "text":
        lines: list[str] = []
        if source:
            inject_source_on_demand(graph_data, DiskSourceProvider())
            node_index = _build_node_index(graph_data.get("nodes", []))
            edges_by_caller: dict[str, list[dict[str, Any]]] = {}
            for e in graph_data.get("edges", []):
                edges_by_caller.setdefault(e.get("caller", ""), []).append(e)
            for node_id, node in node_index.items():
                object_type = node.get("object_type", "")
                if not include_assignments and object_type in {"assignment", "unknown"}:
                    continue
                pos = node.get("position", [0, 0])
                rel = _rel_path(node.get("file_path", ""), project_path)
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
@click.option(
    "--compare",
    "compare_ref",
    type=str,
    default=None,
    help="VCS ref to compare against (branch, tag, or commit hash). Shows added/removed/modified nodes.",
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
    compare_ref: str | None,
) -> None:
    """Flat list of all nodes for agent discovery.

    Use this to find relevant node IDs before constructing selectors for analyze.

    PATH is the directory to analyze (defaults to current directory).

    Examples:
        serpentine catalog .
        serpentine catalog . --filter "auth*" --filter "login*"
        serpentine catalog . --include-assignments --filter "auth*"
        serpentine catalog . --include-third-party --format json --pretty
    """
    project_path = Path(path).resolve()
    state_manager = GraphStateManager(project_path)
    state_manager.analyze_project(project_path)

    if compare_ref is not None:
        backend = detect_backend(project_path)
        if backend is None:
            raise click.ClickException(
                "No VCS backend found. Is this a git repo? "
                "Install git support with: pip install serpentine[git]"
            )
        try:
            vcs_manager = VcsManager(
                backend, VcsRefCacheManager(project_path), state_manager.config
            )
            from_graph_json = vcs_manager.get_graph_at(compare_ref)
        except Exception as e:
            raise click.ClickException(
                f"Could not resolve ref '{compare_ref}': {e}"
            ) from e
        state_manager.set_vcs_comparison(from_graph_json, to_graph_json=None)

    graph_data = state_manager.get_graph_data()

    if fmt == "text":
        graph_data = filter_by_origin(graph_data, include_standard, include_third_party)
        state_filter = (
            {s.strip() for s in state.split(",") if s.strip()} if state else None
        )
        lines = _render_catalog_text(
            graph_data.get("nodes", []),
            project_path,
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

    # Build catalog via service, then apply CLI-specific glob filters
    flat_nodes = get_catalog(
        graph_data,
        include_assignments=include_assignments,
        include_standard=include_standard,
        include_third_party=include_third_party,
        state=state,
    )

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

    graph_data = filter_by_origin(
        state_manager.get_graph_data(), include_standard, include_third_party
    )
    result = get_stats(graph_data)
    click.echo(json.dumps(result, indent=2) if pretty else json.dumps(result))


_SUPPORTED_HARNESSES = ("claude", "cursor", "copilot", "codex", "opencode")


def _detect_harnesses(project_path: Path) -> list[str]:
    detected = []
    if (project_path / ".claude").is_dir():
        detected.append("claude")
    if (project_path / ".cursor").is_dir():
        detected.append("cursor")
    if (project_path / ".github" / "copilot-instructions.md").exists() or (
        project_path / ".vscode"
    ).is_dir():
        detected.append("copilot")
    if (project_path / "AGENTS.md").exists():
        detected.append("codex")
    if (project_path / ".opencode").is_dir():
        detected.append("opencode")
    return detected or ["claude"]


def _init_claude(project_path: Path) -> list[tuple[str, str]]:
    results: list[tuple[str, str]] = []

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

    return results


def _init_cursor(project_path: Path) -> list[tuple[str, str]]:
    rule_path = project_path / ".cursor" / "rules" / "serpentine.mdc"
    rel = str(rule_path.relative_to(project_path))
    if rule_path.exists():
        return [("⚠", f"{rel} — already exists, skipped")]
    try:
        content = (
            _res_files("serpentine.skills")
            .joinpath("cursor-rules.md")
            .read_text(encoding="utf-8")
        )
    except FileNotFoundError:
        return [
            ("✗", f"{rel} — content not found in package, try reinstalling serpentine")
        ]
    try:
        rule_path.parent.mkdir(parents=True, exist_ok=True)
        rule_path.write_text(content)
        return [("✓", f"{rel} installed")]
    except (PermissionError, OSError) as e:
        return [("✗", f"{rel} — {e}")]


def _init_copilot(project_path: Path) -> list[tuple[str, str]]:
    instructions_path = project_path / ".github" / "copilot-instructions.md"
    rel = str(instructions_path.relative_to(project_path))
    try:
        section = (
            _res_files("serpentine.skills")
            .joinpath("copilot-instructions.md")
            .read_text(encoding="utf-8")
        )
    except FileNotFoundError:
        return [
            ("✗", f"{rel} — content not found in package, try reinstalling serpentine")
        ]
    try:
        if instructions_path.exists():
            content = instructions_path.read_text()
            if "## Serpentine" in content:
                return [("⚠", f"{rel} — serpentine section already present")]
            sep = "" if content.endswith("\n") else "\n"
            instructions_path.write_text(f"{content}{sep}\n{section}")
            return [("✓", f"{rel} updated")]
        instructions_path.parent.mkdir(parents=True, exist_ok=True)
        instructions_path.write_text(section)
        return [("✓", f"{rel} created")]
    except (PermissionError, OSError) as e:
        return [("✗", f"{rel} — {e}")]


def _init_agents_md(project_path: Path, label: str) -> list[tuple[str, str]]:
    agents_path = project_path / "AGENTS.md"
    try:
        section = (
            _res_files("serpentine.skills")
            .joinpath("agents-md.md")
            .read_text(encoding="utf-8")
        )
    except FileNotFoundError:
        return [
            (
                "✗",
                "AGENTS.md — content not found in package, try reinstalling serpentine",
            )
        ]
    try:
        if agents_path.exists():
            content = agents_path.read_text()
            if "## Serpentine" in content:
                return [
                    (
                        "⚠",
                        f"AGENTS.md — serpentine section already present (skipped for {label})",
                    )
                ]
            sep = "" if content.endswith("\n") else "\n"
            agents_path.write_text(f"{content}{sep}\n{section}")
            return [("✓", f"AGENTS.md updated (for {label})")]
        agents_path.write_text(section)
        return [("✓", f"AGENTS.md created (for {label})")]
    except (PermissionError, OSError) as e:
        return [("✗", f"AGENTS.md — {e}")]


@main.command()
@click.argument("path", type=click.Path(exists=True), default=".")
@click.option(
    "--harness",
    "harnesses",
    multiple=True,
    type=click.Choice(_SUPPORTED_HARNESSES),
    help="Harness to initialize (can be repeated). Auto-detects if not specified.",
)
def init(path: str, harnesses: tuple[str, ...]) -> None:
    """Initialize a project for use with serpentine.

    Generates .serpentine.yml, updates .gitignore, and installs serpentine
    instructions for your AI coding harness. Supports Claude Code, Cursor,
    GitHub Copilot, Codex, and OpenCode. Auto-detects which harnesses are in
    use; use --harness to target a specific one.

    PATH is the directory to initialize (defaults to current directory).

    Examples:
        serpentine init                      # Auto-detect harnesses
        serpentine init --harness cursor     # Cursor only
        serpentine init --harness claude --harness copilot
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

    # Step 3: Harness-specific setup
    active = list(harnesses) if harnesses else _detect_harnesses(project_path)
    for harness in active:
        if harness == "claude":
            results.extend(_init_claude(project_path))
        elif harness == "cursor":
            results.extend(_init_cursor(project_path))
        elif harness == "copilot":
            results.extend(_init_copilot(project_path))
        elif harness == "codex":
            results.extend(_init_agents_md(project_path, "Codex"))
        elif harness == "opencode":
            results.extend(_init_agents_md(project_path, "OpenCode"))

    harness_label = ", ".join(active)
    click.echo(f"\nserpentine init complete ({harness_label}):")
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


@main.group()
def mcp() -> None:
    """MCP server commands for remote codebase analysis."""
    pass


@mcp.command("serve")
@click.option("--host", default="127.0.0.1", help="Bind host (default: 127.0.0.1)")
@click.option("--port", default=8001, type=int, help="Bind port (default: 8001)")
def mcp_serve(host: str, port: int) -> None:
    """Start the MCP server (Streamable HTTP via FastMCP)."""
    try:
        from serpentine.mcp.auth import build_jwt_verifier
        from serpentine.mcp.server import create_mcp_app
        from serpentine.storage.factory import build_store
        from serpentine.vcs.factory import build_vcs_managers
    except ImportError as e:
        raise click.ClickException(
            f"MCP dependencies missing: {e}. Install with: pip install serpentine[mcp]"
        )

    try:
        auth = build_jwt_verifier()
        store = build_store()
        vcs_managers = build_vcs_managers()
    except Exception as e:
        raise click.ClickException(str(e))

    click.echo(
        f"Loaded {len(vcs_managers)} repo(s): {', '.join(vcs_managers) or '(none)'}",
        err=True,
    )

    app = create_mcp_app(store, vcs_managers, auth=auth)
    click.echo(f"Starting MCP server at http://{host}:{port}", err=True)
    uvicorn.run(app.http_app(), host=host, port=port)


@mcp.command("ingest")
@click.argument("repo_id")
@click.argument("ref", default="")
@click.option(
    "--all-refs", is_flag=True, default=False, help="Ingest all refs for this repo"
)
@click.option(
    "--ignore-config",
    is_flag=True,
    default=False,
    help="Use default Config even without .serpentine.toml",
)
def mcp_ingest(repo_id: str, ref: str, all_refs: bool, ignore_config: bool) -> None:
    """Ingest a repo ref into the graph store."""
    try:
        from serpentine.services import ingest_ref as _ingest_ref
        from serpentine.storage.factory import build_store
        from serpentine.vcs.factory import build_vcs_manager
    except ImportError as e:
        raise click.ClickException(
            f"MCP dependencies missing: {e}. Install with: pip install serpentine[mcp,git]"
        )

    try:
        store = build_store()
        vcs = build_vcs_manager(repo_id)
    except Exception as e:
        raise click.ClickException(str(e))

    if all_refs:
        refs_to_ingest = [r.id for r in vcs.list_refs()]
    elif ref:
        refs_to_ingest = [ref]
    else:
        raise click.ClickException("Provide a <ref> argument or use --all-refs")

    for r in refs_to_ingest:
        try:
            commit_hash = _ingest_ref(
                vcs, store, repo_id, r, ignore_config=ignore_config
            )
            click.echo(f"Ingested {repo_id}/{r} → {commit_hash[:7]}")
        except Exception as e:
            click.echo(f"Error ingesting {repo_id}/{r}: {e}", err=True)


if __name__ == "__main__":
    main()
