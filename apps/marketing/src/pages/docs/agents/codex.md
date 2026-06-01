---
layout: ../../../layouts/Docs.astro
title: Codex
description: Add Serpentine instructions to your Codex project context via AGENTS.md.
---

Serpentine integrates with OpenAI Codex via `AGENTS.md`. Running `serpentine init` appends a `## Serpentine` section that instructs Codex to use Serpentine's CLI for structural orientation instead of reading files or grepping.

## Setup

In any project you want to instrument:

```bash
serpentine init
```

Codex is detected automatically if an `AGENTS.md` file is already present. You can also target it explicitly:

```bash
serpentine init --harness codex
```

## What gets installed

### `AGENTS.md`

If `AGENTS.md` already exists, `serpentine init` appends a `## Serpentine` section to it. If it doesn't exist, the file is created with just the Serpentine section. The section instructs Codex to use three commands:

1. `serpentine stats .` — get the module list and scale
2. `serpentine catalog . --filter "*<topic>*"` — find nodes relevant to the task
3. `serpentine analyze . --select "*.Symbol" --source` — read source and edges

Codex reads `AGENTS.md` automatically for project context, so no per-session configuration is needed.

## Updating

After upgrading Serpentine, remove the existing `## Serpentine` section from `AGENTS.md` and re-run `serpentine init` to write the updated instructions.
