//! Scope tree subscriber - builds hierarchical scope trees from events.
//!
//! This subscriber listens to `EnterScope` and `ExitScope` events to build
//! a tree representation of the scope hierarchy (modules, classes, functions).

use std::collections::HashMap;

use crate::events::{Event, ScopeType};
use crate::message_bus::{Subscriber, SubscriberFactory, SubscriberResult};

/// A node in the scope tree.
#[derive(Debug, Clone)]
pub struct ScopeNode {
    pub node_id: Option<String>,
    pub name: String,
    pub qualname: String,
    pub scope_type: String,      // "module", "class", "function"
    pub parameters: Vec<String>, // Function parameters (excluding self/cls)
    pub bases: Vec<String>,      // Base class names (for classes only)
    pub children: Vec<ScopeNode>,
}

impl ScopeNode {
    fn new(name: String, qualname: String, scope_type: &str) -> Self {
        ScopeNode {
            node_id: None,
            name,
            qualname,
            scope_type: scope_type.to_string(),
            parameters: Vec::new(),
            bases: Vec::new(),
            children: Vec::new(),
        }
    }

    fn with_node_id(mut self, node_id: &str) -> Self {
        self.node_id = Some(node_id.to_string());
        self
    }

    fn with_parameters(mut self, params: Vec<String>) -> Self {
        self.parameters = params;
        self
    }

    fn with_bases(mut self, bases: Vec<String>) -> Self {
        self.bases = bases;
        self
    }

    fn to_json(&self) -> serde_json::Value {
        let mut obj = serde_json::json!({
            "node_id": self.node_id,
            "name": self.name,
            "qualname": self.qualname,
            "scope_type": self.scope_type,
            "children": self.children.iter().map(|c| c.to_json()).collect::<Vec<_>>(),
        });
        if !self.parameters.is_empty() {
            obj["parameters"] = serde_json::json!(self.parameters);
        }
        if !self.bases.is_empty() {
            obj["bases"] = serde_json::json!(self.bases);
        }
        obj
    }
}

/// Subscriber that builds scope trees for each file.
///
/// Listens to `EnterScope` and `ExitScope` events to construct a hierarchical
/// representation of scopes (modules containing classes containing methods, etc.).
pub struct ScopeTreeSubscriber {
    name: String,
    /// Map from file path to root scope node
    file_trees: HashMap<String, ScopeNode>,
    /// Current scope stack per file (indices into children vectors)
    scope_stacks: HashMap<String, Vec<usize>>,
}

impl ScopeTreeSubscriber {
    pub fn new(name: &str) -> Self {
        ScopeTreeSubscriber {
            name: name.to_string(),
            file_trees: HashMap::new(),
            scope_stacks: HashMap::new(),
        }
    }

    /// Get or create the root node for a file.
    ///
    /// The module name/qualname is derived from the first scope's qualname.
    /// For example, if the first scope has qualname "test_package.app.Engine",
    /// the module qualname is "test_package.app" and name is "app".
    fn ensure_file_root(&mut self, file: &str, qualname: &str, scope_name: &str) -> &mut ScopeNode {
        if !self.file_trees.contains_key(file) {
            // Derive module qualname by stripping the scope name from the full qualname
            let module_qualname = qualname
                .strip_suffix(&format!(".{}", scope_name))
                .unwrap_or(qualname)
                .to_string();

            // Module name is the last part of the module qualname
            let module_name = module_qualname
                .rsplit('.')
                .next()
                .unwrap_or(&module_qualname)
                .to_string();

            let root = ScopeNode::new(module_name, module_qualname, "module");
            self.file_trees.insert(file.to_string(), root);
            self.scope_stacks.insert(file.to_string(), Vec::new());
        }
        self.file_trees.get_mut(file).unwrap()
    }

    /// Navigate to the current scope node using the stack.
    fn get_current_scope(&mut self, file: &str) -> &mut ScopeNode {
        let stack = self.scope_stacks.get(file).unwrap().clone();
        let mut current = self.file_trees.get_mut(file).unwrap();

        for idx in stack {
            current = &mut current.children[idx];
        }

        current
    }

    fn handle_enter_scope(
        &mut self,
        scope_type: &ScopeType,
        name: &str,
        qualname: &str,
        node_id: &str,
        parameters: &[String],
        bases: &[String],
        file: &str,
    ) {
        // Ensure root exists (derives module name from first scope's qualname)
        self.ensure_file_root(file, qualname, name);

        let scope_type_str = match scope_type {
            ScopeType::Module => "module",
            ScopeType::Class => "class",
            ScopeType::Function => "function",
            ScopeType::Lambda => "lambda",
            ScopeType::Comprehension => "comprehension",
            ScopeType::Interface => "interface",
        };

        // Create new scope node and add to current scope's children
        let new_node = ScopeNode::new(name.to_string(), qualname.to_string(), scope_type_str)
            .with_node_id(node_id)
            .with_parameters(parameters.to_vec())
            .with_bases(bases.to_vec());
        let current = self.get_current_scope(file);
        current.children.push(new_node);
        let new_idx = current.children.len() - 1;

        // Push onto stack to track nesting
        self.scope_stacks.get_mut(file).unwrap().push(new_idx);
    }

    fn handle_exit_scope(&mut self, file: &str) {
        if let Some(stack) = self.scope_stacks.get_mut(file) {
            stack.pop();
        }
    }
}

impl Subscriber for ScopeTreeSubscriber {
    fn handle_event(&mut self, event: &Event) -> Result<(), String> {
        match event {
            Event::EnterScope {
                scope_type,
                name,
                qualname,
                node_id,
                parameters,
                bases,
                file,
                ..
            } => {
                self.handle_enter_scope(scope_type, name, qualname, node_id, parameters, bases, file);
            }
            Event::ExitScope { file, .. } => {
                self.handle_exit_scope(file);
            }
            _ => {} // Other events handled by different subscribers
        }
        Ok(())
    }

    fn finalize(&mut self) -> Result<SubscriberResult, String> {
        let trees: Vec<serde_json::Value> = self
            .file_trees
            .values()
            .map(|node| node.to_json())
            .collect();

        Ok(SubscriberResult {
            subscriber_name: self.name.clone(),
            data: serde_json::json!({
                "files": trees,
            }),
        })
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Factory for creating `ScopeTreeSubscriber` instances.
pub struct ScopeTreeSubscriberFactory {
    name: String,
}

impl ScopeTreeSubscriberFactory {
    pub fn new(name: &str) -> Self {
        ScopeTreeSubscriberFactory {
            name: name.to_string(),
        }
    }
}

impl SubscriberFactory for ScopeTreeSubscriberFactory {
    fn create(&self) -> Box<dyn Subscriber> {
        Box::new(ScopeTreeSubscriber::new(&self.name))
    }
}
