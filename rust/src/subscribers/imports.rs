//! Imports subscriber - collects import statements for dependency resolution.
//!
//! This subscriber listens to `ImportStatement` events to collect all imports
//! organized by the file they appear in. This information is used by the graph
//! builder to create dependency edges between modules.

use std::collections::HashMap;

use crate::events::Event;
use crate::message_bus::{Subscriber, SubscriberFactory, SubscriberResult};

/// A single import statement.
#[derive(Debug, Clone)]
pub struct ImportInfo {
    /// The source module being imported from
    /// For `import os` -> "os"
    /// For `from typing import List` -> "typing"
    /// For `from .models import Foo` -> resolved absolute path
    pub source_module: String,
    /// Names imported from the source module
    /// For `import os` -> []
    /// For `from typing import List, Dict` -> ["List", "Dict"]
    /// For `from typing import *` -> ["*"]
    pub imported_names: Vec<String>,
    /// Alias mapping: original_name → local_alias
    /// For `from x import Foo as Bar` -> {"Foo": "Bar"}
    /// For `import os as operating_system` -> {"os": "operating_system"}
    pub aliases: std::collections::HashMap<String, String>,
    /// Whether this import is inside an `if TYPE_CHECKING:` block
    pub is_type_checking: bool,
    /// Line number of the import
    pub line: usize,
    /// The file containing this import
    pub file: String,
}

impl ImportInfo {
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "source_module": self.source_module,
            "imported_names": self.imported_names,
            "aliases": self.aliases,
            "is_type_checking": self.is_type_checking,
            "line": self.line,
            "file": self.file,
        })
    }
}

/// Subscriber that collects import statements.
pub struct ImportsSubscriber {
    name: String,
    /// Map from file path to list of imports in that file
    imports_by_file: HashMap<String, Vec<ImportInfo>>,
}

impl ImportsSubscriber {
    pub fn new(name: &str) -> Self {
        ImportsSubscriber {
            name: name.to_string(),
            imports_by_file: HashMap::new(),
        }
    }

    fn handle_import_statement(
        &mut self,
        module: &str,
        names: &[String],
        aliases: std::collections::HashMap<String, String>,
        is_type_checking: bool,
        file: &str,
        line: usize,
    ) {
        let import_info = ImportInfo {
            source_module: module.to_string(),
            imported_names: names.to_vec(),
            aliases,
            is_type_checking,
            line,
            file: file.to_string(),
        };

        self.imports_by_file
            .entry(file.to_string())
            .or_default()
            .push(import_info);
    }
}

impl Subscriber for ImportsSubscriber {
    fn handle_event(&mut self, event: &Event) -> Result<(), String> {
        if let Event::ImportStatement {
            module,
            names,
            aliases,
            is_type_checking,
            file,
            line,
            ..
        } = event
        {
            self.handle_import_statement(module, names, aliases.clone(), *is_type_checking, file, *line);
        }
        Ok(())
    }

    fn finalize(&mut self) -> Result<SubscriberResult, String> {
        // Convert to a flat list of all imports
        let all_imports: Vec<serde_json::Value> = self
            .imports_by_file
            .values()
            .flatten()
            .map(|i| i.to_json())
            .collect();

        // Also organize by file for convenience
        let by_file: serde_json::Map<String, serde_json::Value> = self
            .imports_by_file
            .iter()
            .map(|(file, imports)| {
                let imports_json: serde_json::Value = imports
                    .iter()
                    .map(|i| i.to_json())
                    .collect::<Vec<_>>()
                    .into();
                (file.clone(), imports_json)
            })
            .collect();

        Ok(SubscriberResult {
            subscriber_name: self.name.clone(),
            data: serde_json::json!({
                "imports": all_imports,
                "imports_by_file": by_file,
            }),
        })
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Factory for creating `ImportsSubscriber` instances.
pub struct ImportsSubscriberFactory {
    name: String,
}

impl ImportsSubscriberFactory {
    pub fn new(name: &str) -> Self {
        ImportsSubscriberFactory {
            name: name.to_string(),
        }
    }
}

impl SubscriberFactory for ImportsSubscriberFactory {
    fn create(&self) -> Box<dyn Subscriber> {
        Box::new(ImportsSubscriber::new(&self.name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_import_collection() {
        let mut subscriber = ImportsSubscriber::new("imports");

        // Simulate `import os`
        subscriber.handle_import_statement("os", &[], std::collections::HashMap::new(), false, "test.py", 1);

        // Simulate `from typing import List, Dict`
        subscriber.handle_import_statement(
            "typing",
            &["List".to_string(), "Dict".to_string()],
            std::collections::HashMap::new(),
            false,
            "test.py",
            2,
        );

        let result = subscriber.finalize().unwrap();
        let imports = result.data.get("imports").unwrap().as_array().unwrap();
        assert_eq!(imports.len(), 2);

        assert_eq!(imports[0]["source_module"], "os");
        assert!(imports[0]["imported_names"].as_array().unwrap().is_empty());

        assert_eq!(imports[1]["source_module"], "typing");
        assert_eq!(imports[1]["imported_names"][0], "List");
        assert_eq!(imports[1]["imported_names"][1], "Dict");
    }
}
