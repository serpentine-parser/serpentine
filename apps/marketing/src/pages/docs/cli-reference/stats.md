---
layout: ../../../layouts/Docs.astro
title: serpentine stats
description: Quick summary of project scale — node and edge counts by type and origin.
---

Print a quick summary of project scale: node and edge counts broken down by type and origin.

```
serpentine stats [PATH] [OPTIONS]
```

## Options

| Option | Description |
|--------|-------------|
| `--pretty` | Pretty-print JSON |
| `--include-standard` | Include stdlib nodes |
| `--include-third-party` | Include third-party nodes |

## Example output

```json
{
  "nodes": {
    "total": 312,
    "by_type": {
      "module": 18,
      "class": 44,
      "function": 241,
      "variable": 9
    },
    "by_origin": {
      "project": 312,
      "standard": 0,
      "third_party": 0
    }
  },
  "edges": {
    "total": 847,
    "by_type": {
      "calls": 501,
      "has-a": 89,
      "references": 193,
      "is-a": 64
    }
  },
  "modules": [
    "src.auth",
    "src.payments",
    "src.api",
    "src.core"
  ]
}
```

## When to use it

`stats` is the right first command when orienting to an unfamiliar codebase. The module list gives you the top-level structure at a glance; the node and edge counts give you a sense of scale before deciding how to slice the graph.

In an agent workflow, run `stats` before `catalog` or `analyze` to avoid over-fetching — if the project is small, you may be able to run `catalog` without a filter.
