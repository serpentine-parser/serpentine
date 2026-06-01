---
layout: ../../../layouts/Docs.astro
title: Installation
description: Get Serpentine installed in under a minute.
---

## PyPI (recommended)

The simplest way to install Serpentine is via pip:

```bash
pip install serpentine-parser
```

Or with [uv](https://docs.astral.sh/uv/), the fast Python package manager:

```bash
uv tool install serpentine-parser
```

Installing via `uv tool` makes the `serpentine` command available globally without activating a virtual environment.

## From source

> **Prerequisites**: Python 3.12+, [Rust toolchain](https://rustup.rs), Node.js 18+

```bash
git clone https://github.com/serpentine-parser/serpentine.git
cd serpentine

# 1. Build the frontend
cd frontend && npm install && npm run build && cd ..

# 2. Build the Rust extension and install the CLI
make install
```

This installs the `serpentine` CLI globally via `uv tool`, which you will need to [install separately](https://docs.astral.sh/uv/getting-started/installation/).

## Verify the installation

```bash
serpentine --version
```

You should see the current version. If the command is not found, check that the `uv` tools directory is on your `PATH`.

## Next steps

Once installed, head to [Quick Start](/docs/getting-started/quickstart) to analyze your first project.
