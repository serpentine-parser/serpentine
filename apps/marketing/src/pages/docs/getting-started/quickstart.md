---
layout: ../../../layouts/Docs.astro
title: Quick Start
description: Analyze your first project in three commands.
---

## Start the visualization server

Navigate to any Python, JavaScript/TypeScript, Rust, or Terraform project and run:

```bash
serpentine serve
```

Serpentine will analyze the project, start a local server at `http://127.0.0.1:8765`, and open the interactive graph in your browser. File changes are detected automatically and the graph updates in real time via WebSocket. See the [UI Overview](/docs/ui/overview) for a tour of the interface.

## The agent-oriented workflow

If you're using Serpentine to give an AI agent structural context, the workflow is three commands:

```bash
# 1. Get the lay of the land — module names, rough scale
serpentine stats .

# 2. Find relevant node IDs by name
serpentine catalog . --filter "*auth*"

# 3. Get the subgraph and source for the relevant area
serpentine analyze . --select "*auth*.*" --source
```

Read the source and edges to understand what connects to what, then read only the files that are actually relevant. For a walkthrough of how this works in practice, see [Why I Give Claude a Dependency Graph Instead of File Dumps](/blog/why-i-give-claude-a-dependency-graph).

## Wire it in

Run this once inside any project:

```bash
serpentine init
```

This creates `.serpentine.yml`, updates `.gitignore`, and installs configuration for whichever AI coding tools are detected. For Claude Code: installs the `code-analysis` skill to `.claude/skills/code-analysis/` and appends to `CLAUDE.md`. After that, Claude orients structurally — running `stats`, `catalog`, and `analyze` before reading files or writing a plan — on every task.

Also supports Cursor (`.cursor/rules/serpentine.mdc`), GitHub Copilot (`.github/copilot-instructions.md`), Codex, and OpenCode (`AGENTS.md`). Use `--harness` to target one explicitly:

```bash
serpentine init --harness cursor
```

## What Serpentine tracks

Serpentine builds a **code reference graph** — every place one definition statically mentions another, resolved through the language's scoping rules to the specific definition it refers to.

| Reference type | Example                    | Edge                              |
| -------------- | -------------------------- | --------------------------------- |
| Function call  | `result = parse(data)`     | `result --calls--> parse`         |
| Constructor    | `loader = CSVLoader(path)` | `loader --has-a--> CSVLoader`     |
| Name reference | `default = MISSING`        | `default --references--> MISSING` |
| Inheritance    | `class Stats(Base)`        | `Stats --is-a--> Base`            |

All four edge types resolve through imports — so a reference to an imported name traces all the way back to its definition.

## Next steps

- [CLI Reference](/docs/cli-reference/serve) — full option docs for every command
- [Selectors](/docs/querying/selectors) — slice the graph to exactly the nodes you need
- [Using with AI Agents](/docs/agents/overview) — recommended workflows for coding agents
