---
layout: ../../layouts/Docs.astro
title: Configuration
description: Customize Serpentine's analysis with .serpentine.yml.
---

Serpentine looks for `.serpentine.yml` or `serpentine.yml` in the project root. All settings are optional — Serpentine works with zero configuration.

## File location

```
your-project/
  .serpentine.yml   ← picked up automatically
  src/
  ...
```

Run `serpentine init` to generate a starter config with all defaults filled in. You can also create the file manually.

## Full example

```yaml
analysis:
  # File extensions to analyze (default: all supported)
  extensions: [".py", ".js", ".jsx", ".ts", ".tsx", ".rs"]

  # Directories to skip
  exclude_dirs:
    - node_modules
    - .venv
    - dist
    - build
    - .git
    - __pycache__

  # Glob patterns for files to skip
  exclude_patterns:
    - "**/*.test.ts"
    - "**/*.spec.py"
    - "**/generated/**"
    - "**/migrations/**"
```

## Options reference

### `analysis.extensions`

List of file extensions to include in analysis. Defaults to all supported extensions:
`.py`, `.js`, `.jsx`, `.ts`, `.tsx`, `.rs`.

Restrict to a subset if your project mixes languages but you only need one:

```yaml
analysis:
  extensions: [".py"]
```

### `analysis.exclude_dirs`

Directory names (not paths) to skip entirely. The default exclusion list covers common build and dependency directories.

Add project-specific directories:

```yaml
analysis:
  exclude_dirs:
    - node_modules
    - .venv
    - my_generated_code
```

### `analysis.exclude_patterns`

Glob patterns matched against file paths relative to the project root. Useful for excluding generated code, test fixtures, or vendored dependencies.

```yaml
analysis:
  exclude_patterns:
    - "**/*.test.ts"
    - "**/generated/**"
    - "vendor/**"
```

## Precedence

CLI flags take precedence over configuration file values. For example, `--include-standard` on the command line includes stdlib nodes even if they would be excluded by your config.
