---
layout: ../../../layouts/Docs.astro
title: VCS Integration
description: Compare any two git refs and see structural changes highlighted on the graph.
---

Serpentine's VCS integration lets you compare any two points in your git history and see the structural diff directly on the code reference graph — added, modified, and deleted nodes highlighted in place.

---

## Requirements

VCS support requires the `git` optional extra:

```bash
pip install "serpentine-parser[git]"
# or
uv tool install "serpentine-parser[git]"
```

This installs [pygit2](https://github.com/libgit2/pygit2), which reads git history directly without requiring the `git` binary. The feature is automatically enabled when you run `serpentine serve` inside a git repository. If the repo has no git history or the `[git]` extra is not installed, the compare toolbar is hidden.

---

## The compare toolbar

When VCS is available, a compare toolbar appears in the header:

```
compare  [At checkpoint] ← [Current (live)]
```

The two dropdowns select the **base** (left) and **compare** (right) refs. The `←` arrow reads "compare compared against base" — the same direction as a GitHub pull request (`feature ← main`).

### Ref types

The dropdowns list three categories of refs:

| Category | What it contains |
|----------|-----------------|
| **Current (live)** | The graph as it exists on disk right now |
| **At checkpoint** | A snapshot you saved with the Checkpoint button |
| **Branches** | All local git branches |
| **Tags** | All git tags |
| **Recent commits** | The last 100 commits on HEAD |

Type in the search box to filter the list. Use arrow keys to navigate, Enter to select, Escape to close.

### Keyboard navigation

The ref dropdowns support full keyboard navigation:

| Key | Action |
|-----|--------|
| `↓` / `↑` | Move through the list |
| `Enter` | Select the highlighted ref |
| `Escape` | Close without selecting |
| Click outside | Close without selecting |

### Making a comparison

1. Click the **base** dropdown (left side of `←`) and select a ref — for example, the `main` branch.
2. Click the **compare** dropdown (right side) — it defaults to **Current (live)**.
3. The graph reloads immediately with change overlays.

Nodes that exist in compare but not in base are **added** (sky blue). Nodes that exist in both but whose dependency surface changed are **modified** (amber). Nodes that existed in base but are gone in compare are **deleted** (red, shown as ghost nodes).

### Clearing a comparison

Select **Current (live)** in both dropdowns, or navigate away and back. The compare toolbar resets to its default state and change overlays are removed.

### Persistence

Your comparison selection is saved to `localStorage` and restored on page reload. When the page reconnects to the server (including after a restart), Serpentine replays the `set_vcs_comparison` action automatically so change overlays reappear without any action on your part.

---

## Checkpoints

A **checkpoint** is a named snapshot of the current live graph state. It lets you mark a "before" point and then track changes as you edit files — without needing a git commit.

Click the **Checkpoint** button (flag icon) in the compare toolbar to mark the current graph as the checkpoint. From that point:

- The base dropdown shows **At checkpoint** by default.
- File changes appear as change overlays relative to the saved checkpoint, not relative to the last git commit.

The checkpoint is stored in server memory and cleared when the server restarts. It is separate from your git history.

---

## Filtering by change status

Use the `state:` selector in the search bar to filter the graph to only nodes with a specific change status:

| Selector | What it shows |
|----------|--------------|
| `state:modified` | Only nodes whose dependency surface changed |
| `state:added` | Only nodes that are new in the compare ref |
| `state:deleted` | Only nodes that were removed |

The autocomplete dropdown in the search bar suggests all three as you type `state`.

### Composing with graph operators

`state:` works with all selector operators:

```
# All modified nodes plus everything they depend on
+state:modified

# Full connected component of every added node
@state:added

# Modified nodes, 2 hops upstream
2+state:modified

# Everything modified or added
state:modified,state:added
```

See [Selectors](/docs/querying/selectors) for the full operator syntax.

---

## CLI usage

`serpentine analyze` and `serpentine catalog` both accept `--compare <ref>` to annotate CLI output with change status:

```bash
# See what changed on this branch vs main
serpentine analyze . --compare main --state modified --pretty

# List only added nodes in JSON
serpentine catalog . --compare main --state added --format json --pretty
```

The `--state` flag filters the output to nodes matching those statuses. Without `--state`, all nodes are returned but each carries a `change_status` field set to `"modified"`, `"added"`, `"deleted"`, or `null`.

---

## Configuration

Files excluded via `.serpentine.yml` are excluded from both live analysis and VCS snapshot analysis. This ensures that `exclude_patterns` and `exclude_dirs` behave identically whether Serpentine is reading the current working tree or a historical git snapshot — important for teams that use exclusions to keep sensitive files out of the graph.

```yaml
# .serpentine.yml
analysis:
  exclude_patterns:
    - "secrets/**"
    - "**/*.env"
  exclude_dirs:
    - private
```

See [Configuration](/docs/configuration) for the full schema.

---

## How it works

When you select a base ref, Serpentine:

1. Resolves the ref to a commit hash via pygit2 (no `git` subprocess).
2. Extracts the file tree at that commit into memory, filtering to extensions and directories from `.serpentine.yml`.
3. Runs the Rust analyzer over the snapshot to produce a graph JSON string.
4. Caches the result under `.serpentine_cache/refs/` keyed by `sha256(commit_hash + config_fingerprint)`. Subsequent comparisons to the same ref are instant.
5. Computes a structural diff between the base graph and the live graph, annotating each node with its `change_status`.
6. Broadcasts the annotated graph to all WebSocket clients.

The cache is invalidated automatically when the Rust analyzer binary is updated.
