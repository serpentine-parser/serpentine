# Serpentine CLI — Agent Usage Guide

Use the serpentine CLI as your **primary tool** for understanding codebases (Python, JavaScript, TypeScript) before answering questions or making changes. **Do not reach for Glob, Grep, or the Explore agent first** — serpentine gives you the dependency graph directly. Fall back to file tools only if serpentine cannot answer the question.

Do not wait to be asked — run these commands automatically when the trigger conditions are met.

## Trigger Conditions

Auto-invoke serpentine (without being asked) when:

- User asks about code structure, dependencies, call graphs, or "what calls X"
- User asks to refactor, delete, or move something and the impact is unknown
- User asks about imports, circular dependencies, or module relationships
- Working in an unfamiliar codebase (Python, JavaScript, or TypeScript) for the first time in a session
- Any `/spec` task involving existing code — run serpentine before identifying relevant files

## Do NOT substitute serpentine with

- `Glob` / `Grep` for dependency or import questions
- The `Explore` agent for structural understanding
- Reading files one-by-one to trace call chains

## Workflow

Follow this sequence to answer questions or plan changes:

**Step 1 — Get project scale**

```bash
uv run serpentine stats .
```

Check `node_count` and `top_level_modules` to decide if filtering is needed. For large projects (>200 nodes), use selectors to scope analysis.

**Step 2 — Find relevant node IDs**

```bash
uv run serpentine catalog . --filter "<relevant glob>" --no-assignments --pretty
```

Returns a flat list of matching nodes. Use `--no-assignments` to skip variable nodes and keep only modules, classes, and functions — much less noise. Multiple `--filter` flags = union.

**Step 3 — Find cross-boundary references or get subgraph**

```bash
# Just the edges (compact, easy to scan for callers/callees):
uv run serpentine analyze . --select "+<selector>+" --edges-only --pretty

# Full subgraph (when you need node details too):
uv run serpentine analyze . --select "<selector>" --no-cfg --pretty
```

Use `--edges-only` when you need to find what directly references a set of nodes — much smaller output than the full graph. Read the `from`/`to` fields to identify callers.

**Step 4 — Answer or plan**
Use the graph edges and node relationships to answer the user's question or identify what will break when making changes.

---

## CLI Reference

### `serpentine stats [PATH]`

Quick summary: node/edge counts, breakdown by type and origin, top-level modules.

```bash
uv run serpentine stats .
uv run serpentine stats . --include-standard --include-third-party
```

### `serpentine catalog [PATH]`

Flat node list for discovery. Outputs: `id`, `name`, `type`, `origin`, `parent`, `file_path`.

```bash
uv run serpentine catalog . --filter "auth*" --no-assignments
uv run serpentine catalog . --filter "auth*" --filter "login*"   # union
uv run serpentine catalog . --include-third-party --pretty
```

`--no-assignments` removes variable/assignment nodes. Use it by default — you almost never need them for structural understanding.

### `serpentine analyze [PATH]`

Full dependency graph as JSON. Supports selectors for focused output.

```bash
uv run serpentine analyze . --pretty
uv run serpentine analyze . --select "auth*" --no-cfg --pretty
uv run serpentine analyze . --select "+auth*" --exclude "test_*" --no-cfg --pretty
uv run serpentine analyze . --select "+auth*+" --edges-only --pretty   # just edges
```

`--edges-only` outputs only the edges array — use when you need to identify callers/callees without the full node tree.

### Selector Syntax

| Pattern           | Meaning                                |
| ----------------- | -------------------------------------- |
| `pattern`         | Nodes matching pattern                 |
| `+pattern`        | Pattern + all upstream dependencies    |
| `pattern+`        | Pattern + all downstream dependents    |
| `@pattern`        | Full connected component               |
| `N+pattern+M`     | Bounded hops: N upstream, M downstream |
| `pattern,pattern` | Nodes matching both patterns           |

### Pattern Matching Notes

Node IDs are **dotted full paths** (e.g., `src.serpentine.details`, `src.serpentine.state.GraphStateManager`).
`*` matches any characters **including dots**. `**` is equivalent to `*`.

| You want                        | Use                        | NOT                      |
| ------------------------------- | -------------------------- | ------------------------ |
| Nodes containing "details"      | `*details*`                | `details*`               |
| All children of a module        | `src.serpentine.*`         | `serpentine*`            |
| A specific nested class         | `*.GraphStateManager`      | `GraphStateManager*`     |

`details*` only matches nodes whose full ID **starts** with `details` — it misses `src.serpentine.details`.

All PATH arguments default to `.` (current directory). All output is JSON to stdout. Progress lines go to stderr and won't interfere with piping.

---

## Example: "What depends on the auth module?"

```bash
# 1. Find structural nodes for auth (skip variables)
uv run serpentine catalog . --filter "auth*" --no-assignments --pretty

# 2. Get edges for everything that calls into auth (compact)
uv run serpentine analyze . --select "+auth*+" --edges-only --pretty
```

Scan the edges for `to` values matching auth nodes — those `from` values are the callers.

## Example: "Is it safe to delete module X?"

```bash
# Find what directly references X (1 hop downstream of X's callers)
uv run serpentine analyze . --select "X+" --edges-only --pretty
```

Any edges with `from` outside of X indicate dependents that would break.
