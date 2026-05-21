---
layout: ../../../layouts/Docs.astro
title: Quick Start
description: Analyze your first project in three commands.
---

## Start the visualization server

Navigate to any Python, JavaScript/TypeScript, or Rust project and run:

```bash
serpentine serve
```

Serpentine will analyze the project, start a local server at `http://127.0.0.1:8765`, and open the interactive graph in your browser. File changes are detected automatically and the graph updates in real time via WebSocket.

## The agent-oriented workflow

If you're using Serpentine to give an AI agent structural context, the workflow is three commands:

```bash
# 1. Get the lay of the land — module names, rough scale
serpentine stats .

# 2. Find relevant node IDs by name
serpentine catalog . --filter "*auth*" --format json --pretty

# 3. Get the subgraph for the relevant area
serpentine analyze . --select "+src.auth.*+" --no-cfg --edges-only --pretty
```

Read the edges to understand what connects to what, then read only the files that are actually relevant.

## Wire it into Claude Code

Run this once inside any project:

```bash
serpentine init
```

This installs the `code-analysis` skill to `.claude/skills/`, updates `CLAUDE.md`, and creates `.serpentine.yml`. After that, Claude Code orients structurally — running `stats`, `catalog`, and `analyze` before reading files or writing a plan — on every task.

## What Serpentine tracks

Serpentine builds a **code reference graph** — every place one named entity statically mentions another, resolved through the language's scoping rules to the specific definition it refers to.

| Reference type | Example                    | Edge                              |
|----------------|----------------------------|-----------------------------------|
| Function call  | `result = parse(data)`     | `result --calls--> parse`         |
| Constructor    | `loader = CSVLoader(path)` | `loader --has-a--> CSVLoader`     |
| Name reference | `default = MISSING`        | `default --references--> MISSING` |
| Inheritance    | `class Stats(Base)`        | `Stats --is-a--> Base`            |

All four edge types resolve through imports — so a reference to an imported name traces all the way back to its definition.

## Next steps

- [CLI Reference](/docs/cli-reference/serve) — full option docs for every command
- [Selectors](/docs/querying/selectors) — slice the graph to exactly the nodes you need
- [Using with AI Agents](/docs/agents/overview) — recommended workflows for coding agents
