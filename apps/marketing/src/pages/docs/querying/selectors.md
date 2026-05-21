---
layout: ../../../layouts/Docs.astro
title: Selectors
description: Slice the code reference graph down to exactly the nodes you care about.
---

`serpentine analyze` and `serpentine catalog` accept `--select` and `--exclude` to filter the graph down to just the nodes you care about. This is essential for large codebases where the full graph is too noisy, and for feeding precise context into AI agents.

## Step 1 — Find your node IDs

Every node has a dotted ID that reflects its location in the codebase. Use `catalog` with `--format json` to discover them:

```bash
serpentine catalog . --format json --pretty
```

Narrow it down with `--filter` (glob matched against both ID and name):

```bash
# Find everything related to auth
serpentine catalog . --filter "*auth*" --format json --pretty

# Find by name across modules
serpentine catalog . --filter "*User*" --format json --pretty
```

## Step 2 — Select nodes and their dependencies

Once you have IDs, use `--select` with `serpentine analyze`.

### Plain pattern — just the matching nodes

```bash
serpentine analyze --select "src.auth.*" --no-cfg --pretty
```

### `+pattern` — matching nodes plus everything they depend on (upstream)

```bash
# What does the login view need to work?
serpentine analyze --select "+src.auth.views.login" --no-cfg --pretty
```

### `pattern+` — matching nodes plus everything that depends on them (downstream)

```bash
# What breaks if I change the User model?
serpentine analyze --select "src.auth.models.User+" --no-cfg --pretty
```

### `+pattern+` — both directions (full blast radius)

```bash
serpentine analyze --select "+src.payments.*+" --no-cfg --pretty
```

### `@pattern` — the full connected component

Everything reachable in any direction from the matching nodes:

```bash
serpentine analyze --select "@src.auth.*" --no-cfg --pretty
```

### Bounded hops — limit traversal depth

```bash
# 2 levels upstream, 1 level downstream
serpentine analyze --select "2+src.auth.views.login+1" --no-cfg --pretty
```

### Multiple selectors — combined as a union

```bash
serpentine analyze --select "+src.auth.*,+src.payments.*" --no-cfg --pretty
```

## Step 3 — Exclude noise

`--exclude` removes nodes from the result (including their descendants):

```bash
# Show auth and its deps, but skip test files
serpentine analyze --select "+src.auth.*" --exclude "*test*" --no-cfg --pretty
```

## Glob pattern rules

`*` in a selector matches any characters **including dots**, so it crosses module boundaries:

| You want | Use |
|----------|-----|
| All nodes whose ID contains "auth" | `*auth*` |
| All children of a specific module | `src.auth.*` |
| A class anywhere in the project | `*.User` |
| All test files | `*test*` |

> `**` is equivalent to `*` — both match across dots.

## Compact output for large graphs

Use `--edges-only` to get just the edge list — much smaller than the full node tree, and sufficient for agents that only need to trace call chains:

```bash
serpentine analyze --select "+src.auth.*+" --edges-only --pretty
```

Output:

```json
{
  "edges": [
    { "from": "src.auth.views.login", "to": "src.auth.models.User.get", "type": "calls" },
    { "from": "src.auth.views.login", "to": "src.auth.forms.LoginForm", "type": "has-a" }
  ]
}
```
