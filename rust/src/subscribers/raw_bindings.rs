//! Raw Bindings subscriber - tracks dependency candidates between names.
//!
//! This subscriber collects raw binding relationships that represent potential
//! dependencies between definitions and their uses. These bindings form a
//! directed graph that can be used to build higher-level dependency graphs
//! and control flow analysis.
//!
//! Binding types:
//! - ASSIGNED: A name is assigned a value (x = value)
//! - CALLS: A function/method is called (fn())
//! - WITH: An argument is passed to a call (fn(arg))
//! - IMPORTS: A name is imported (import x)
//! - RETURNS: A value is returned from a function (return x)
//! - GUARDS: A condition guards a block (if condition -> then block)
//! - GUARDS_ELSE: A condition's else branch (if condition -> else block)
//! - CONTAINS: A block contains a statement/expression
//! - USES: An expression uses a variable (condition uses variable)

use crate::events::Event;
use crate::message_bus::{Subscriber, SubscriberFactory, SubscriberResult};

/// The type of binding relationship between two nodes.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)] // Variants are defined for future parser enhancements
pub enum BindingType {
    /// x = value - name is assigned a value
    Assigned,
    /// fn() - something calls something
    Calls,
    /// fn(arg) - argument passed to a call
    With,
    /// import x - a module/name is imported
    Imports,
    /// return x - a value is returned
    Returns,
    /// condition -> then block (true branch)
    Guards,
    /// condition -> else block (false branch)
    GuardsElse,
    /// block contains statement
    Contains,
    /// expression uses variable
    Uses,
}

impl BindingType {
    fn as_str(&self) -> &'static str {
        match self {
            BindingType::Assigned => "ASSIGNED",
            BindingType::Calls => "CALLS",
            BindingType::With => "WITH",
            BindingType::Imports => "IMPORTS",
            BindingType::Returns => "RETURNS",
            BindingType::Guards => "GUARDS",
            BindingType::GuardsElse => "GUARDS_ELSE",
            BindingType::Contains => "CONTAINS",
            BindingType::Uses => "USES",
        }
    }
}

/// A node in the binding graph - can be a name, literal, or expression.
#[derive(Debug, Clone)]
pub struct BindingNode {
    /// Unique deterministic node ID
    pub node_id: Option<String>,
    /// The text/name of this node
    pub text: String,
    /// Optional qualified name if this is a definition
    pub qualname: Option<String>,
    /// Node category: "name", "literal", "expression", "call", "condition", "block"
    pub category: String,
    /// Line number where this node appears
    pub line: usize,
    /// Column number
    pub column: usize,
}

impl BindingNode {
    fn new(text: &str, category: &str, line: usize, column: usize) -> Self {
        BindingNode {
            node_id: None,
            text: text.to_string(),
            qualname: None,
            category: category.to_string(),
            line,
            column,
        }
    }

    fn with_node_id(mut self, node_id: &str) -> Self {
        self.node_id = Some(node_id.to_string());
        self
    }

    fn with_qualname(mut self, qualname: &str) -> Self {
        self.qualname = Some(qualname.to_string());
        self
    }

    fn to_json(&self) -> serde_json::Value {
        let mut obj = serde_json::json!({
            "text": self.text,
            "category": self.category,
            "line": self.line,
            "column": self.column,
        });
        if let Some(id) = &self.node_id {
            obj["node_id"] = serde_json::Value::String(id.clone());
        }
        if let Some(qn) = &self.qualname {
            obj["qualname"] = serde_json::Value::String(qn.clone());
        }
        obj
    }
}

/// A single binding/edge in the dependency graph.
#[derive(Debug, Clone)]
pub struct RawBinding {
    /// The source node (subject of the relationship)
    pub source: BindingNode,
    /// The type of relationship
    pub binding_type: BindingType,
    /// The target node (object of the relationship)
    pub target: BindingNode,
    /// The scope/context where this binding occurs
    pub scope: String,
    /// Line number of the binding
    pub line: usize,
}

impl RawBinding {
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "source": self.source.to_json(),
            "relationship": self.binding_type.as_str(),
            "target": self.target.to_json(),
            "scope": self.scope,
            "line": self.line,
        })
    }
}

/// Info about a control block on the stack
struct ControlBlockInfo {
    node_id: String,
    block_type: String,
    line: usize,
}

/// Subscriber that collects raw binding relationships.
pub struct RawBindingsSubscriber {
    name: String,
    /// All bindings collected
    bindings: Vec<RawBinding>,
    /// Current scope stack (qualnames)
    scope_stack: Vec<String>,
    /// Current control block stack (for CONTAINS bindings)
    control_block_stack: Vec<ControlBlockInfo>,
    /// Module qualname (derived from first scope event)
    module_qualname: Option<String>,
    /// Pending call info for associating WITH bindings
    /// (callee_text, node_id, line)
    pending_call: Option<(String, String, usize)>,
}

