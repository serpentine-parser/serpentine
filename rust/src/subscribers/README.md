# Subscribers

Subscribers are the core analysis components of serpentine-rust. They implement a publish-subscribe pattern: the tree-sitter parser walks Python source files and emits semantic `Event`s onto a `MessageBus`, which fans them out to all registered subscribers in parallel (each subscriber runs on its own thread via crossbeam channels). Each subscriber accumulates state as events arrive, then produces a JSON result when `finalize()` is called.

Every subscriber has a corresponding `SubscriberFactory` that the message bus uses to create fresh instances per analysis run.

---

## Event Types

Before describing the subscribers, here are the events they consume (defined in `rust/src/events.rs`):

| Event | Description |
|---|---|
| `DefineName` | A name is being defined (variable, function, class, import). Carries `node_id`, `name`, `qualname`, `node_type`, file/line/column. |
| `EnterScope` | Entering a function or class scope. Carries `scope_type`, `name`, `qualname`, `parameters` (for functions), file/line. |
| `ExitScope` | Leaving a function or class scope. |
| `UseName` | A name is being referenced/read. Carries `name`, file/line/column. |
| `CallExpression` | A function or method call. Carries `callee` (e.g. `"math.sqrt"`), `arguments` (text of each argument expression), file/line/column. |
| `ImportStatement` | An import statement. Carries `module` (source module) and `names` (imported names). |
| `Assignment` | An assignment `target = value`. Carries `target`, `target_qualname`, `value` (expression text), `value_type`. |
| `ControlBlock` | Start of an `if`/`for`/`while`/`try`/`with` block. Carries `block_type` and `condition` text. |
| `EndControlBlock` | End of a control block. |
| `ElseBlock` | An `else` or `elif` clause within a control block. |
| `Return` | A return statement. Carries `value` (expression text) and `value_type`. |
| `BreakStatement` | A `break` statement. |
| `ContinueStatement` | A `continue` statement. |
| `RaiseStatement` | A `raise` statement. Carries `exception` (expression text). |
| `Literal` | A literal value (string, int, float, bool, None). Carries `value` and `literal_type`. |
| `AttributeAccess` | Accessing an attribute, e.g. `obj.attr`. Carries `object` and `attribute`. |

Each event includes a deterministic `node_id` (hash of file path + tree-sitter node position) and `file` path.

---

## 1. CfgSubscriber (`cfg.rs`)

**Purpose:** Collects all statements within each function and module scope. Acts as a simple statement collector - control flow edges are generated later by `graph.rs` based on `raw_bindings` data.

**Events consumed:** `EnterScope`, `ExitScope`, `Assignment`, `CallExpression`, `Return`, `BreakStatement`, `ContinueStatement`, `RaiseStatement`, `Literal`, `UseName`, `AttributeAccess`

**Events ignored:** `ControlBlock`, `EndControlBlock`, `ElseBlock` (these are handled by `RawBindingsSubscriber`)

### How It Works

The CFG subscriber maintains a **stack of `FunctionCfg` objects** (to handle nested function definitions) and an optional **module-level CFG** for code outside any function or class. Each `FunctionCfg` is simply a collection of statements with no internal structure:

- **Statements** (`Statement`): A flat list of statements with `line`, `column`, `stmt_type`, and `text`.
- **Parameters**: Function parameter names (captured from `EnterScope` events).

#### Scope Tracking

When `EnterScope(Function)` fires, a new `FunctionCfg` is created and pushed onto the function stack. When `ExitScope(Function)` fires, the CFG is popped and stored in `completed_cfgs`. Class scopes are tracked via `class_depth` to distinguish module-level code from class-level code.

**No control flow tracking** - the subscriber simply collects statements in the order they appear. Control flow edges (condition branches, loops, etc.) are constructed later by `graph.rs` using data from `RawBindingsSubscriber`.

#### Statement Handling

Statements are added to the current function's statement list in the order events arrive:

| Event | Statement Type | Details |
|---|---|---|
| `Assignment` | `assignment_target` | Only the LHS variable name; the RHS is handled by its own event. |
| `CallExpression` | `call` | The callee name (e.g., `"print"`, `"math.sqrt"`). Records in `consumed_callees` to suppress redundant UseName/AttributeAccess nodes. |
| `Literal` | `literal` | The literal value text (e.g., `"42"`, `"hello"`). |
| `UseName` | `name` | A variable reference. Skipped if already consumed by a call on the same line. |
| `AttributeAccess` | `attribute_access` | Full text like `"obj.attr"`. Skipped if consumed by a call on the same line. |
| `Return` | `return` | The return statement with optional value. |
| `BreakStatement` | `break` | Loop break statement. |
| `ContinueStatement` | `continue` | Loop continue statement. |
| `RaiseStatement` | `raise` | Exception raise statement with optional exception expression. |

#### JSON Output

The output is a flat list of statement nodes per function:

```json
{
  "cfgs": {
    "module.function_name": {
      "nodes": [
        {
          "id": "module.function_name::node_0",
          "line": 0,
          "column": 0,
          "type": "parameter",
          "text": "x"
        },
        {
          "id": "module.function_name::node_1",
          "line": 5,
          "column": 4,
          "type": "call",
          "text": "print"
        },
        {
          "id": "module.function_name::node_2",
          "line": 6,
          "column": 4,
          "type": "return",
          "text": "return x"
        }
      ]
    }
  }
}
```

**No edges in the output** - edges are generated separately by `graph.rs` from raw_bindings data and added to the root-level `cfg_edges` array in the final graph structure.

---

## 2. DefinitionsSubscriber (`definitions.rs`)

**Purpose:** Collects all name definitions in the codebase, organized by their containing scope. This provides a "what's defined where" index.

**Events consumed:** `EnterScope`, `ExitScope`, `DefineName`

### How It Works

Maintains a `scope_stack` of qualnames and a `definitions_by_scope` map. On each `EnterScope`, the scope qualname is pushed onto the stack, and the scope itself (class or function) is recorded as a definition in its *parent* scope. The module qualname is derived from the first scope event by stripping the scope name from the qualname (e.g. `"pkg.module.MyClass"` → `"pkg.module"`).

On `DefineName`, the definition is added to the current scope's list. Class and function definitions are skipped here (since they're already handled by `EnterScope`). If no scope context exists yet (definitions before the first `EnterScope`), definitions are stored in a `"<pending>"` list that gets assigned to the module scope once it's established.

### JSON Output

```json
{
  "definitions_by_scope": {
    "pkg.module": [
      { "node_id": "abc123", "name": "MyClass", "qualname": "pkg.module.MyClass", "type": "class", "line": 0 },
      { "node_id": "def456", "name": "APP_NAME", "qualname": "pkg.module.APP_NAME", "type": "variable", "line": 3 }
    ],
    "pkg.module.MyClass": [
      { "node_id": "ghi789", "name": "__init__", "qualname": "pkg.module.MyClass.__init__", "type": "function", "line": 0 }
    ]
  }
}
```

---

## 3. UsesSubscriber (`uses.rs`)

**Purpose:** Collects all name references (reads/uses of variables, functions, etc.), organized by their containing scope. Complements `DefinitionsSubscriber` by tracking where names are *used* rather than *defined*.

**Events consumed:** `EnterScope`, `ExitScope`, `UseName`

### How It Works

Structurally very similar to `DefinitionsSubscriber`. Maintains a `scope_stack` and `uses_by_scope` map. On `UseName`, records the name reference in the current scope. Uses a `"<pending>"` list for name uses that arrive before the first scope is established.

### JSON Output

```json
{
  "pkg.module.MyClass.__init__": [
    { "node_id": "abc123", "name": "os", "line": 5, "column": 12 },
    { "node_id": "def456", "name": "config", "line": 6, "column": 8 }
  ]
}
```

---

## 4. ImportsSubscriber (`imports.rs`)

**Purpose:** Collects all import statements for dependency resolution between modules. Used by the graph builder to create edges in the module dependency graph.

**Events consumed:** `ImportStatement`

### How It Works

The simplest subscriber. On each `ImportStatement`, stores an `ImportInfo` recording the source module, imported names, line number, and file path. Imports are indexed by file path in `imports_by_file`.

Handles all Python import forms:
- `import os` → source_module: `"os"`, imported_names: `[]`
- `from typing import List, Dict` → source_module: `"typing"`, imported_names: `["List", "Dict"]`
- `from .models import Foo` → source_module: resolved absolute path, imported_names: `["Foo"]`
- `from typing import *` → imported_names: `["*"]`

### JSON Output

```json
{
  "imports": [
    { "source_module": "os", "imported_names": [], "line": 1, "file": "app.py" },
    { "source_module": "typing", "imported_names": ["List", "Dict"], "line": 2, "file": "app.py" }
  ],
  "imports_by_file": {
    "app.py": [...]
  }
}
```

---

## 5. ScopeTreeSubscriber (`scope_tree.rs`)

**Purpose:** Builds a hierarchical tree of scopes (modules → classes → functions) for each file. Provides the structural skeleton of the codebase.

**Events consumed:** `EnterScope`, `ExitScope`

### How It Works

