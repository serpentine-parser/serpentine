---
title: "How We Built a Polyglot Code Graph Parser with Rust and tree-sitter"
description: "A walk through the architecture behind Serpentine's code reference graph: tree-sitter for parsing, a Rust event bus for analysis, and PyO3 to make it available from Python."
pubDate: 2026-05-31
---

Building a code analysis tool that works across Python, JavaScript, TypeScript, and Rust without writing four separate parsers requires picking the right foundation. We picked tree-sitter. This post covers what drove that decision, what the architecture looks like, and where the approach has limits.

## Why tree-sitter

The obvious alternative for Python analysis is Python's own `ast` module. It's built in, it's well-documented, and it produces a clean AST. The problem is that it only works for Python. The moment you want to handle a TypeScript file in the same repo, you're writing a second parser with a completely different AST shape and no shared infrastructure.

tree-sitter solves this by providing a uniform C API and a library of language grammars. Every language produces a concrete syntax tree with the same traversal interface — same `Node` type, same cursor API, same way to query by node kind. Once you've written tree-sitter traversal code for one language, adding a second language is mostly a matter of understanding its grammar and adding a new grammar dependency.

The other advantage is performance. tree-sitter is designed for editors that need to re-parse files on every keystroke. It's incremental by design and fast: parsing a large Python file takes a few milliseconds. For batch analysis of an entire project, that speed compounds quickly.

## The architecture: events, not AST walks

A naive approach to building a code graph from a tree-sitter parse is to walk the AST and mutate some global state as you go — push a function into a list when you see a function definition node, record a call when you see a call expression node. This works for small tools, but it couples the traversal logic tightly to the output format and makes it hard to compute multiple views of the same file.

Serpentine uses an event-driven model instead. The parser walks the tree and emits **typed events** rather than directly building a graph. The current event vocabulary looks like this:

- `EnterScope`: entering a function, class, or module boundary
- `ExitScope`: leaving a scope
- `DefineName`: a name is defined (a class, function, variable)
- `UseName`: a name is referenced
- `CallExpression`: a function is called, with arguments

These events are published through a **MessageBus** to a set of **Subscribers**. Each subscriber implements a single trait with three methods: `handle_event`, `finalize` (called after all events are emitted, returns a JSON result), and `name`. The graph builder is one subscriber. If we later want a "find all usages" index or a complexity scorer, those are additional subscribers — the traversal stays the same.

This separation means the Rust side has two clean jobs: emit events faithfully from the syntax tree, and route them to whoever cares.

<!-- SCREENSHOT: UI_Overview.png — The Serpentine interactive graph showing the full module structure of a Python project. Caption: "The output of the event pipeline: a navigable code reference graph." -->

## Crossing the language boundary with PyO3

Serpentine's server is Python — the file watcher, the WebSocket layer, the HTTP routes. The parser is Rust. PyO3 bridges them.

The boundary is a single class, `FileManager`, exposed as a Python-callable type via PyO3's `#[pyclass]` macro. Python code does:

```python
from serpentine import _analyzer
fm = _analyzer.FileManager()
fm.open_file(path, content)
result = fm.get_graph_json()
```

On the Rust side, `FileManager` owns the tree-sitter parser instances and the in-memory parse state. Calling `open_file` parses the content with the appropriate grammar (selected by file extension), runs the event pipeline, and stores the result. `get_graph_json` serializes the accumulated graph to a JSON string that Python unpacks.

The key constraint at this boundary: `FileManager` is not thread-safe. The Python layer holds a single instance and ensures it's only touched from one thread at a time. This is fine for the current architecture — analysis runs in a background thread, and the results are published via a callback once done.

## What "call graph" means here, and what it doesn't

Serpentine's graph is **static and name-based**. When we see `foo()` called inside `bar`, we record an edge from `bar` to a node named `foo`. We don't do type inference to determine which `foo` is being called in a case like:

```python
def dispatch(handler):
    handler.process(event)  # Which process()? We don't know.
```

A fully type-resolved call graph requires a type checker like pyright or mypy for Python, or the Rust compiler itself for Rust. That's a significantly harder problem and adds substantial latency. For Serpentine's use case (giving developers and AI agents a structural map of a codebase), name-based resolution is accurate enough for most real-world code and adds no external dependencies.

What we do handle well:

- **Inheritance** (`is-a` edges from class definitions with bases)
- **Containment** (`has-a` edges from class → method, module → class)
- **Calls** from function body to name references that resolve to defined nodes
- **Import relationships** between modules

What we deliberately skip:

- Dynamic dispatch through duck-typed interfaces
- Runtime-generated attributes (`setattr`, `__getattr__`)
- Decorator-transformed call targets

<!-- SCREENSHOT: Code_Details.png — The code detail panel showing a function node with its call edges listed alongside source code. Caption: "Call edges are derived statically from name references in function bodies." -->

## The graph schema

Every node in the output graph follows a consistent shape:

```json
{
  "id": "a3f7c2b18d4e9012",
  "name": "parse_file",
  "type": "function",
  "parent": "serpentine.parser",
  "children": [],
  "metadata": {
    "file": "src/parser.py",
    "line": 42
  }
}
```

Node IDs are stable hashes derived from the file path, node kind, and byte offsets in the source. This means the same function gets the same ID across re-parses as long as its location in the file doesn't change — useful for incremental updates and for the WebSocket diff protocol that pushes changes to the browser.

Edges are typed:

| Type | Meaning |
|------|---------|
| `calls` | Function A calls function B |
| `is-a` | Class A inherits from class B |
| `has-a` | Module/class A contains node B |
| `imports` | Module A imports from module B |

## Multi-language support

Adding a new language requires three things:

1. A tree-sitter grammar crate for that language (added to `Cargo.toml`)
2. A parser module that maps that grammar's node kinds to our event vocabulary
3. Registering the file extensions that trigger it

Python, JavaScript, TypeScript, and Rust are supported today. The parsers share the event infrastructure and the same graph schema — a Python function and a Rust function are both `type: "function"` nodes with the same edge types. This is what makes polyglot repos work: you can follow a `calls` edge from a Python module into a TypeScript module and it's the same traversal.

<!-- SCREENSHOT: Searchbar.png — The search bar showing cross-language results: Python and TypeScript nodes appearing together in a single search result list. Caption: "Language-agnostic node IDs mean Python and TypeScript live in the same graph." -->

## What's next

The current architecture has two areas we're actively working on:

**Incremental re-analysis.** tree-sitter supports incremental parsing — re-parsing only the changed regions of a file on edit. We use this for re-parsing individual files, but the graph builder currently recomputes from scratch. Diffing the graph at the event level (rather than rebuilding after every file change) is the next performance frontier.

**Richer edge resolution.** Right now, a `calls` edge to an unresolved name is dropped. A smarter resolver that uses import information to match unresolved names to their definitions across module boundaries would improve call graph completeness, especially for Python codebases that import heavily from third-party libraries.

If you want to dig into the implementation, the Rust source is in `rust/src/` in the [Serpentine repo](https://github.com/serpentine-parser/serpentine). The event definitions are in `events.rs`, the message bus in `message_bus.rs`, and the subscriber implementations in `subscribers/`.
