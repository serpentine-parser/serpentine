---
layout: ../../../layouts/Docs.astro
title: Using with AI Agents
description: How Serpentine's structured CLI output gives AI coding agents precise structural context before they read a single file.
---

Serpentine's CLI is designed to be consumed by AI coding agents — Claude Code, Cursor, GitHub Copilot, Codex, OpenCode, and others. The structured JSON output, selector syntax, and `--edges-only` flag exist specifically to give agents precise, low-noise subgraphs without requiring file reads.

## Wiring it in

Run once inside any project:

```bash
serpentine init
```

`init` always creates `.serpentine.yml` (project config) and adds `.serpentine` to `.gitignore`. It then auto-detects which harnesses are in use and installs the appropriate configuration for each:

| Harness | What gets installed |
| ------- | ------------------- |
| `claude` | `.claude/skills/code-analysis/SKILL.md` + appends serpentine section to `CLAUDE.md` |
| `cursor` | `.cursor/rules/serpentine.mdc` |
| `copilot` | appends serpentine section to `.github/copilot-instructions.md` |
| `codex` | appends serpentine section to `AGENTS.md` |
| `opencode` | appends serpentine section to `AGENTS.md` |

Append-mode harnesses (`claude`, `copilot`, `codex`, `opencode`) add a `## Serpentine` section to existing files and skip if the section is already present. `cursor` skips if `.cursor/rules/serpentine.mdc` already exists.

Target a specific harness with `--harness`:

```bash
serpentine init --harness cursor
serpentine init --harness claude --harness copilot
```

## The problem it solves

When an agent starts a non-trivial task, it faces a cold-start problem: it doesn't know which files are relevant, how components relate, or what the blast radius of a change might be. The usual approach — grep, glob, read files one at a time — is expensive in tokens and often incomplete.

Serpentine collapses that exploration into 2–3 CLI calls. The agent gets a structural picture of the relevant code _before_ it starts reading files or writing a plan. In practice this means:

- Plans are scoped to the right files and boundaries from the start
- Less back-and-forth between reading files to trace call chains
- Fewer surprises mid-implementation when a dependency was missed

## Recommended workflow

```bash
# 1. Get the lay of the land — module names, rough scale
serpentine stats .

# 2. Find relevant node IDs by name
serpentine catalog . --filter "*auth*"

# 3. Get the subgraph with source code for the relevant area
serpentine analyze . --select "+src.auth.*+" --source

# 3b. Or edges-only for a compact cross-boundary picture
serpentine analyze . --select "+src.auth.*+" --edges-only --pretty
```

Use `--source` to include source code blocks inline — useful when the agent needs to read the implementation without making separate file reads. Use `--edges-only` when you only need the structural picture; it produces a much smaller output.

## Scenarios where this pays off

### Before refactoring a module

Use `--select "module+"` to map everything that depends on the module before writing a plan. This ensures the plan accounts for every callsite and doesn't discover new dependents halfway through.

### Before adding a feature that spans modules

Use `--select "+moduleA.*,+moduleB.*"` to get the combined subgraph of two areas and understand exactly where they intersect. The boundaries of the work become clear before any code is written.

### Orienting to an unfamiliar codebase

`serpentine stats` gives you the top-level module list and scale. `serpentine catalog .` gives a flat index of every class and function. Together these give structural orientation without reading a single file.

### Before deleting code

Use `--select "*.TargetFunction+"` to get all downstream dependents. An empty result confirms the code is truly unused.

### Tracing a call chain

Use `--select "+*.entrypoint+3"` to get 3 hops downstream from an entry point and understand the execution path before adding instrumentation or fixing a bug.

## Token efficiency

`--edges-only` outputs only the edges array rather than the full node tree, which is typically 10–50× smaller while containing all the structural information an agent needs to trace call chains.

For very large codebases, chain `--select` with `--exclude "*test*"` to remove noise before passing the result to an agent.