impl RawBindingsSubscriber {
    pub fn new(name: &str) -> Self {
        RawBindingsSubscriber {
            name: name.to_string(),
            bindings: Vec::new(),
            scope_stack: Vec::new(),
            control_block_stack: Vec::new(),
            module_qualname: None,
            pending_call: None,
        }
    }

    fn current_scope(&self) -> String {
        self.scope_stack
            .last()
            .cloned()
            .or_else(|| self.module_qualname.clone())
            .unwrap_or_else(|| "unknown".to_string())
    }

    /// Set the module qualname from file path info (called before first event)
    fn ensure_module_qualname(&mut self, file: &str) {
        if self.module_qualname.is_none() {
            // Derive module name from file path
            self.module_qualname = Some(Self::derive_module_from_file(file));
        }
    }

    fn derive_module_from_file(file_path: &str) -> String {
        use std::path::Path;
        let path = Path::new(file_path);
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("__main__");

        // Build module path from parent directories, starting from "src" if present
        // This must match the qualname format used by Events
        let mut parts = Vec::new();
        let mut found_src = false;

        // Collect path components starting from "src" (or first component if no "src")
        for component in path.parent().iter().flat_map(|p| p.components()) {
            if let std::path::Component::Normal(name) = component {
                if let Some(name_str) = name.to_str() {
                    // Start collecting from "src" onwards
                    if name_str == "src" {
                        found_src = true;
                    }

                    if found_src && !matches!(name_str, "." | "..") {
                        parts.push(name_str.to_string());
                    }
                }
            }
        }

        // If we didn't find "src", use the last component as a fallback
        if parts.is_empty() {
            if let Some(parent) = path.parent() {
                if let Some(parent_name) = parent.file_name().and_then(|s| s.to_str()) {
                    parts.push(parent_name.to_string());
                }
            }
        }

        parts.push(stem.to_string());
        parts.join(".")
    }

    fn handle_enter_scope(&mut self, _name: &str, qualname: &str) {
        self.scope_stack.push(qualname.to_string());
    }

    fn handle_exit_scope(&mut self) {
        self.scope_stack.pop();
    }

    fn handle_define_name(
        &mut self,
        name: &str,
        qualname: &str,
        node_type: &str,
        line: usize,
        column: usize,
    ) {
        // For imports, create an IMPORTS binding
        if node_type == "import" {
            let source = BindingNode::new(name, "name", line, column).with_qualname(qualname);
            let target = BindingNode::new(name, "module", line, column);

            self.bindings.push(RawBinding {
                source,
                binding_type: BindingType::Imports,
                target,
                scope: self.current_scope(),
                line,
            });
        }

        // For variables, we might have a pending assignment value
        // This will be enhanced when we add assignment value tracking
    }

    fn handle_call_expression(&mut self, callee: &str, node_id: &str, line: usize, column: usize) {
        // Create a CALLS binding from current scope to the callee
        let source = BindingNode::new(&self.current_scope(), "scope", line, 0);
        let target = BindingNode::new(callee, "call", line, column).with_node_id(node_id);

        self.bindings.push(RawBinding {
            source,
            binding_type: BindingType::Calls,
            target: target.clone(),
            scope: self.current_scope(),
            line,
        });

        // Store pending call for WITH bindings
        self.pending_call = Some((callee.to_string(), node_id.to_string(), line));
    }

    fn handle_use_name(&mut self, name: &str, line: usize, column: usize) {
        // If we have a pending call on the same line, this might be an argument
        if let Some((callee, call_node_id, call_line)) = &self.pending_call {
            if *call_line == line {
                // This is an argument to the call - data flows FROM argument TO call
                let source = BindingNode::new(name, "name", line, column);
                let target = BindingNode::new(callee, "call", line, 0).with_node_id(call_node_id);

                self.bindings.push(RawBinding {
                    source,
                    binding_type: BindingType::With,
                    target,
                    scope: self.current_scope(),
                    line,
                });
            }
        }
    }

