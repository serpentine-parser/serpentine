# Contributing to Serpentine

Thanks for your interest in contributing. This document covers how to get the project running locally and how to submit changes.

## Prerequisites

- Python 3.12+
- [Rust toolchain](https://rustup.rs) (stable)
- Node.js 18+
- [uv](https://docs.astral.sh/uv/) (Python package manager)

## Local Setup

```bash
git clone https://github.com/serpentine-parser/serpentine.git
cd serpentine

# Install Python dev dependencies
uv sync

# Build the Rust extension
uv run maturin develop

# Build the frontend
cd frontend && npm install && npm run build && cd ..

# Verify the CLI works
uv run serpentine stats .
```

### Development Workflow

[Tilt](https://tilt.dev) is the easiest way to develop — it watches for changes and rebuilds automatically:

```bash
tilt up
```

Or rebuild manually after changes:

| Change location    | Command                       |
|--------------------|-------------------------------|
| `rust/src/`        | `uv run maturin develop`      |
| `frontend/src/`    | `cd frontend && npm run build` |
| `src/serpentine/`  | No rebuild needed (Python)    |

## Project Structure

```
serpentine/
├── rust/src/           # Rust analyzer (tree-sitter parsers, graph builder)
├── src/serpentine/     # Python package (CLI, server, state, selector)
├── frontend/src/       # React frontend (Vite + TypeScript)
└── pyproject.toml
```

See [CLAUDE.md](CLAUDE.md) for a detailed architecture overview.

## Making Changes

### Rust (parser/analyzer)

The Rust analyzer uses a message bus pattern. Each parser emits events (`ImportStatement`, `UseName`, `CallExpression`) that subscribers process in parallel.

- Parsers live in `rust/src/python/`, `rust/src/javascript/`, `rust/src/rust_lang/`
- Subscribers live in `rust/src/subscribers/`
- Graph building logic is in `rust/src/graph/`

After editing Rust code, run `uv run maturin develop` to recompile.

### Python (server/CLI)

The Python layer is a Starlette web server with a WebSocket broadcaster. `GraphStateManager` in `state.py` is the single source of truth — all graph mutations go through it.

- CLI commands are in `src/serpentine/cli.py`
- Server routes are in `src/serpentine/server/routes.py`
- Selector logic is in `src/serpentine/selector.py`

### Frontend

The frontend is React + TypeScript with Zustand for state. The graph is rendered with D3 and ELK for layout.

- All UI state lives in `frontend/src/store/`
- Graph rendering is in `frontend/src/ui/components/Graph.tsx`
- Layout logic is in `frontend/src/domains/graph/lib/`

After editing frontend code, run `cd frontend && npm run build` to rebuild static assets.

## Submitting a Pull Request

1. Fork the repository and create a branch from `main`
2. Make your changes
3. Verify the CLI works end-to-end: `uv run serpentine serve <some-project>`
4. Open a pull request with a clear description of what changed and why

For larger changes, open an issue first to discuss the approach.

## Code Style

- **Python**: Follow existing patterns; full type annotations on all functions
- **Rust**: Run `cargo clippy` before submitting; fix any warnings
- **TypeScript**: Run `npx tsc --noEmit` in `frontend/` to check for type errors

## Reporting Bugs

Please use the [issue tracker](https://github.com/serpentine-parser/serpentine/issues). Include:

- Serpentine version (`serpentine --version`)
- OS and Python version
- The project you were analyzing (or a minimal reproduction)
- Full error output
