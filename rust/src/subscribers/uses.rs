//! Uses subscriber - collects name uses organized by scope.
//!
//! This subscriber listens to `UseName`, `EnterScope`, and `ExitScope` events
//! to collect all name references and organize them by their containing scope.

use std::collections::HashMap;

use crate::events::{Event, ScopeType};
use crate::message_bus::{Subscriber, SubscriberFactory, SubscriberResult};

/// A single name use within a scope.
#[derive(Debug, Clone)]
pub struct NameUse {
    pub node_id: String,
    pub name: String,
    pub line: usize,
    pub column: usize,
}

impl NameUse {
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "node_id": self.node_id,
            "name": self.name,
            "line": self.line,
            "column": self.column,
        })
    }
}

/// Subscriber that collects name uses organized by scope.
///
/// Tracks the current scope using `EnterScope`/`ExitScope` events and
/// collects `UseName` events into lists keyed by scope qualname.
pub struct UsesSubscriber {
    name: String,
    /// Map from scope qualname to list of name uses in that scope
    uses_by_scope: HashMap<String, Vec<NameUse>>,
    /// Current scope stack (qualnames)
    scope_stack: Vec<String>,
    /// Module qualname (derived from first scope event)
    module_qualname: Option<String>,
}

impl UsesSubscriber {
    pub fn new(name: &str) -> Self {
        UsesSubscriber {
            name: name.to_string(),
            uses_by_scope: HashMap::new(),
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

    fn handle_enter_scope(&mut self, _scope_type: &ScopeType, name: &str, qualname: &str) {
        // Initialize module qualname from first scope event
        let is_first_scope = self.module_qualname.is_none();
        if is_first_scope {
            let module_qualname = Self::derive_module_qualname(qualname, name);
            self.module_qualname = Some(module_qualname.clone());

            // Move any pending uses to the module scope
            if let Some(pending) = self.uses_by_scope.remove("<pending>") {
                self.uses_by_scope
                    .entry(module_qualname)
                    .or_default()
                    .extend(pending);
            }
        }

        // Push this scope onto the stack
        self.scope_stack.push(qualname.to_string());

        // Ensure there's an entry for this scope (even if empty)
        self.uses_by_scope
            .entry(qualname.to_string())
            .or_default();
    }

    fn handle_exit_scope(&mut self) {
        self.scope_stack.pop();
    }

    fn handle_use_name(&mut self, name: &str, node_id: &str, line: usize, column: usize) {
        // Get the current scope - if we don't have module context yet,
        // store in a temporary "pending" list
        let Some(current_scope) = self.current_scope() else {
            self.uses_by_scope
                .entry("<pending>".to_string())
                .or_default()
                .push(NameUse {
                    node_id: node_id.to_string(),
                    name: name.to_string(),
                    line,
                    column,
                });
            return;
        };

        let name_use = NameUse {
            node_id: node_id.to_string(),
            name: name.to_string(),
            line,
            column,
        };

        self.uses_by_scope
            .entry(current_scope)
            .or_default()
            .push(name_use);
    }
}

impl Subscriber for UsesSubscriber {
    fn name(&self) -> &str {
        &self.name
    }

    fn handle_event(&mut self, event: &Event) -> Result<(), String> {
        match event {
            Event::EnterScope {
                scope_type,
                name,
                qualname,
                ..
            } => {
                self.handle_enter_scope(scope_type, name, qualname);
            }
            Event::ExitScope { .. } => {
                self.handle_exit_scope();
            }
            Event::UseName {
                name,
                node_id,
                line,
                column,
                ..
            } => {
                self.handle_use_name(name, node_id, *line, *column);
            }
            _ => {}
        }
        Ok(())
    }

    fn finalize(&mut self) -> Result<SubscriberResult, String> {
        // Build JSON output
        let mut scopes_json = serde_json::Map::new();

        for (scope, uses) in &self.uses_by_scope {
            // Skip pending (should be empty after processing)
            if scope == "<pending>" {
                continue;
            }

            let uses_json: Vec<serde_json::Value> = uses.iter().map(|u| u.to_json()).collect();

            scopes_json.insert(scope.clone(), serde_json::Value::Array(uses_json));
        }

        Ok(SubscriberResult {
            subscriber_name: self.name.clone(),
            data: serde_json::Value::Object(scopes_json),
        })
    }
}

/// Factory for creating UsesSubscriber instances.
pub struct UsesSubscriberFactory {
    name: String,
}

impl UsesSubscriberFactory {
    pub fn new(name: &str) -> Self {
        UsesSubscriberFactory {
            name: name.to_string(),
        }
    }
}

impl SubscriberFactory for UsesSubscriberFactory {
    fn create(&self) -> Box<dyn Subscriber> {
        Box::new(UsesSubscriber::new(&self.name))
    }
}
