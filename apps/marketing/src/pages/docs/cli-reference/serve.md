---
layout: ../../../layouts/Docs.astro
title: serpentine serve
description: Start the interactive visualization server with live file watching.
---

Start the visualization server with live file watching.

```
serpentine serve [PATH] [OPTIONS]

Arguments:
  PATH    Directory to analyze (default: current directory)
```

## Options

| Option | Default | Description |
|--------|---------|-------------|
| `-p`, `--port INT` | `8765` | Port to run the server on |
| `-h`, `--host TEXT` | `127.0.0.1` | Host to bind to |
| `--no-browser` | — | Don't open browser automatically |
| `--no-watch` | — | Disable file watching (static analysis only) |

## Examples

Analyze the current directory and open the browser:

```bash
serpentine serve
```

Analyze a specific project on a custom port:

```bash
serpentine serve /path/to/project --port 3000
```

Run without opening the browser (useful in remote/CI environments):

```bash
serpentine serve --no-browser
```

Static analysis only — no file watcher:

```bash
serpentine serve --no-watch
```

## How it works

`serpentine serve` builds the full code reference graph for the target directory, then starts a Starlette web server that serves the interactive visualization. When file watching is enabled (default), changes to source files are detected and the graph is updated via WebSocket push — no manual refresh needed.

The server binds to `127.0.0.1` by default, which means it's only accessible from the local machine. Set `--host 0.0.0.0` to expose it on the network.
