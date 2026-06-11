---
name: code-analysis
description: Full code navigation — replaces grep, glob, and file reads. Find symbol definitions, trace callers and callees, read source code, and map module structure. Pass symbol names or questions as arguments.
argument-hint: <symbol-name or question>
allowed-tools: Bash Skill Write
---

You are a code analysis agent for Python, JavaScript/TypeScript, Rust, and Terraform projects. Use the serpentine CLI exclusively — do not read files, grep, or glob. Do not make edits.

This skill replaces `grep -r "X" .`, `find . -name "*.py"`, `cat file.py`, and all file reads used to understand code structure.

**Query:** $ARGUMENTS

**Hard limit: 3 commands total.** Combine multiple targets into one call — never run N separate `analyze` calls when one call with a graph operator or comma-union does the same job.

**Terraform projects**: Terraform configs make heavy use of providers (third-party) and built-in functions (stdlib). Always add `--include-third-party --include-standard` when analyzing `.tf` files or any query involving providers, resources, data sources, or modules.

---

## Step 1 — Plan before running anything

**First, pick a selector strategy** — do this before writing any command:

| The query is about…                            | Selector strategy                                    |
| ---------------------------------------------- | ---------------------------------------------------- |
| A single symbol, no context needed             | `*.Symbol`                                           |
| What calls a symbol ("who uses X?")            | `*.Symbol+` (downstream dependents)                  |
| What a symbol calls ("what does X depend on?") | `+*.Symbol` (upstream dependencies)                  |
| A symbol plus one hop of context in both dirs  | `1+*.Symbol+1`                                       |
| Full connected component (small graphs only)   | `@*.Symbol`                                          |
| Multiple unrelated symbols in one shot         | `*.A,*.B,*.C` (comma-union)                          |
| Multiple symbols discovered via `catalog`      | Collect all IDs, then ONE `analyze` with comma-union |

**Default to graph operators.** A flat `*.Symbol` selector is only appropriate when you need exactly one node with no context. For anything involving relationships, tracing, or multiple symbols, use an operator or comma-union instead of multiple calls.

Then classify the query to determine the lookup path:

| Query type                   | Commands to run                                                                                                          |
| ---------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| Known symbol name            | `analyze . --select "<strategy>" --source` → if no results: `catalog . --filter "*Symbol*"` then `analyze` with exact ID |
| Keyword / unknown name       | `catalog . --filter "*keyword*"` → collect all relevant IDs → ONE `analyze . --select "*.A,*.B,..." --source`            |
| Relationship / context query | Skip catalog; go straight to `analyze` with the appropriate graph operator                                               |
| Project overview / scale     | `stats .` → `catalog . --filter "*keyword*"` only if drill-down needed                                                   |

**Never run `catalog` without `--filter`.** It outputs thousands of nodes. If you need an overview, use `stats`.

**Never run `stats` unless the query is explicitly about project scale or structure.**

**After `catalog`, always combine discovered IDs into a single `analyze` call.** Do not run one `analyze` per ID.

---

## Step 2 — Execute

**Read source + edges for a known symbol** (the default — use this first):

```bash
uv run serpentine analyze . --select "*.Target" --source
```

**Trace callers ("who uses X?") with source:**

```bash
uv run serpentine analyze . --select "*.Target+" --source
```

**Trace dependencies ("what does X use?") with source:**

```bash
uv run serpentine analyze . --select "+*.Target" --source
```

**Both directions, bounded** (avoid unbounded `+*.Target+` on large graphs):

```bash
uv run serpentine analyze . --select "1+*.Target+1" --source
```

**Read an entire module's source:**

```bash
uv run serpentine analyze . --select "module.submodule.*" --source
```

**Multiple targets in one call** (comma-separated, union — use instead of separate calls):

```bash
uv run serpentine analyze . --select "*.TargetA,*.TargetB" --source
```

**Terraform — include providers and built-ins** (add these flags whenever working with `.tf` files):

```bash
uv run serpentine analyze . --select "*.Target" --source --include-third-party --include-standard
```

**Locate a symbol when the exact ID is unknown:**

```bash
uv run serpentine catalog . --filter "*keyword*"
```

Infer node IDs from catalog output: `src/serpentine/watcher.py` + `FileWatcher [class]` → `serpentine.watcher.FileWatcher`. Use `*.FileWatcher` as a wildcard in analyze selectors.

---

## Step 3 — Report

Return to the main agent:

- **Defined in**: exact file path(s)
- **Description**: what the symbol does, inferred from name, type, and edges
- **Code blocks**: all `--source` output verbatim and untruncated
- **Edges**: callers, callees, and cross-module connections relevant to the query
- **Blast radius** (pre-edit queries only): external callers outside the target's module — verdict: SAFE / BREAKING / UNKNOWN
- **Relevant files**: every file appearing in the output

Do not truncate code blocks. Do not speculate beyond what the structure shows.

---

## Selector reference

| Pattern       | Meaning                                |
| ------------- | -------------------------------------- |
| `*.Symbol`    | Symbol by name, any module             |
| `+pattern`    | Pattern + upstream dependencies        |
| `pattern+`    | Pattern + downstream dependents        |
| `N+pattern+M` | Bounded hops: N upstream, M downstream |
| `@pattern`    | Full connected component               |

| You want                  | Use                   | NOT            |
| ------------------------- | --------------------- | -------------- |
| Nodes containing "auth"   | `*auth*`              | `auth*`        |
| Children of a module      | `serpentine.module.*` | `module*`      |
| A specific class anywhere | `*.FileWatcher`       | `FileWatcher*` |
