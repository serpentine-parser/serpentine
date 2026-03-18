//! Definitions subscriber - collects name definitions organized by scope.
//!
//! This subscriber listens to `DefineName`, `EnterScope`, and `ExitScope` events
//! to collect all definitions and organize them by their containing scope.

use std::collections::HashMap;

use crate::events::{Event, ScopeType};
use crate::message_bus::{Subscriber, SubscriberFactory, SubscriberResult};

/// A single definition within a scope.
#[derive(Debug, Clone)]
pub struct Definition {
    pub node_id: String,
    pub name: String,
    pub qualname: String,
    pub def_type: String, // "function", "class", "variable", "import", etc.
    pub line: usize,
}

impl Definition {
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "node_id": self.node_id,
            "name": self.name,
            "qualname": self.qualname,
            "type": self.def_type,
            "line": self.line,
        })
    }
}

/// Subscriber that collects definitions organized by scope.
///
/// Tracks the current scope using `EnterScope`/`ExitScope` events and
/// collects `DefineName` events into lists keyed by scope qualname.
pub struct DefinitionsSubscriber {
    name: String,
    /// Map from scope qualname to list of definitions in that scope
    definitions_by_scope: HashMap<String, Vec<Definition>>,
    /// Current scope stack (qualnames)
    scope_stack: Vec<String>,
    /// Module qualname (derived from first scope event)
    module_qualname: Option<String>,
}

impl DefinitionsSubscriber {
    pub fn new(name: &str) -> Self {
        DefinitionsSubscriber {
            name: name.to_string(),
            definitions_by_scope: HashMap::new(),
            scope_stack: Vec::new(),
            module_qualname: None,
        }
    }

    /// Get the current scope's qualname.
    fn current_scope(&self) -> Option<String> {
        self.scope_stack
            .last()
            .cloned()
            .or_else(|| self.module_qualname.clone())
    }

    /// Derive the module qualname from a scope's qualname.
    fn derive_module_qualname(qualname: &str, scope_name: &str) -> String {
        qualname
            .strip_suffix(&format!(".{}", scope_name))
            .unwrap_or(qualname)
            .to_string()
    }

    fn handle_enter_scope(
        &mut self,
        scope_type: &ScopeType,
        name: &str,
        qualname: &str,
        node_id: &str,
    ) {
        // Initialize module qualname from first scope event
        let is_first_scope = self.module_qualname.is_none();
        if is_first_scope {
            let module_qualname = Self::derive_module_qualname(qualname, name);
            self.module_qualname = Some(module_qualname.clone());

            // Move any pending definitions to the module scope
            if let Some(pending) = self.definitions_by_scope.remove("<pending>") {
                self.definitions_by_scope
                    .entry(module_qualname)
                    .or_default()
                    .extend(pending);
            }
        }

        // Push this scope onto the stack
        self.scope_stack.push(qualname.to_string());

        // Ensure there's an entry for this scope (even if empty)
        self.definitions_by_scope
            .entry(qualname.to_string())
            .or_default();

        // Also add a DefineName for the scope itself (class or function definition)
        let def_type = match scope_type {
            ScopeType::Module => "module",
            ScopeType::Class => "class",
            ScopeType::Function => "function",
            ScopeType::Lambda => "function",
            ScopeType::Comprehension => "comprehension",
            ScopeType::Interface => "interface",
        };

        // The definition belongs to the parent scope
        let parent_scope = if self.scope_stack.len() > 1 {
            self.scope_stack[self.scope_stack.len() - 2].clone()
        } else {
            self.module_qualname
                .clone()
                .unwrap_or_else(|| qualname.to_string())
        };

        let def = Definition {
            node_id: node_id.to_string(),
            name: name.to_string(),
            qualname: qualname.to_string(),
            def_type: def_type.to_string(),
            line: 0, // We don't have line info in EnterScope for the definition itself
        };

        self.definitions_by_scope
            .entry(parent_scope)
            .or_default()
            .push(def);
    }

    fn handle_exit_scope(&mut self) {
        self.scope_stack.pop();
    }

    fn handle_define_name(
        &mut self,
        name: &str,
        qualname: &str,
        node_type: &str,
        node_id: &str,
        line: usize,
    ) {
        // Language parsers must not emit a DefineName event for names that are
        // already introduced by a paired EnterScope event (e.g. function/class
        // names in Python).  If a language emits both, the node would be double-
        // counted here.  Trust the parser contract rather than filtering by type.

        // Get the current scope - if we don't have module context yet,
        // we need to wait for an EnterScope event to establish it
        let Some(current_scope) = self.current_scope() else {
            // No scope context yet - store in a temporary "pending" list
            // that will be assigned to module scope once we know it
            self.definitions_by_scope
                .entry("<pending>".to_string())
                .or_default()
                .push(Definition {
                    node_id: node_id.to_string(),
                    name: name.to_string(),
                    qualname: qualname.to_string(),
                    def_type: node_type.to_string(),
                    line,
                });
            return;
        };

        let def = Definition {
            node_id: node_id.to_string(),
            name: name.to_string(),
            qualname: qualname.to_string(),
            def_type: node_type.to_string(),
            line,
        };

        self.definitions_by_scope
            .entry(current_scope)
            .or_default()
            .push(def);
    }
}

impl Subscriber for DefinitionsSubscriber {
    fn handle_event(&mut self, event: &Event) -> Result<(), String> {
        match event {
            Event::EnterScope {
                scope_type,
                name,
                qualname,
                node_id,
                ..
            } => {
                self.handle_enter_scope(scope_type, name, qualname, node_id);
            }
            Event::ExitScope { .. } => {
                self.handle_exit_scope();
            }
            Event::DefineName {
                name,
                qualname,
                node_type,
                node_id,
                line,
                ..
            } => {
                self.handle_define_name(name, qualname, node_type, node_id, *line);
            }
            _ => {} // Ignore other events
        }
        Ok(())
    }

    fn finalize(&mut self) -> Result<SubscriberResult, String> {
        // Convert definitions to JSON, organized by scope
        let scopes: serde_json::Map<String, serde_json::Value> = self
            .definitions_by_scope
            .iter()
            .map(|(scope, defs)| {
                let defs_json: serde_json::Value =
                    defs.iter().map(|d| d.to_json()).collect::<Vec<_>>().into();
                (scope.clone(), defs_json)
            })
            .collect();

        Ok(SubscriberResult {
            subscriber_name: self.name.clone(),
            data: serde_json::json!({
                "definitions_by_scope": scopes,
            }),
        })
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Factory for creating `DefinitionsSubscriber` instances.
pub struct DefinitionsSubscriberFactory {
    name: String,
}

impl DefinitionsSubscriberFactory {
    pub fn new(name: &str) -> Self {
        DefinitionsSubscriberFactory {
            name: name.to_string(),
        }
    }
}

impl SubscriberFactory for DefinitionsSubscriberFactory {
    fn create(&self) -> Box<dyn Subscriber> {
        Box::new(DefinitionsSubscriber::new(&self.name))
    }
}