For each file, creates a root `ScopeNode` with `scope_type: "module"` (derived from the first scope event's qualname). Uses a stack of child indices (`scope_stacks`) to track the current nesting position in the tree.

On `EnterScope`, a new `ScopeNode` is created as a child of the current scope, with:
- `name`: the short name (e.g. `"MyClass"`)
- `qualname`: the fully qualified name (e.g. `"pkg.module.MyClass"`)
- `scope_type`: `"class"` or `"function"`
- `node_id`: deterministic hash ID
- `parameters`: function parameter names (empty for classes)

On `ExitScope`, the stack is popped to return to the parent scope.

### JSON Output

```json
{
  "files": [
    {
      "node_id": null,
      "name": "app",
      "qualname": "pkg.app",
      "scope_type": "module",
      "children": [
        {
          "node_id": "abc123",
          "name": "Engine",
          "qualname": "pkg.app.Engine",
          "scope_type": "class",
          "children": [
            {
              "node_id": "def456",
              "name": "__init__",
              "qualname": "pkg.app.Engine.__init__",
              "scope_type": "function",
              "parameters": ["horsepower", "fuel_type"],
              "children": []
            }
          ]
        }
      ]
    }
  ]
}
```

---

## 6. RawBindingsSubscriber (`raw_bindings.rs`)

**Purpose:** Tracks low-level dependency relationships between names, expressions, and blocks. Produces a flat list of directed bindings that form a graph of "what depends on what." Used as the foundation for higher-level dependency analysis.

**Events consumed:** `EnterScope`, `ExitScope`, `DefineName`, `CallExpression`, `UseName`, `Literal`, `Assignment`, `ControlBlock`, `EndControlBlock`, `Return`

### How It Works

Collects `RawBinding` records, each consisting of a source `BindingNode`, a `BindingType`, and a target `BindingNode`, scoped to the current function/module.

#### Binding Types

| Type | Meaning | Example |
|---|---|---|
| `ASSIGNED` | A name is assigned a value | `x = 42` → `x ASSIGNED 42` |
| `CALLS` | The current scope calls something | `scope CALLS fn()` |
| `WITH` | An argument is passed to a call | `fn() WITH arg` |
| `IMPORTS` | A name is imported | `os IMPORTS os` |
| `RETURNS` | A value is returned from a function | `scope RETURNS x` |
| `GUARDS` | A condition guards a block (true branch) | `condition GUARDS if-block` |
| `GUARDS_ELSE` | A condition's else branch | `condition GUARDS_ELSE else-block` |
| `CONTAINS` | A block contains a statement | `if-block CONTAINS call` |

#### Tracking Logic

- **Scope tracking**: Pushes/pops qualnames on a scope stack to determine `current_scope()` for each binding.
- **Call arguments**: When a `CallExpression` fires, the callee is stored in `pending_call`. Subsequent `UseName` or `Literal` events on the same line are treated as arguments, generating `WITH` bindings.
- **Control blocks**: Pushed onto `control_block_stack`. Statements inside a control block also get a `CONTAINS` binding.
- **Module qualname**: Derived from the file path if not set from a scope event.

Each `BindingNode` carries text, a category (`"name"`, `"literal"`, `"call"`, `"condition"`, `"block"`, `"scope"`, `"module"`, `"return"`, `"statement"`), line/column, and optional `node_id` and `qualname`.

### JSON Output

```json
[
  {
    "source": { "text": "pkg.module.main", "category": "scope", "line": 5, "column": 0 },
    "relationship": "CALLS",
    "target": { "text": "print", "category": "call", "line": 5, "column": 4, "node_id": "abc123" },
    "scope": "pkg.module.main",
    "line": 5
  },
  {
    "source": { "text": "print", "category": "call", "line": 5, "column": 0 },
    "relationship": "WITH",
    "target": { "text": "message", "category": "name", "line": 5, "column": 10 },
    "scope": "pkg.module.main",
    "line": 5
  }
]
```

---

## 7. EventCounterSubscriber (`event_counter.rs`)

**Purpose:** Counts the number of each event type emitted during analysis. Useful for debugging, testing, and understanding the distribution of language constructs in a codebase.

**Events consumed:** All events.

### How It Works

Maintains a `HashMap<String, usize>` mapping event type names to counts. Every event increments the corresponding counter. The event type names are lowercase snake_case strings: `define_name`, `enter_scope`, `exit_scope`, `use_name`, `call_expression`, `import_statement`, `control_block`, `end_control_block`, `return`, `attribute_access`, `literal`, `assignment`, `break_statement`, `continue_statement`, `raise_statement`, `else_block`.

### JSON Output

```json
{
  "event_counts": {
    "define_name": 15,
    "enter_scope": 8,
    "exit_scope": 8,
    "use_name": 42,
    "call_expression": 12,
    "assignment": 7,
    "literal": 5,
    "control_block": 3,
    "end_control_block": 3,
    "return": 4,
    "import_statement": 3,
    "attribute_access": 6,
    "else_block": 1
  },
  "total": 117
}
```

---

## Message Bus Architecture

All subscribers are orchestrated by the `MessageBus` (`rust/src/message_bus.rs`):

1. Subscriber factories are registered via `bus.register(factory)`.
2. On `bus.publish_events(events)`, fresh subscriber instances are created from each factory.
3. A bounded crossbeam channel (capacity 1000) is created for each subscriber.
4. Each subscriber runs on its own thread, consuming events from its channel.
5. Events are fanned out to all channels.
6. After all events are sent, channels are dropped to signal completion.
7. Each subscriber's `finalize()` result is collected and returned in registration order.

This parallel architecture means subscribers are fully independent and cannot communicate with each other during event processing.
