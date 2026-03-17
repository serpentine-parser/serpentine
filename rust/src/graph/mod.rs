//! Shared graph module — language-agnostic dependency graph data structures and builder.
//!
//! This module takes the output from all subscribers (definitions, uses, scope_tree, raw_bindings)
//! and resolves them into a single `DependencyGraph` structure suitable for visualization.
//!
//! Key principles:
//! - Nodes are ONLY created for actual definitions (modules, classes, functions, important variables)
//! - Edges are created by resolving raw bindings to actual definition qualnames
//! - Call expressions like "Car(eng)" are resolved to the class/function being called
//! - Language-specific behaviour is pluggable via the `LanguageConfig` trait (Phase 3)

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

mod builder;
mod pdg;
mod loaders;
mod resolvers;

pub use builder::GraphBuilder;

// ============================================================================
// Data Structures (matching serpentine Python models)
// ============================================================================

/// Origin classification for modules
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Origin {
    Local,
    Standard,
    #[serde(rename = "third-party")]
    ThirdParty,
}

/// Object type classification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ObjectType {
    Module,
    Class,
    Function,
    Assignment,
    /// Structural type contract — TypeScript interface/object-shape type, Rust trait, etc.
    Interface,
    Unknown,
}

impl From<&str> for ObjectType {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "module" => ObjectType::Module,
            "class" => ObjectType::Class,
            "function" => ObjectType::Function,
            "assignment" | "variable" => ObjectType::Assignment,
            "interface" => ObjectType::Interface,
            _ => ObjectType::Unknown,
        }
    }
}

/// A node in the dependency graph (matches serpentine's NodeData)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeData {
    pub id: String,
    pub name: String,
    pub object_type: ObjectType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<(usize, usize)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docstring: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_block: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<Origin>,
    #[serde(default)]
    pub children: Vec<NodeData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pdg: Option<serde_json::Value>,
    /// Function parameter names (excluding self/cls). Used by CONSTRUCTOR-ARG pass.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub parameters: Vec<String>,
}

impl NodeData {
    pub fn new(name: &str, object_type: ObjectType) -> Self {
        NodeData {
            id: name.to_string(),
            name: name.to_string(),
            object_type,
            position: Some((0, 0)),
            docstring: None,
            code_block: None,
            content_hash: None,
            file_path: None,
            origin: None,
            children: Vec::new(),
            pdg: None,
            parameters: Vec::new(),
        }
    }
}

/// An edge in the dependency graph (matches serpentine's EdgeData)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct EdgeData {
    pub caller: String,
    pub callee: String,
    #[serde(rename = "type")]
    pub edge_type: String,
}

impl EdgeData {
    pub fn new(caller: &str, callee: &str, edge_type: &str) -> Self {
        EdgeData {
            caller: caller.to_string(),
            callee: callee.to_string(),
            edge_type: edge_type.to_string(),
        }
    }
}

/// Graph metadata
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphMetadata {
    pub node_count: usize,
    pub edge_count: usize,
    pub node_types: HashMap<String, usize>,
}

/// The complete dependency graph (matches serpentine's GraphData)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyGraph {
    pub nodes: Vec<NodeData>,
    pub edges: Vec<EdgeData>,
    pub metadata: GraphMetadata,
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl DependencyGraph {
    pub fn new() -> Self {
        DependencyGraph {
            nodes: Vec::new(),
            edges: Vec::new(),
            metadata: GraphMetadata::default(),
        }
    }

    pub fn compute_metadata(&mut self) {
        self.metadata.node_count = self.count_nodes(&self.nodes);
        self.metadata.edge_count = self.edges.len();

        let mut type_counts: HashMap<String, usize> = HashMap::new();
        self.count_node_types(&self.nodes, &mut type_counts);
        self.metadata.node_types = type_counts;
    }

    fn count_nodes(&self, nodes: &[NodeData]) -> usize {
        nodes
            .iter()
            .map(|n| 1 + self.count_nodes(&n.children))
            .sum()
    }

    fn count_node_types(&self, nodes: &[NodeData], counts: &mut HashMap<String, usize>) {
        for node in nodes {
            let type_str = format!("{:?}", node.object_type).to_lowercase();
            *counts.entry(type_str).or_insert(0) += 1;
            self.count_node_types(&node.children, counts);
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }
}

// ============================================================================
// Language Configuration Trait
// ============================================================================

/// Language-specific configuration for graph building.
///
/// Implementors provide language-specific knowledge for module path derivation,
/// re-export detection, and stdlib/third-party classification.
/// Language configs are plugged in during Phase 3; until then, the builder
/// retains its current Python-specific internal methods.
pub trait LanguageConfig: Send + Sync {
    /// Derive the logical module qualname from a file path and project root.
    /// Python: "src/pkg/mod.py" → "pkg.mod"
    /// JS: "src/components/Button.tsx" → "components/Button"
    fn derive_module_path(&self, file_path: &str, project_root: &str) -> String;

    /// Whether this file acts as a re-export hub (Python: __init__.py, JS: index.ts).
    fn is_reexport_file(&self, file_path: &str) -> bool;

    /// Whether a module name refers to a language stdlib or built-in.
    fn is_stdlib(&self, module: &str) -> bool;

    /// Whether a module name refers to a third-party package (not local).
    fn is_third_party(&self, module: &str) -> bool;

    /// File extensions handled by this language config.
    fn extensions(&self) -> &[&str];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_origin_classification() {
        struct TestConfig;
        impl LanguageConfig for TestConfig {
            fn derive_module_path(&self, f: &str, _: &str) -> String { f.to_string() }
            fn is_reexport_file(&self, _: &str) -> bool { false }
            fn is_stdlib(&self, module: &str) -> bool { matches!(module, "os" | "math") }
            fn is_third_party(&self, module: &str) -> bool { !self.is_stdlib(module) }
            fn extensions(&self) -> &[&str] { &[] }
        }

        let mut builder = GraphBuilder::new();
        builder.local_prefixes = vec!["myproject".to_string()];
        builder.lang_configs = vec![Box::new(TestConfig)];

        assert_eq!(builder.classify_module("myproject.app"), Origin::Local);
        assert_eq!(builder.classify_module("os"), Origin::Standard);
        assert_eq!(builder.classify_module("math"), Origin::Standard);
        assert_eq!(builder.classify_module("requests"), Origin::ThirdParty);
        assert_eq!(builder.classify_module("numpy"), Origin::ThirdParty);
    }

    #[test]
    fn test_extract_callable() {
        let builder = GraphBuilder::new();
        assert_eq!(builder.extract_callable("Car(eng)"), "Car");
        assert_eq!(builder.extract_callable("Engine()"), "Engine");
        assert_eq!(builder.extract_callable("math.sqrt(25)"), "math.sqrt");
    }
}
