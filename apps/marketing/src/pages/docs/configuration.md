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

This is the default configuration — equivalent to running with no config file at all. Only include what you want to change.

```yaml
analysis:
  # File extensions to analyze
  extensions: [".py", ".js", ".jsx", ".ts", ".tsx", ".rs"]

  # Directories to skip (matched by name, not path)
  exclude_dirs:
    - __pycache__
    - .git
    - .venv
    - venv
    - node_modules
    - .mypy_cache
    - .pytest_cache
    - .tox
    - dist
    - build
    - static
    - .next
    - .nuxt
    - coverage
    - .egg-info
    - target

  # Glob patterns for files to skip (default: none)
  exclude_patterns: []
```

## Options reference

### `analysis.extensions`

List of file extensions to include in analysis. Default: all supported extensions.

```yaml
# Default
extensions: [".py", ".js", ".jsx", ".ts", ".tsx", ".rs"]
```

Restrict to a subset if your project mixes languages but you only need one:

```yaml
analysis:
  extensions: [".py"]
```

### `analysis.exclude_dirs`

Directory names (not paths) to skip entirely. Overrides the default list entirely when set, so include any defaults you still want.

Default:

```yaml
exclude_dirs:
  - __pycache__
  - .git
  - .venv
  - venv
  - node_modules
  - .mypy_cache
  - .pytest_cache
  - .tox
  - dist
  - build
  - static
  - .next
  - .nuxt
  - coverage
  - .egg-info
  - target
```

To add project-specific directories, include the defaults plus your additions:

```yaml
analysis:
  exclude_dirs:
    - __pycache__
    - .git
    - .venv
    - venv
    - node_modules
    - .mypy_cache
    - .pytest_cache
    - .tox
    - dist
    - build
    - static
    - .next
    - .nuxt
    - coverage
    - .egg-info
    - target
    - my_generated_code
```

### `analysis.exclude_patterns`

Glob patterns matched against file paths relative to the project root. Default: none.

```yaml
# Default
exclude_patterns: []
```

Useful for excluding generated code, test fixtures, or vendored dependencies:

```yaml
analysis:
  exclude_patterns:
    - "**/*.test.ts"
    - "**/generated/**"
    - "vendor/**"
```

## Precedence

CLI flags take precedence over configuration file values. For example, `--include-standard` on the command line includes stdlib nodes even if they would be excluded by your config.
