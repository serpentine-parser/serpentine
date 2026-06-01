---
layout: ../../../layouts/Docs.astro
title: GitHub Copilot
description: Add Serpentine instructions to your Copilot project context.
---

Serpentine integrates with GitHub Copilot via the `copilot-instructions.md` file. Running `serpentine init` appends a `## Serpentine` section that instructs Copilot to use Serpentine's CLI for structural orientation instead of reading files or grepping.

## Setup

In any project you want to instrument:

```bash
serpentine init
```

Copilot is detected automatically if `.github/copilot-instructions.md` already exists or if a `.vscode/` directory is present. You can also target it explicitly:

```bash
serpentine init --harness copilot
```

## What gets installed

### `.github/copilot-instructions.md`

If the file already exists, `serpentine init` appends a `## Serpentine` section to it. If it doesn't exist, the file is created with just the Serpentine section. The section instructs Copilot to use three commands:

1. `serpentine stats .` — get the module list and scale
2. `serpentine catalog . --filter "*<topic>*"` — find nodes relevant to the task
3. `serpentine analyze . --select "*.Symbol" --source` — read source and edges

GitHub Copilot reads `copilot-instructions.md` automatically for every session in the project, so no per-session setup is needed.

## Updating

After upgrading Serpentine, remove the existing `## Serpentine` section from `.github/copilot-instructions.md` and re-run `serpentine init` to write the updated instructions.