    fn handle_literal(&mut self, value: &str, literal_type: &str, line: usize, column: usize) {
        // If we have a pending call on the same line, this literal is an argument
        if let Some((callee, call_node_id, call_line)) = &self.pending_call {
            if *call_line == line {
                // Literal flows FROM literal TO call
                let source = BindingNode::new(value, literal_type, line, column);
                let target = BindingNode::new(callee, "call", line, 0).with_node_id(call_node_id);

                self.bindings.push(RawBinding {
                    source,
                    binding_type: BindingType::With,
                    target,
                    scope: self.current_scope(),
                    line,
                });
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_assignment(
        &mut self,
        node_id: &str,
        target: &str,
        target_qualname: &str,
        value: &str,
        value_type: &str,
        line: usize,
        column: usize,
    ) {
        // Source is the assignment target variable (with node_id for the assignment_target CFG node)
        let source = BindingNode::new(target, "name", line, column)
            .with_qualname(target_qualname)
            .with_node_id(node_id);
        // Target is the value expression (no node_id - it's the RHS expression)
        let target_node = BindingNode::new(value, value_type, line, column);

        self.bindings.push(RawBinding {
            source,
            binding_type: BindingType::Assigned,
            target: target_node,
            scope: self.current_scope(),
            line,
        });
    }

    fn handle_control_block(
        &mut self,
        block_type: &str,
        node_id: &str,
        condition: &str,
        line: usize,
    ) {
        // Push control block onto stack
        self.control_block_stack.push(ControlBlockInfo {
            node_id: node_id.to_string(),
            block_type: block_type.to_string(),
            line,
        });

        // Create GUARDS binding: condition guards this block
        if !condition.is_empty() {
            let source = BindingNode::new(condition, "condition", line, 0).with_node_id(node_id);
            let target = BindingNode::new(block_type, "block", line, 0).with_node_id(node_id);

            // Determine if this is an else branch (GuardsElse) or regular guard
            let binding_type = if block_type == "else" || block_type == "elif" {
                BindingType::GuardsElse
            } else {
                BindingType::Guards
            };

            self.bindings.push(RawBinding {
                source,
                binding_type,
                target,
                scope: self.current_scope(),
                line,
            });

            // Extract variable references from condition and create USES bindings
            self.extract_variable_uses(condition, node_id, line);
        }
    }

    fn handle_end_control_block(&mut self) {
        self.control_block_stack.pop();
    }

    /// Extract variable references from an expression and create USES bindings.
    /// This parses the expression text to identify variable names.
    ///
    /// Language-agnostic: relies only on structural token properties (starts with digit,
    /// starts with quote) rather than any language-specific keyword list. Language parsers
    /// are responsible for not emitting spurious identifier events for their keywords.
    fn extract_variable_uses(&mut self, expression: &str, expr_node_id: &str, line: usize) {
        // Simple tokenization approach:
        // 1. Split on operators and whitespace
        // 2. Filter for valid identifiers
        // 3. Exclude numeric and string literals

        // Split on common operators and delimiters
        let delimiters = [
            " ", "(", ")", "[", "]", "{", "}", ",", ":", ";",
            "+", "-", "*", "/", "%", "=", "!", "<", ">", "&", "|", "^", "~",
            ".", "\t", "\n",
        ];

        let mut tokens = Vec::new();
        let mut current_token = String::new();

        for ch in expression.chars() {
            if delimiters.iter().any(|&d| d.starts_with(ch)) {
                if !current_token.is_empty() {
                    tokens.push(current_token.clone());
                    current_token.clear();
                }
            } else {
                current_token.push(ch);
            }
        }
        if !current_token.is_empty() {
            tokens.push(current_token);
        }

        // Filter for valid identifiers (not numeric or string literals)
        for token in tokens {
            // Skip numeric literals
            if token.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                continue;
            }

            // Skip string literals (very basic check)
            if token.starts_with('"') || token.starts_with('\'') {
                continue;
            }

            // Check if it looks like a valid identifier
            if token.chars().all(|c| c.is_alphanumeric() || c == '_')
                && !token.is_empty()
            {
                // Create USES binding: expression node → variable name
                let source = BindingNode::new(expression, "condition", line, 0)
                    .with_node_id(expr_node_id);
                let target = BindingNode::new(&token, "name", line, 0);

                self.bindings.push(RawBinding {
                    source: source.clone(),
                    binding_type: BindingType::Uses,
                    target,
                    scope: self.current_scope(),
                    line,
                });
            }
        }
    }

    fn handle_return(&mut self, value: &str, node_id: &str, line: usize, column: usize) {
        let source = BindingNode::new(&self.current_scope(), "scope", line, 0);
        let target = BindingNode::new(value, "return", line, column).with_node_id(node_id);

        self.bindings.push(RawBinding {
            source,
            binding_type: BindingType::Returns,
            target,
            scope: self.current_scope(),
            line,
        });
    }

    /// Create a CONTAINS binding when a statement occurs inside a control block
    fn add_contains_binding(&mut self, statement_node_id: &str, statement_type: &str, line: usize) {
        if let Some(control_block) = self.control_block_stack.last() {
            let source =
                BindingNode::new(&control_block.block_type, "block", control_block.line, 0)
                    .with_node_id(&control_block.node_id);
            let target = BindingNode::new(statement_type, "statement", line, 0)
                .with_node_id(statement_node_id);

            self.bindings.push(RawBinding {
                source,
                binding_type: BindingType::Contains,
                target,
                scope: self.current_scope(),
                line,
            });
        }
    }
}

impl Subscriber for RawBindingsSubscriber {
    fn name(&self) -> &str {
        &self.name
    }

    fn handle_event(&mut self, event: &Event) -> Result<(), String> {
        // Ensure module qualname is set from any event with a file path
        match event {
            Event::DefineName { file, .. }
            | Event::EnterScope { file, .. }
            | Event::CallExpression { file, .. }
            | Event::Literal { file, .. }
            | Event::ControlBlock { file, .. }
            | Event::Return { file, .. } => {
                self.ensure_module_qualname(file);
            }
            _ => {}
        }

        match event {
            Event::EnterScope { name, qualname, .. } => {
                self.handle_enter_scope(name, qualname);
            }
            Event::ExitScope { .. } => {
                self.handle_exit_scope();
            }
            Event::DefineName {
                name,
                qualname,
                node_type,
                line,
                column,
                ..
            } => {
                self.handle_define_name(name, qualname, node_type, *line, *column);
            }
            Event::CallExpression {
                callee,
                node_id,
                line,
                column,
                ..
            } => {
                self.handle_call_expression(callee, node_id, *line, *column);
                self.add_contains_binding(node_id, "call", *line);
            }
            Event::UseName {
                name, line, column, ..
            } => {
                self.handle_use_name(name, *line, *column);
            }
            Event::Literal {
                value,
                literal_type,
                line,
                column,
                ..
            } => {
                self.handle_literal(value, literal_type, *line, *column);
            }
            Event::Assignment {
                target,
                target_qualname,
                value,
                value_type,
                node_id,
                line,
                column,
                file,
            } => {
                self.ensure_module_qualname(file);
                self.handle_assignment(node_id, target, target_qualname, value, value_type, *line, *column);
                self.add_contains_binding(node_id, "assignment", *line);
            }
            Event::ControlBlock {
                block_type,
                node_id,
                condition,
                line,
                ..
            } => {
                self.handle_control_block(block_type, node_id, condition, *line);
            }
            Event::ElseBlock {
                block_type,
                node_id,
                condition: _,
                line,
                ..
            } => {
                // For else blocks, we need to create a GUARDS_ELSE binding from the condition
                // to this else block (which has its own unique node_id)
                // The condition comes from the parent if statement
                if let Some(parent_block) = self.control_block_stack.last() {
                    let source = BindingNode::new(&parent_block.block_type, "condition", parent_block.line, 0)
                        .with_node_id(&parent_block.node_id);
                    let target = BindingNode::new(block_type, "block", *line, 0)
                        .with_node_id(node_id);

                    self.bindings.push(RawBinding {
                        source,
                        binding_type: BindingType::GuardsElse,
                        target,
                        scope: self.current_scope(),
                        line: *line,
                    });
                }

                // Push the else block onto the stack for CONTAINS bindings
                self.control_block_stack.push(ControlBlockInfo {
                    node_id: node_id.to_string(),
                    block_type: block_type.to_string(),
                    line: *line,
                });
            }
            Event::EndControlBlock { .. } => {
                self.handle_end_control_block();
            }
            Event::Return {
                value,
                node_id,
                line,
                column,
                ..
            } => {
                self.handle_return(value, node_id, *line, *column);
                self.add_contains_binding(node_id, "return", *line);
            }
            _ => {}
        }
        Ok(())
    }

    fn finalize(&mut self) -> Result<SubscriberResult, String> {
        let bindings_json: Vec<serde_json::Value> =
            self.bindings.iter().map(|b| b.to_json()).collect();

        Ok(SubscriberResult {
            subscriber_name: self.name.clone(),
            data: serde_json::Value::Array(bindings_json),
        })
    }
}

/// Factory for creating RawBindingsSubscriber instances.
pub struct RawBindingsSubscriberFactory {
    name: String,
}

impl RawBindingsSubscriberFactory {
    pub fn new(name: &str) -> Self {
        RawBindingsSubscriberFactory {
            name: name.to_string(),
        }
    }
}

impl SubscriberFactory for RawBindingsSubscriberFactory {
    fn create(&self) -> Box<dyn Subscriber> {
        Box::new(RawBindingsSubscriber::new(&self.name))
    }
}
