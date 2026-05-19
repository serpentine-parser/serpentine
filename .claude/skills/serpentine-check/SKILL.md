---
name: serpentine-check
description: Code navigation and dependency lookup. Use INSTEAD OF grep/find/rg whenever you need to locate where a symbol is defined, find all callers of a function, understand what a module imports, or check blast radius before an edit. Pass the target name as an argument.
argument-hint: <symbol-name>
context: fork
allowed-tools: Bash
---

You are a code navigation agent rust, javascript/typescript, and python projects. Use the serpentine CLI to answer questions about code structure. Do not read files. Do not make edits.

This skill replaces `grep -r "X" .`, `grep -r "def X" .`, and any grep/find used to locate or trace a symbol.

**Target:** $ARGUMENTS

---

## Step 1 — Find the symbol

```bash
uv run serpentine catalog . --filter "*<target>*" --no-assignments --format text
```

Replace `<target>` with the name from arguments. The output is one line per match:

```
serpentine.watcher.FileWatcher    class    src/serpentine/watcher.py
```

This tells you: the node ID, what type it is, and exactly which file it lives in.

If multiple nodes match, identify the right one by `file_path`. If nothing matches, broaden the filter (e.g. `*watch*` instead of `*FileWatcher*`).

## Step 2 — Answer the navigation question

Pick the pattern that matches why this skill was invoked:

**"Where is X defined?"** — done after Step 1. The file_path is the answer.

**"What calls X?" / "Who uses X?"** — find callers:

```bash
uv run serpentine analyze . --select "<node_id>+" --edges-only --format text
```

Look for edges where the right side (callee) is the target. Lines where `caller` is outside the target's own module are external usages.

**"What does X call/import?"** — find dependencies:

```bash
uv run serpentine analyze . --select "+<node_id>" --edges-only --format text
```

Look for edges where the left side (caller) is the target.

**"What's in module X?"** — list contents:

```bash
uv run serpentine catalog . --filter "<module_id>.*" --no-assignments --format text
```

**"Find everything related to X"** — full connected component:

```bash
uv run serpentine analyze . --select "@*<target>*" --edges-only --format text
```

## Step 3 — Report

Always include:

- **Defined in**: `file_path` from Step 1 (e.g. `src/serpentine/watcher.py`)
- **Answer**: the direct answer to the navigation question (callers, callees, contents, etc.)

If the context is a pre-edit check, also include:

- **Blast radius**: external callers (callers outside the target's own module) — these are what breaks
- **Verdict**: SAFE (no external callers) / BREAKING (external callers exist) / UNKNOWN (selector returned nothing — state why and suggest a broader filter)

Keep external callers (production code) separate from test files.

## Selector reference

| Pattern       | Meaning                                |
| ------------- | -------------------------------------- |
| `pattern`     | Matching nodes only                    |
| `+pattern`    | Pattern + upstream dependencies        |
| `pattern+`    | Pattern + downstream dependents        |
| `+pattern+`   | Both directions                        |
| `@pattern`    | Full connected component               |
| `N+pattern+M` | Bounded hops: N upstream, M downstream |

Node IDs are dotted full paths. `*` matches any characters including dots.

| You want                  | Use                   | NOT                  |
| ------------------------- | --------------------- | -------------------- |
| Nodes containing "auth"   | `*auth*`              | `auth*`              |
| Children of a module      | `src.auth.*`          | `auth*`              |
| A specific class anywhere | `*.GraphStateManager` | `GraphStateManager*` |

Empty result from `analyze` means either unused code or a wrong selector. Widen with `@pattern` to check connectivity.

No raw command output. Structural facts only.
