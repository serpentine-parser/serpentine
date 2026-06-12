use std::collections::HashMap;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ScopeNode {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub qualname: String,
    #[serde(default)]
    pub scope_type: String,
    #[serde(default)]
    pub bases: Vec<String>,
    #[serde(default)]
    pub parameters: Vec<String>,
    #[serde(default)]
    pub children: Vec<ScopeNode>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ScopeTree {
    #[serde(default)]
    pub files: Vec<ScopeNode>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct DefinitionEntry {
    #[serde(default)]
    pub qualname: String,
    #[serde(rename = "type", default)]
    pub def_type: String,
    #[serde(default)]
    pub line: usize,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Definitions {
    #[serde(default)]
    pub definitions_by_scope: HashMap<String, Vec<DefinitionEntry>>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UseEntry {
    #[serde(default)]
    pub name: String,
}

pub type Uses = HashMap<String, Vec<UseEntry>>;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ImportEntry {
    #[serde(default)]
    pub file: String,
    #[serde(default)]
    pub source_module: String,
    #[serde(default)]
    pub imported_names: Option<Vec<String>>,
    #[serde(default)]
    pub aliases: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Imports {
    #[serde(default)]
    pub imports: Vec<ImportEntry>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct DecoratorEntry {
    #[serde(default)]
    pub decorated_fn: String,
    #[serde(default)]
    pub root: String,
    #[serde(default)]
    pub is_attribute: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Decorators {
    #[serde(default)]
    pub decorators: Vec<DecoratorEntry>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct RawBindingSource {
    #[serde(default)]
    pub qualname: String,
    #[serde(default)]
    pub text: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct RawBindingTarget {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub arguments: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct RawBinding {
    #[serde(default)]
    pub relationship: String,
    #[serde(default)]
    pub scope: String,
    #[serde(default)]
    pub source: RawBindingSource,
    #[serde(default)]
    pub target: RawBindingTarget,
}

pub struct FileSubscriberData {
    pub scope_tree: ScopeTree,
    pub definitions: Definitions,
    pub uses: Uses,
    pub imports: Imports,
    pub decorators: Decorators,
    pub raw_bindings: Vec<RawBinding>,
    /// PDG and code_snippet kept as Value — already stored as Value in NodeData.
    pub pdg: serde_json::Value,
    pub code_snippet: serde_json::Value,
}

impl Default for FileSubscriberData {
    fn default() -> Self {
        FileSubscriberData {
            scope_tree: ScopeTree::default(),
            definitions: Definitions::default(),
            uses: Uses::default(),
            imports: Imports::default(),
            decorators: Decorators::default(),
            raw_bindings: Vec::new(),
            pdg: serde_json::Value::Null,
            code_snippet: serde_json::Value::Null,
        }
    }
}
