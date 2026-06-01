---
layout: ../../../layouts/Docs.astro
title: Cursor
description: Install the Serpentine rule and wire structural orientation into every Cursor session.
---

Serpentine integrates with Cursor via its rules system. Running `serpentine init` installs a rule file that instructs Cursor to use Serpentine's CLI instead of reading files or grepping — running `stats`, `catalog`, and `analyze` to get source code and structural context in a single token-efficient pass.

## Setup

In any project you want to instrument:

```bash
serpentine init
```

If `.cursor/` is present, it's detected automatically. You can also target Cursor explicitly:

```bash
serpentine init --harness cursor
```

## What gets installed

### `.cursor/rules/serpentine.mdc`

A Cursor rule file with `alwaysApply: true`, which means Cursor applies it automatically on every task — no slash command needed. The rule instructs Cursor to use three commands:

1. `serpentine stats .` — get the module list and scale
2. `serpentine catalog . --filter "*<topic>*"` — find nodes relevant to the task
3. `serpentine analyze . --select "+<relevant-area>+" --source` — get the subgraph and source

These replace the typical exploratory loop of opening files and grepping.

## Updating

After upgrading Serpentine, re-run `serpentine init` to refresh the rule. Existing files are skipped by default; delete `.cursor/rules/serpentine.mdc` first if you want to force an update.
