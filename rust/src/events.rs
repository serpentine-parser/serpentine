use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use tree_sitter::Node;

/// Generate a deterministic node ID from file path and tree-sitter node position.
/// This ID will be the same across runs for the same source code.
pub fn generate_node_id(file: &str, node: Node) -> String {
    let mut hasher = DefaultHasher::new();
    file.hash(&mut hasher);
    node.kind().hash(&mut hasher);
    node.start_byte().hash(&mut hasher);
    node.end_byte().hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Semantic events emitted during tree walking
/// Based on the event model in app.py
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields will be used by subscribers in the future
pub enum Event {
    /// A name is being defined (variable, function, class, import, etc.)
    DefineName {
        node_id: String,
        name: String,
        qualname: String,
        node_type: String, // e.g., "function", "class", "variable", "import"
        file: String,
        line: usize,
        end_line: usize, // Last line of the definition (same as line for single-line defs)
        column: usize,
    },

    /// Entering a new scope (function or class)
    EnterScope {
        node_id: String,
        scope_type: ScopeType, // "function" or "class"
        name: String,
        qualname: String,
        parameters: Vec<String>, // Parameter names (for functions; empty for classes)
        bases: Vec<String>,      // Base class names (for classes only; empty for functions)
        docstring: Option<String>, // First string literal in the scope body, if any
        file: String,
        line: usize,
    },

    /// Exiting a scope (function or class)
    ExitScope {
        node_id: String,
        scope_type: ScopeType,
        name: String,
        qualname: String,
        file: String,
        line: usize,
    },

    /// A name is being used/referenced
    UseName {
        node_id: String,
        name: String,
        file: String,
        line: usize,
        column: usize,
    },

    /// A function/method is being called
    CallExpression {
        node_id: String,
        callee: String,         // The name/expression being called
        arguments: Vec<String>, // Text of each top-level argument expression
        file: String,
        line: usize,
        column: usize,
    },

    /// An import statement
    ImportStatement {
        node_id: String,
        module: String,
        names: Vec<String>, // Names being imported (empty for `import x`)
        aliases: std::collections::HashMap<String, String>, // original_name → alias
        is_type_checking: bool, // True if inside `if TYPE_CHECKING:` block
        file: String,
        line: usize,
    },

    /// A control flow block (if, for, while, etc.)
    ControlBlock {
        node_id: String,
        block_type: String, // "if", "for", "while", "try", etc.
        condition: String,  // The condition expression (e.g., "x > 10")
        file: String,
        line: usize,
    },

    /// End of a control flow block
    EndControlBlock {
        node_id: String,
        block_type: String,
        file: String,
        line: usize,
    },

    /// A return statement
    Return {
        node_id: String,
        value: String,      // The return expression (empty for bare return)
        value_type: String, // "literal", "name", "call", "expression", "none"
        file: String,
        line: usize,
        column: usize,
    },

    /// Accessing an attribute (e.g., obj.attr)
    AttributeAccess {
        node_id: String,
        object: String,
        attribute: String,
        file: String,
        line: usize,
        column: usize,
    },

    /// A literal value (string, number, boolean, None)
    Literal {
        node_id: String,
        value: String,
        literal_type: String, // "string", "integer", "float", "boolean", "none"
        file: String,
        line: usize,
        column: usize,
    },

    /// An assignment statement (x = value)
    Assignment {
        node_id: String,
        target: String,          // The name being assigned to
        target_qualname: String, // Qualified name of the target
        value: String,           // The text of the value expression
        value_type: String,      // "literal", "call", "name", "expression"
        file: String,
        line: usize,
        column: usize,
    },

    /// A break statement (loop control)
    BreakStatement {
        node_id: String,
        file: String,
        line: usize,
    },

    /// A continue statement (loop restart)
    ContinueStatement {
        node_id: String,
        file: String,
        line: usize,
    },

    /// A raise statement (exception throwing)
    RaiseStatement {
        node_id: String,
        exception: String, // The exception expression (empty for bare raise)
        file: String,
        line: usize,
    },

    /// An else/elif clause within a control block
    ElseBlock {
        node_id: String,
        block_type: String, // "else" or "elif"
        condition: String,  // For elif: the condition; for else: empty
        file: String,
        line: usize,
    },

    /// A decorator applied to a function or class
    Decorator {
        node_id: String,
        name: String,           // The full decorator expression text (@click.command → "click.command")
        is_call: bool,          // True if decorator is invoked (@click.command())
        arguments: Vec<String>, // Arguments if is_call
        file: String,
        line: usize,
        column: usize,
    },

    /// A yield expression (marks function as generator)
    YieldExpression {
        node_id: String,
        value: String,   // The yielded expression (empty for bare yield)
        is_from: bool,   // True for `yield from`
        file: String,
        line: usize,
        column: usize,
    },

    /// A single line of source code from a file
    SourceLine {
        file: String,
        line_number: usize, // 1-indexed
        text: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScopeType {
    /// File-level module scope (JS/TS files, future Rust modules).
    Module,
    Function,
    Class,
    Lambda,
    Comprehension,
    /// Structural type contract: TypeScript interface/object-shape type, Rust trait, etc.
    Interface,
}

impl Event {
    /// Helper to create a DefineName event from a tree-sitter node.
    /// end_line defaults to the same line as the start (single-line definition).
    pub fn define_name(
        name: String,
        qualname: String,
        node_type: &str,
        node: Node,
        file: &str,
    ) -> Self {
        let pos = node.start_position();
        let line = pos.row + 1;
        Event::DefineName {
            node_id: generate_node_id(file, node),
            name,
            qualname,
            node_type: node_type.to_string(),
            file: file.to_string(),
            line,
            end_line: line,
            column: pos.column,
        }
    }

    /// Helper to create an EnterScope event (docstring defaults to None)
    pub fn enter_scope(
        scope_type: ScopeType,
        name: String,
        qualname: String,
        parameters: Vec<String>,
        bases: Vec<String>,
        node: Node,
        file: &str,
    ) -> Self {
        let pos = node.start_position();
        Event::EnterScope {
            node_id: generate_node_id(file, node),
            scope_type,
            name,
            qualname,
            parameters,
            bases,
            docstring: None,
            file: file.to_string(),
            line: pos.row + 1,
        }
    }

    /// Helper to create an EnterScope event with an optional docstring
    #[allow(clippy::too_many_arguments)]
    pub fn enter_scope_with_docstring(
        scope_type: ScopeType,
        name: String,
        qualname: String,
        parameters: Vec<String>,
        bases: Vec<String>,
        docstring: Option<String>,
        node: Node,
        file: &str,
    ) -> Self {
        let pos = node.start_position();
        Event::EnterScope {
            node_id: generate_node_id(file, node),
            scope_type,
            name,
            qualname,
            parameters,
            bases,
            docstring,
            file: file.to_string(),
            line: pos.row + 1,
        }
    }

    /// Helper to create an ExitScope event
    pub fn exit_scope(
        scope_type: ScopeType,
        name: String,
        qualname: String,
        node: Node,
        file: &str,
    ) -> Self {
        let pos = node.end_position();
        Event::ExitScope {
            node_id: generate_node_id(file, node),
            scope_type,
            name,
            qualname,
            file: file.to_string(),
            line: pos.row + 1,
        }
    }

    /// Helper to create a UseName event
    pub fn use_name(name: String, node: Node, file: &str) -> Self {
        let pos = node.start_position();
        Event::UseName {
            node_id: generate_node_id(file, node),
            name,
            file: file.to_string(),
            line: pos.row + 1,
            column: pos.column,
        }
    }

    /// Helper to create a CallExpression event
    pub fn call_expression(callee: String, arguments: Vec<String>, node: Node, file: &str) -> Self {
        let pos = node.start_position();
        Event::CallExpression {
            node_id: generate_node_id(file, node),
            callee,
            arguments,
            file: file.to_string(),
            line: pos.row + 1,
            column: pos.column,
        }
    }

    /// Helper to create an ImportStatement event
    pub fn import_statement(
        module: String,
        names: Vec<String>,
        aliases: std::collections::HashMap<String, String>,
        is_type_checking: bool,
        node: Node,
        file: &str,
    ) -> Self {
        let pos = node.start_position();
        Event::ImportStatement {
            node_id: generate_node_id(file, node),
            module,
            names,
            aliases,
            is_type_checking,
            file: file.to_string(),
            line: pos.row + 1,
        }
    }

    /// Helper to create a ControlBlock event
    pub fn control_block(block_type: &str, condition: String, node: Node, file: &str) -> Self {
        let pos = node.start_position();
        Event::ControlBlock {
            node_id: generate_node_id(file, node),
            block_type: block_type.to_string(),
            condition,
            file: file.to_string(),
            line: pos.row + 1,
        }
    }

    /// Helper to create an EndControlBlock event
    pub fn end_control_block(block_type: &str, node: Node, file: &str) -> Self {
        let pos = node.end_position();
        Event::EndControlBlock {
            node_id: generate_node_id(file, node),
            block_type: block_type.to_string(),
            file: file.to_string(),
            line: pos.row + 1,
        }
    }

    /// Helper to create a Return event
    pub fn return_stmt(value: String, value_type: &str, node: Node, file: &str) -> Self {
        let pos = node.start_position();
        Event::Return {
            node_id: generate_node_id(file, node),
            value,
            value_type: value_type.to_string(),
            file: file.to_string(),
            line: pos.row + 1,
            column: pos.column,
        }
    }

    /// Helper to create an AttributeAccess event
    pub fn attribute_access(object: String, attribute: String, node: Node, file: &str) -> Self {
        let pos = node.start_position();
        Event::AttributeAccess {
            node_id: generate_node_id(file, node),
            object,
            attribute,
            file: file.to_string(),
            line: pos.row + 1,
            column: pos.column,
        }
    }

    /// Helper to create a Literal event
    pub fn literal(value: String, literal_type: &str, node: Node, file: &str) -> Self {
        let pos = node.start_position();
        Event::Literal {
            node_id: generate_node_id(file, node),
            value,
            literal_type: literal_type.to_string(),
            file: file.to_string(),
            line: pos.row + 1,
            column: pos.column,
        }
    }

    /// Helper to create an Assignment event
    pub fn assignment(
        target: String,
        target_qualname: String,
        value: String,
        value_type: &str,
        node: Node,
        file: &str,
    ) -> Self {
        let pos = node.start_position();
        Event::Assignment {
            node_id: generate_node_id(file, node),
            target,
            target_qualname,
            value,
            value_type: value_type.to_string(),
            file: file.to_string(),
            line: pos.row + 1,
            column: pos.column,
        }
    }

    /// Helper to create a BreakStatement event
    pub fn break_statement(node: Node, file: &str) -> Self {
        let pos = node.start_position();
        Event::BreakStatement {
            node_id: generate_node_id(file, node),
            file: file.to_string(),
            line: pos.row + 1,
        }
    }

    /// Helper to create a ContinueStatement event
    pub fn continue_statement(node: Node, file: &str) -> Self {
        let pos = node.start_position();
        Event::ContinueStatement {
            node_id: generate_node_id(file, node),
            file: file.to_string(),
            line: pos.row + 1,
        }
    }

    /// Helper to create a RaiseStatement event
    pub fn raise_statement(exception: String, node: Node, file: &str) -> Self {
        let pos = node.start_position();
        Event::RaiseStatement {
            node_id: generate_node_id(file, node),
            exception,
            file: file.to_string(),
            line: pos.row + 1,
        }
    }

    /// Helper to create an ElseBlock event
    pub fn else_block(block_type: &str, condition: String, node: Node, file: &str) -> Self {
        let pos = node.start_position();
        Event::ElseBlock {
            node_id: generate_node_id(file, node),
            block_type: block_type.to_string(),
            condition,
            file: file.to_string(),
            line: pos.row + 1,
        }
    }

    /// Helper to create a Decorator event
    pub fn decorator(name: String, is_call: bool, arguments: Vec<String>, node: Node, file: &str) -> Self {
        let pos = node.start_position();
        Event::Decorator {
            node_id: generate_node_id(file, node),
            name,
            is_call,
            arguments,
            file: file.to_string(),
            line: pos.row + 1,
            column: pos.column,
        }
    }

    /// Helper to create a YieldExpression event
    pub fn yield_expression(value: String, is_from: bool, node: Node, file: &str) -> Self {
        let pos = node.start_position();
        Event::YieldExpression {
            node_id: generate_node_id(file, node),
            value,
            is_from,
            file: file.to_string(),
            line: pos.row + 1,
            column: pos.column,
        }
    }
}
