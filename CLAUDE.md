# Project

Fast dependency graph analysis and visualization for projects.

## Dev Environment

The project runs under **Tilt** — the server auto-reloads whenever source files change. There is no need to manually restart the server after edits; changes take effect immediately.

**Never use `grep`, `find`, `rg`, or the Read tool for code navigation.** Serpentine is the replacement for all of these. Use the skill below instead:

| Instead of                            | Use                            |
| ------------------------------------- | ------------------------------ |
| `grep -r "ClassName" .`               | `/code-analysis ClassName`     |
| `grep -r "def function_name" .`       | `/code-analysis function_name` |
| `find . -name "*.py" \| xargs grep X` | `/code-analysis X`             |
| `ls src/module/` or `find . -type f`  | `/code-analysis`               |
| `cat file.py` or Read tool            | `/code-analysis SymbolName`    |
| Understanding module structure        | `/code-analysis`               |

- `/code-analysis <target>` — find where a symbol is defined, read its source code, trace callers/callees, and get its blast radius before an edit. Also handles structural questions ("what's in module X?", "what are the top-level modules?"). Pass one or more symbol names or questions. Think carefully, and use it a limited number of times to answer the question about the codebase you have.

`grep`/`find`/`rg` are only permitted when serpentine cannot answer the question (e.g. searching for a literal string value inside file contents, not for code structure). The Read tool is only permitted when explicitly asked to verify a file after an edit.

## Features

- **Fast Analysis**: Rust-powered parser using tree-sitter for blazing fast code analysis
- **Interactive Graph**: Visual dependency graph with expandable nodes
- **Real-time Updates**: File watcher detects changes and updates the graph via WebSocket

## Reading Files

- Before reading any file, check if it has already been read this session
- Never re-read a file you've already seen unless explicitly asked to verify changes
- When modifying a file you've already read, work from memory — don't re-read it first

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

---

## Debugging & Common Tasks

- Don't write tests unless specfically instructed.
- Do not run tests, attempt to build or run any server commands. The user will do all those steps themselves.
- Use `uv` to run any python commands (e.g. `uv run serpentine serve`)
