---
layout: ../../../layouts/Docs.astro
title: serpentine catalog
description: Flat list of every named entity in the codebase — modules, classes, functions, and variables.
---

Output a flat list of all nodes. The default format is a file-grouped text hierarchy; use `--format json` to see node IDs for use in selector expressions.

```
serpentine catalog [PATH] [OPTIONS]
```

## Options

| Option | Description |
|--------|-------------|
| `--filter TEXT` | Glob pattern matched against node ID and name (repeatable; multiple patterns = union) |
| `--format TEXT` | Output format: `text` (default) or `json` |
| `--include-assignments` | Include variable/assignment nodes (default: excluded) |
| `-o`, `--output PATH` | Write output to file instead of stdout |
| `--pretty` | Pretty-print JSON output |
| `--include-standard` | Include stdlib nodes |
| `--include-third-party` | Include third-party nodes |
| `--state TEXT` | Filter by change state: `modified`, `added`, `deleted` |
| `--compare TEXT` | VCS ref to compare against. Annotates output with `change_status`. Requires `serpentine-parser[git]`. |

## Finding node IDs

Every node has a dotted ID that reflects its location in the codebase. Use `catalog` with `--format json` to discover them:

```bash
serpentine catalog . --format json --pretty
```

This outputs a flat list of every module, class, and function with its ID:

```json
{
  "nodes": [
    { "id": "src.auth.models.User", "name": "User", "type": "class" },
    { "id": "src.auth.views.login", "name": "login", "type": "function" },
    { "id": "src.payments.stripe.charge", "name": "charge", "type": "function" }
  ]
}
```

## Filtering with globs

Narrow results with `--filter` (glob matched against both ID and name). The `*` wildcard matches across dots, so it crosses module boundaries.

```bash
# Everything related to auth
serpentine catalog . --filter "*auth*"

# Find a class by name across all modules
serpentine catalog . --filter "*User*" --format json --pretty

# Combine multiple filters (union)
serpentine catalog . --filter "*auth*" --filter "*payment*"
```

## Text output format

The text output groups nodes by file, showing the hierarchy:

```
src/auth/models.py
  class User
    def get_full_name
    def save
  class Permission

src/auth/views.py
  def login
  def logout
```

Use this to get a quick structural overview of any module or the whole project.
