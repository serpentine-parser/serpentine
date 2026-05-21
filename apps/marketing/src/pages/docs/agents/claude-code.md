---
layout: ../../../layouts/Docs.astro
title: Claude Code
description: Install the Serpentine skill and wire structural orientation into every Claude Code session.
---

Claude Code is the primary agent integration for Serpentine. Running `serpentine init` installs a `code-analysis` skill that instructs Claude to use Serpentine's CLI instead of reading files or grepping — running `stats`, `catalog`, and `analyze` to get source code and structural context in a single token-efficient pass.

## Setup

In any project you want to instrument:

```bash
serpentine init
```

This installs two skill files to `.claude/skills/` and appends a `## Serpentine` section to `CLAUDE.md`.

## What gets installed

### `serpentine-orient` skill

Runs at the start of tasks. Claude calls:

1. `serpentine stats .` — get the module list and scale
2. `serpentine catalog . --filter "*<topic>*"` — find nodes relevant to the task
3. `serpentine analyze . --select "+<relevant-area>+"` — get the subgraph

These three calls replace the typical exploratory loop of opening files and grepping, which saves tokens and produces more accurate plans.

### `serpentine-check` skill

Runs before edits to assess blast radius. Claude calls:

```bash
serpentine analyze . --select "<target>+" --no-cfg --edges-only --pretty
```

This tells Claude exactly what depends on the thing it's about to change, so it doesn't discover new dependents halfway through an implementation.

## The CLAUDE.md update

`serpentine init` appends navigation instructions to `CLAUDE.md` that tell Claude:

- To use `serpentine` commands instead of `grep`/`find`/`rg` for structural exploration
- When to invoke `serpentine-orient` vs `serpentine-check`
- How to interpret edge types (`calls`, `has-a`, `references`, `is-a`)

These instructions persist across sessions, so you don't need to re-explain the workflow each time.

## Manual skill invocation

Even without automatic invocation, you can call the skills explicitly in any Claude Code session:

```
/serpentine-orient
/serpentine-check src.auth.models.User
```

## Updating

After upgrading Serpentine, re-run `serpentine init` to refresh the skills. Existing files are skipped by default; delete `.claude/skills/serpentine-orient` and `.claude/skills/serpentine-check` first if you want to force an update.
