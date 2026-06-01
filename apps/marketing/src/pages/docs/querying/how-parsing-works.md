---
layout: ../../../layouts/Docs.astro
title: How Parsing Works
description: What Serpentine builds from your code, how edges are determined, and what makes the graph useful.
---

Serpentine's goal is to answer one question: **when entity A mentions entity B by name, which specific definition does that reference resolve to?** The result is a graph where every node is a named entity in your code and every edge is a statically-resolved reference — not a file import, not a string match, but a traced pointer from one definition to another.

## The graph structure

Every node represents a named entity — a module, class, function, method, or variable assignment. Nodes are identified by a dotted path that reflects their location in the codebase (`src.auth.models.User.get`). Each node has a type:

| Type | What it represents |
|---|---|
| `module` | A source file or package |
| `class` | A class definition |
| `function` | A function, method, or lambda |
| `assignment` | A variable or constant assignment |
| `interface` | A TypeScript interface |

Edges are directional and typed. There are five kinds:

| Edge type | What it represents | Example |
|---|---|---|
| `calls` | A function or method invocation | `parse(data)` → `parse` |
| `has-a` | A constructor assignment or attribute-form decorator | `loader = CSVLoader(path)` → `CSVLoader` |
| `is-a` | Class inheritance | `class Stats(Base)` → `Base` |
| `references` | A name used as a value, or a plain decorator | `default = MISSING` → `MISSING` |
| `imports` | A module-level import statement | `from auth import User` → `auth.User` |

## Variable-type inference

One of Serpentine's more useful properties is that it resolves method calls through local variables **without needing type annotations**. Consider this Python:

```python
def main():
    car = Car(engine)
    car.drive()
```

Most tools would need a type annotation (`car: Car`) to know that `car.drive()` resolves to `Car.drive`. Serpentine infers it from the assignment:

1. `car = Car(engine)` → ASSIGNED pass records `main.car --has-a--> Car`
2. `car.drive()` → CALLS pass sees `car.drive`, resolves `car` via LEGB to `main.car`, follows the `has-a` edge to find that `main.car` is a `Car`, then looks up `drive` on `Car`
3. Result: `main --calls--> Car.drive`

This works for chained attribute access too. If `self.engine = Engine()` is set in `__init__`, then `self.engine.start()` elsewhere in the class resolves to `Engine.start`. The same inference applies to factory functions: if `build_car()` has a single return type, variables assigned from it are typed accordingly.

## How the graph is built

The parser is built on [tree-sitter](https://tree-sitter.github.io/tree-sitter/) and processes each file through a subscriber pipeline. As the AST is walked, events are emitted — `EnterScope`, `ExitScope`, `DefineName`, `UseName`, `ImportStatement`, and so on — and subscribers collect them into structured data. A `GraphBuilder` then runs a sequence of passes over that data to produce the final graph.

The passes, in order:

1. **Scope tree** — builds the module/class/function containment hierarchy
2. **Definitions** — registers variable and assignment nodes
3. **Re-export map** — scans `__init__.py`, `index.ts`, and `mod.rs`/`lib.rs` files to transparently resolve re-exported names back to their definition site
4. **Import bindings** — builds the name resolution table for the "G" (global/module) level; follows import chains up to depth 20
5. **ASSIGNED pass** — creates `has-a` edges for constructor assignments (`x = Foo()`)
6. **RETURNS pass** — infers function return types from `return` statements
7. **RETYPE pass** — upgrades factory-function `has-a` edges to point at the returned class instead of the function
8. **CONSTRUCTOR-ARG pass** — propagates argument types at call sites into `__init__` parameter nodes
9. **PARAM-TYPE pass** — propagates `__init__` parameter types through `self.x = param` assignments
10. **CALLS pass** — resolves all remaining function and method call targets

## Name resolution

Name resolution uses **LEGB rules** (Local → Enclosing → Global → Built-in), the same scoping model Python uses. Given a name and the scope it appears in, the resolver walks up the scope hierarchy checking:

1. Local definitions in the current scope
2. Enclosing scope definitions
3. Module-level import bindings (after following the import chain to the original definition)
4. Language builtins

Submodule names are intentionally excluded from hierarchy lookup — a submodule `foo.bar` existing does not mean `bar` is in `foo`'s namespace. Only an explicit import binding makes it so.

For dotted names like `self.method()` or `obj.method()`, the resolver:
- Identifies the enclosing class for `self` access
- For `self.attr.method()`, follows `has-a` edges to find the attribute's type and then looks up the method on that type
- For `obj.method()`, resolves `obj` via LEGB and then looks up the method on the resolved type

## Import resolution

The import resolver handles all common forms:

| Import form | How it's resolved |
|---|---|
| `import foo` | Binding: `current_module.foo` → `foo` |
| `import foo.bar` | Binding: `current_module.foo` → `foo` (top-level) |
| `from foo import bar` | Binding: `current_module.bar` → `foo.bar` |
| `from foo import bar as b` | Binding: `current_module.b` → `foo.bar` |

Re-exports are transparent. If `server/__init__.py` does `from serpentine.server.app import create_app`, an import of `from serpentine.server import create_app` creates an edge pointing at `serpentine.server.app.create_app` — the actual definition — not at the `__init__.py` re-export.

**Current limitations on imports:**
- Relative imports (`from . import foo`, `from ..utils import x`) are not yet resolved and produce no edges
- `from foo import *` creates an edge to the module but not to individual names

## Decorators

Decorators are handled as a special case of name reference:

- **Attribute-form decorator** (`@router.get("/")`, `@app.route(...)`) where the object (`router`, `app`) resolves to a local definition → a `has-a` edge from the object to the decorated function. This captures registration patterns like Flask routes or FastAPI endpoints.
- **Plain decorator** (`@dataclass`, `@staticmethod`) → a `references` edge from the decorated function to the decorator.

## What Serpentine does not track

Serpentine is a **static** analyzer. It reads source files — it does not run your code. This means:

- **Dynamic references are not tracked.** `getattr(obj, name)`, `importlib.import_module(path)`, and similar runtime lookups produce no edges.
- **Relative imports are not yet resolved.** If your codebase relies heavily on `from . import x`, those edges will be absent.
- **`from foo import *` is partially handled.** An `imports` edge to the module is created, but individual name references through the star import won't resolve to their definitions.
- **Generated code is not analyzed** unless the generated files are present on disk at analysis time.

If a reference you expect to see is missing, it is most likely one of these cases.

## Supported languages

| Language | File extensions |
|---|---|
| Python | `.py` |
| JavaScript | `.js`, `.jsx` |
| TypeScript | `.ts`, `.tsx` |
| Rust | `.rs` |

For a deeper look at the architecture decisions behind the parser — why tree-sitter, how the subscriber pipeline came together, and the tradeoffs of doing this in Rust — see [How We Built a Polyglot Code Graph with Rust and Tree-Sitter](/blog/how-we-built-a-polyglot-code-graph-with-rust-and-tree-sitter).

## Reporting a bug

If you see a missing edge, a phantom edge, or a node that should exist but doesn't, please [open an issue on GitHub](https://github.com/serpentine-parser/serpentine/issues). The most useful reports include:

- The language and a minimal source snippet that reproduces the problem
- The `serpentine analyze` command you ran and the output you got
- What you expected to see instead
