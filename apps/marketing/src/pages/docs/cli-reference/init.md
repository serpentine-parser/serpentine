---
layout: ../../../layouts/Docs.astro
title: serpentine init
description: Wire Serpentine into an existing project in one command.
---

Set up Serpentine in an existing project.

```
serpentine init [PATH]

Arguments:
  PATH    Project directory (default: current directory)
```

## What it does

Running `serpentine init` creates or updates four things:

1. **`.serpentine.yml`** — Default configuration file. Edit this to customize which extensions and directories to include or exclude.

2. **`.gitignore`** — Appends `.serpentine` to prevent the cache directory from being committed.

3. **`.claude/skills/`** — Installs two Claude Code skills:
   - `serpentine-orient` — structural orientation skill, runs `stats` and `catalog` at the start of tasks
   - `serpentine-check` — blast-radius skill, runs `analyze --select` before edits

4. **`CLAUDE.md`** — Appends a `## Serpentine` section with navigation instructions so Claude Code knows to use these skills.

Already-existing files are skipped rather than overwritten, so it's safe to re-run.

## Example output

```
$ serpentine init

✓ Created .serpentine.yml
✓ Updated .gitignore
✓ Installed .claude/skills/serpentine-orient
✓ Installed .claude/skills/serpentine-check
✓ Updated CLAUDE.md

Done. Your agent orients structurally by default.
```

## Agent compatibility

The installed skills work with any agent that reads `CLAUDE.md` or supports skills files:

- Claude Code
- Cursor
- GitHub Copilot
- Codex (OpenAI)
- OpenCode

For agents that don't support `CLAUDE.md`, the structured JSON output from `analyze` and `catalog` can still be fed directly into the agent's context via tool calls or file reads.
