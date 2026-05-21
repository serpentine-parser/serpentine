---
layout: ../../../layouts/Docs.astro
title: serpentine analyze
description: Analyze a project and output the code reference graph as text or JSON.
---

Analyze a project and output the code reference graph. The default format is text (one edge per line as `caller --type--> callee`); use `--format json` for structured output suitable for programmatic use or agent consumption.

```
serpentine analyze [PATH] [OPTIONS]
```

## Options

| Option | Description |
|--------|-------------|
| `-o`, `--output PATH` | Write output to file instead of stdout |
| `--format TEXT` | Output format: `text` (default) or `json` |
| `--pretty` | Pretty-print JSON output |
| `--select TEXT` | Selector expression to filter nodes |
| `--exclude TEXT` | Exclusion pattern (same syntax as `--select`) |
| `--no-cfg` | Strip control-flow graph data from nodes |
| `--edges-only` | Output only the edges array (compact, JSON only) |
| `--source` | Include source code blocks in text output |
| `--include-assignments` | Include variable/assignment nodes in text output |
| `--include-standard` | Include stdlib nodes (default: off) |
| `--include-third-party` | Include third-party nodes (default: off) |
| `--state TEXT` | Filter by change state: `modified`, `added`, `deleted` |

## Examples

Dump the full graph as text:

```bash
serpentine analyze .
```

Get JSON output, pretty-printed:

```bash
serpentine analyze . --format json --pretty
```

Get just the edges for a subgraph (compact, agent-friendly):

```bash
serpentine analyze . --select "+src.auth.*+" --no-cfg --edges-only --pretty
```

Save output to a file:

```bash
serpentine analyze . --format json -o graph.json
```

Filter by recently modified files:

```bash
serpentine analyze . --state modified --format json --pretty
```

## Text output format

Each line in text output is one edge:

```
src.auth.views.login --calls--> src.auth.models.User.get
src.auth.views.login --has-a--> src.auth.forms.LoginForm
src.auth.models.User --is-a--> django.db.models.Model
```

This format is easy to scan and works well piped into other tools.

## Edge types

| Type | Meaning |
|------|---------|
| `calls` | One entity calls another as a function |
| `has-a` | A variable holds an instance of a class (constructor call) |
| `references` | One entity names another without calling it |
| `is-a` | Inheritance — one class extends another |

## Selectors

Use `--select` to focus on a subgraph. See the [Selectors reference](/docs/querying/selectors) for the full syntax, including upstream (`+pattern`), downstream (`pattern+`), and bounded-hop traversal.
