# Python Server (`src/serpentine/`)

## Architecture

**Data flow**: File change → `FileWatcher` → `GraphStateManager` → Rust `_analyzer` → `ConnectionManager` → WebSocket broadcast

| Module                | Responsibility                                                                           |
| --------------------- | ---------------------------------------------------------------------------------------- |
| `state.py`            | `GraphStateManager` — single source of truth for graph data, thread-safe                 |
| `watcher.py`          | `FileWatcher` + `_DebouncedEventHandler` — watchdog-based file change detection          |
| `server/app.py`       | `create_app()` factory — Starlette app, lifespan, static serving                         |
| `server/routes.py`    | HTTP + WebSocket route handlers                                                          |
| `server/websocket.py` | `ConnectionManager` — broadcasts graph updates to all WS clients                         |
| `selector.py`         | `GraphSelector` — filters graph by selector pattern (+upstream, downstream+, @component) |
| `config.py`           | `Config` — loads `.serpentine.toml`, merges with defaults                                |
| `cli.py`              | Typer CLI — `serve`, `analyze`, `catalog`, `stats` commands                              |

## Key Patterns

- `GraphStateManager.set_broadcast_callback()` must be called before analysis
- Rust analyzer: `from serpentine import _analyzer`; call `fm.open_file(path, content)` then `fm.get_graph_json()`
- Static frontend built to `src/serpentine/static/`; run `npm run build` to update

## Conventions

- **Type hints**: All functions must have full type annotations (parameters + return type)
- **Top-level imports**: All imports at module top level — no inline imports inside functions
- **Thread safety**: `GraphStateManager` uses locks; always acquire before mutating state
- **No tests** unless explicitly asked; user runs all build/serve commands
- Use `uv` for Python: `uv run serpentine serve`

## Graph Data Model

```json
{
  "id": "...",
  "name": "...",
  "type": "module|class|function",
  "parent": "...|null",
  "children": [],
  "cfg": { "nodes": [], "edges": [] },
  "collapsed": false,
  "metadata": {}
}
```

Edges: `{"from": "node_id", "to": "node_id", "type": "calls|is-a|has-a"}`

WebSocket — client→server: `{"action": "...", "data": {}}` / server→client: `{"type": "graph_update", "data": {}}`
