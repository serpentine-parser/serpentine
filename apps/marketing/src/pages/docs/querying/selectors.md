---
layout: ../../../layouts/Docs.astro
title: Selectors
description: Slice the code reference graph down to exactly the nodes you care about.
---

`serpentine analyze` and `serpentine catalog` accept `--select` and `--exclude` to filter the graph down to just the nodes you care about. This is essential for large codebases where the full graph is too noisy, and for feeding precise context into AI agents.

## Graph operators

Selectors support graph operators that expand a matched node to include its neighbors in the reference graph. The operator syntax is inspired by [dbt's node selection](https://docs.getdbt.com/reference/node-selection/graph-operators) — if you've used dbt, the `+` and `@` operators will feel familiar.

| Selector | Meaning |
|----------|---------|
| `pattern` | Only the matching nodes |
| `+pattern` | Matching nodes **plus** everything they depend on (upstream) |
| `pattern+` | Matching nodes **plus** everything that depends on them (downstream) |
| `+pattern+` | Both directions — the full blast radius |
| `N+pattern+M` | Bounded: N hops upstream, M hops downstream |
| `@pattern` | The full connected component — everything reachable in any direction |

**Prefer bounded hops on large codebases.** `+pattern+` is unbounded and can return the entire graph if the matched node is central. Use `N+pattern+M` to limit traversal depth.

## Step 1 — Find your node IDs

Every node has a dotted ID that reflects its location in the codebase. Use `catalog` with `--filter` to discover them:

```bash
# Find everything related to auth
serpentine catalog . --filter "*auth*"

# Find a class anywhere in the project
serpentine catalog . --filter "*User*"
```

Wildcard selectors like `*.ClassName` work directly in `--select` without needing to look up the full ID first:

```bash
# Matches any node named "User" regardless of module path
serpentine analyze . --select "*.User" --pretty
```

## Step 2 — Select nodes and their dependencies

### Plain pattern — just the matching nodes

```bash
serpentine analyze . --select "src.auth.*" --pretty
```

### `+pattern` — matching nodes plus everything they depend on (upstream)

```bash
# What does the login view need to work?
serpentine analyze . --select "+*.login" --pretty
```

### `pattern+` — matching nodes plus everything that depends on them (downstream)

```bash
# What breaks if I change the User model?
serpentine analyze . --select "*.User+" --pretty
```

### `N+pattern+M` — bounded hops

Limit traversal depth to avoid pulling in the entire graph:

```bash
# 2 levels upstream, 1 level downstream from the login view
serpentine analyze . --select "2+*.login+1" --pretty
```

### `@pattern` — the full connected component

Everything reachable in any direction from the matching nodes:

```bash
serpentine analyze . --select "@src.auth.*" --pretty
```

### Multiple selectors — combined as a union

```bash
serpentine analyze . --select "+src.auth.*,+src.payments.*" --pretty
```

## Step 3 — Exclude noise

`--exclude` removes nodes from the result (including their descendants):

```bash
# Show auth and its deps, but skip test files
serpentine analyze . --select "+src.auth.*" --exclude "*test*" --pretty
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

## State selectors

When a VCS comparison is active (or after file changes are detected), nodes carry a `change_status` field. The `state:` method lets you filter the graph to only nodes with a given status:

| Selector | Meaning |
|----------|---------|
| `state:modified` | Nodes whose dependency surface changed between the two refs |
| `state:added` | Nodes that are new in the compare ref |
| `state:deleted` | Nodes that existed in the base ref but are gone now |

`state:` composes with all graph operators:

```bash
# All modified nodes plus everything they depend on
serpentine analyze . --select "+state:modified" --pretty

# Full connected component of every added node
serpentine analyze . --select "@state:added" --pretty

# Modified nodes plus 2 hops upstream
serpentine analyze . --select "2+state:modified" --pretty

# Everything modified or added (union)
serpentine analyze . --select "state:modified,state:added" --pretty
```

State selectors return an empty set when no comparison is active (no `change_status` is set). Use `--compare <ref>` to activate a comparison from the CLI. See [VCS Integration](/docs/vcs/overview) for the full feature.

## Compact output for large graphs

Use `--edges-only` to get just the edge list — much smaller than the full node tree, and sufficient for agents that only need to trace call chains:

```bash
serpentine analyze . --select "1+*.login+2" --edges-only --pretty
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
