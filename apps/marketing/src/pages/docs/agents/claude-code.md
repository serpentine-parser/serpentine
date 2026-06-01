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

If `.claude/` is present, it's detected automatically. You can also target Claude Code explicitly:

```bash
serpentine init --harness claude
```

## What gets installed

### `code-analysis` skill

Installed to `.claude/skills/code-analysis/SKILL.md`. This skill replaces `grep`, `find`, and file reads for code navigation. Claude uses three commands:

1. `serpentine stats .` — get the module list and scale
2. `serpentine catalog . --filter "*<topic>*"` — find nodes relevant to the task
3. `serpentine analyze . --select "+<relevant-area>+" --source` — get the subgraph and source

These three calls replace the typical exploratory loop of opening files and grepping, which saves tokens and produces more accurate plans.

### `CLAUDE.md` update

`serpentine init` appends a `## Serpentine` section to `CLAUDE.md` that tells Claude:

- To use `serpentine` commands instead of `grep`/`find`/`rg` for structural exploration
- When to use `catalog` vs `analyze`
- How to interpret edge types (`calls`, `has-a`, `references`, `is-a`)

These instructions persist across sessions, so you don't need to re-explain the workflow each time.

## Manual skill invocation

Even without automatic invocation, you can call the skill explicitly in any Claude Code session:

```
/code-analysis AuthService
/code-analysis what calls parse_request
```

## Updating

After upgrading Serpentine, re-run `serpentine init` to refresh the skill. Existing files are skipped by default; delete `.claude/skills/code-analysis/` first if you want to force an update.
