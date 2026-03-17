# Project

Fast dependency graph analysis and visualization for projects.

Serpentine analyzes your Python and JavaScript codebases and displays an interactive dependency graph and cfg in your browser. It watches for file changes and updates the graph in real-time.

## Features

- **Fast Analysis**: Rust-powered parser using tree-sitter for blazing fast code analysis
- **Interactive Graph**: Visual dependency graph with expandable nodes
- **Real-time Updates**: File watcher detects changes and updates the graph via WebSocket

## Reading Files

- Before reading any file, check if it has already been read this session
- Never re-read a file you've already seen unless explicitly asked to verify changes
- When modifying a file you've already read, work from memory — don't re-read it first

## Architecture Overview

Serpentine is a Python dependency & cfg graph analyzer with a Rust backend for performance. It comprises three key layers:

### 1. **Rust Analyzer** (`rust/src/`)

- **Message Bus Pattern**: Events flow through a multi-threaded message bus (`message_bus.rs`)
- **Subscribers**: Register specialized processors (ImportSubscriber, DefinitionsSubscriber, etc.) that handle events in parallel
- **Multi-language Support**: Tree-sitter parsers for Python, JavaScript
- **PyO3 Bindings**: Core analyzer exported as `serpentine._analyzer` module

**Key Files**: `lib.rs`, `python/`, `subscribers/`

### 2. **Python Server** (`src/serpentine/`)

- **State Management**: `GraphStateManager` in `state.py` is the single source of truth for graph data
- **File Watching**: `FileWatcher` detects changes, triggers re-analysis via watchdog
- **WebSocket Broadcasting**: `ConnectionManager` pushes real-time updates to frontend
- **Starlette Web Server**: Factory pattern in `app.py` for clean dependency injection

**Data Flow**: File changes → Watcher → State Manager → Rust analyzer → WebSocket broadcast

### 3. **Frontend** (`frontend/app/`)

- **Zustand Store**: `useGraphStore` manages all UI state (nodes, edges, viewport, selection)
- **D3 Visualization**: `Graph.tsx` renders dependency graph with interactive features
- **Real-time Updates**: WebSocket client receives graph updates and refreshes store

---

### Adding Analyzer Subscribers

1. Implement `Subscriber` trait in `rust/src/subscribers/` (see `definitions.rs` for pattern)
2. Create corresponding `Factory` struct implementing `SubscriberFactory`
3. Register in `lib.rs`'s `create_analyzer()` function
4. Subscriber receives events, returns JSON via `finalize()`

### Frontend Component Patterns

- Use `useGraphStore()` for all state access (not prop drilling)
- Components selectors: `const nodes = useGraphStore(s => s.nodes)` - triggers re-render only when that value changes
- Layout/collision logic in hooks (`layoutEngine.ts`, `collisionDetector.ts`), not components

---

## Key Conventions & Patterns

### Python Module Organization

- **`main.py`**: Deprecated initial implementation; use `watcher.py` and `state.py` instead
- **File extensions**: Centralized list `SUPPORTED_EXTENSIONS = {".py", ".js", ".jsx"}` maintained in multiple places (`state.py`, `watcher.py`, main.py`)
- **Thread-safety**: `GraphStateManager` uses locks for state updates; check `IGNORED_DIRECTORIES` before watching

### Graph Data Model

All nodes follow pattern:

```json
{
  "id": "unique_id",
  "name": "item_name",
  "type": "module|class|function", // enum: not arbitrary
  "parent": "parent_node_id or null",
  "children": [],
  "cfg": {
    "edges": [],
    "nodes": []
  },
  "collapsed": false,
  "metadata": {}
}
```

Edges: `{from: node_id, to: node_id, type: "calls|is-a|has-a"}`

### WebSocket Protocol

- Frontend → Server: `{"action": "...", "data": {...}}`
- Server → Frontend: `{"type": "graph_update", "data": {...}}` broadcasts to all clients
- See `routes.py:ws()` for expected message formats

### State Change Notifications

`GraphStateManager.set_broadcast_callback()` must be called before analysis. This enables real-time updates:

```python
state = GraphStateManager()
state.set_broadcast_callback(lambda: manager.broadcast_async(...))
```

---

## Important Integration Points

### Rust-Python Boundary

- Analyzer instantiated once: `from serpentine import _analyzer` (PyO3 module)
- `FileManager` object maintains open files and parse tree state (mutable, thread-unsafe)
- Must call `fm.open_file(path, content)` before analysis; results queried via `fm.get_graph_json()`

### Static Frontend Assets

- Built frontend output goes to `src/serpentine/static/`
- Server serves from this directory; must rebuild frontend for changes to appear
- In dev: `frontend/` is watched, but manual `npm run build` needed before `serpentine serve`

### MCP Server Integration

- Optional MCP server at `/mcp` endpoint (enable with `--mcp` flag)
- Shares same `GraphStateManager` as UI; no separate analysis
- Edit proposals flow through state manager to UI via WebSocket

---

## Debugging & Common Tasks

- Don't write tests unless specfically instructed.
- Do not run tests, attempt to build or run any server commands. The user will do all those steps themselves.
- Use `uv` to run any python commands (e.g. `uv run serpentine serve`)

## Dependency Graph Correctness Requirements

This is a **professional-grade static analysis tool**. The graph must resolve dependencies down to the variable/attribute level — not just import-level. Import edges alone are not sufficient.

### The full pipeline for each language

Every language walker MUST emit all three event types to get complete resolution:

1. **`ImportStatement`** — feeds `load_import_bindings` (builds the G-level of LEGB: maps local import names to their actual definition qualnames, through `reexport_map`)
2. **`UseName`** — feeds `load_uses` → `resolve_name_legb` → creates edges for every variable read, type reference, JSX element, etc.
3. **`CallExpression`** — feeds `load_raw_bindings` CALLS pass → `resolve_callee` → creates call edges

**A language that only emits `ImportStatement` and `CallExpression` is incomplete.** `UseName` is required for full coverage.

### Language walker UseName requirements

- **Python** (`rust/src/python/mod.rs`): emits `UseName` via `emit_identifier_events` — complete.
- **JavaScript/TypeScript** (`rust/src/javascript/mod.rs`): emits `UseName` via `emit_identifier_use` in `walk_node` — complete as of this implementation. Do NOT remove or short-circuit the `"identifier" | "type_identifier" | "jsx_identifier"` arm in `walk_node`.
- **Rust lang** (`rust/src/rust_lang/mod.rs`): emits `UseName` via `emit_use_events` — complete.

### Re-export map must cover all languages

`build_reexport_map` in `rust/src/graph/loaders.rs` must use `lang_configs.iter().any(|cfg| cfg.is_reexport_file(file))` — NOT a hardcoded `file.ends_with("__init__.py")` check. This ensures JS `index.ts` and Rust `mod.rs` re-exports are resolved the same way Python `__init__.py` re-exports are.

### JS path alias resolution

`read_tsconfig_aliases_from` in `rust/src/javascript/mod.rs` reads `tsconfig.json` `compilerOptions.paths`. Aliases can map to a **file** (e.g. `"@store": ["./src/store.ts"]`), not just a directory. The `.ts`/`.tsx`/`.js`/`.jsx`/`.mjs` extension MUST be stripped from the target before use — otherwise the resolved module name won't match any node in the graph. This is implemented in `read_tsconfig_aliases_from`; do not regress it.

## References

- **Rust**: Message bus processes events in parallel; each subscriber is thread-safe, results merged
- **State**: Thread-safe dict with lock; listeners notified via broadcast callback
- **Store**: Zustand auto-memoizes selector results; filters/search computed on-demand
