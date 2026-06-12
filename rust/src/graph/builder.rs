use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::{DependencyGraph, EdgeData, GraphMetadata, LanguageConfig, NodeData};

/// Per-file ownership record for incremental graph updates.
#[derive(Default)]
pub(crate) struct FileContributions {
    /// Qualnames that this file's load passes added to `definitions`.
    pub(crate) node_ids: HashSet<String>,
    /// Edges from per-file load passes (uses, decorators, imports, inheritance).
    /// Does NOT include raw_binding edges (tracked separately on GraphBuilder).
    pub(crate) edge_keys: HashSet<EdgeData>,
    /// Phantom keys this file added to `reexport_map`.
    pub(crate) reexport_keys: Vec<String>,
    /// Keys this file added to `import_bindings`.
    pub(crate) import_binding_keys: Vec<String>,
    /// Raw binding array from this file's subscriber output, stored for
    /// global re-merge when any file is dirtied.
    pub(crate) raw_bindings: Vec<Value>,
}

/// Builder that combines subscriber outputs into a DependencyGraph
pub struct GraphBuilder {
    /// Local module top-level prefixes (e.g., {"src", "frontend"})
    pub(crate) local_prefixes: HashSet<String>,
    /// Language configs used for stdlib classification
    pub(crate) lang_configs: Vec<Box<dyn LanguageConfig>>,
    /// All known definitions indexed by qualname (only real definitions!)
    pub(crate) definitions: HashMap<String, NodeData>,
    /// Dependency edges from per-file load passes (uses, imports, decorators, inheritance).
    pub(crate) edges: HashSet<EdgeData>,
    /// Edges produced by the global load_raw_bindings pass, tracked separately
    /// so they can be cleared and re-run incrementally without disturbing per-file edges.
    pub(crate) raw_binding_edges: HashSet<EdgeData>,
    /// Re-export map: phantom qualname → actual definition qualname.
    /// Built from __init__.py imports so that `pkg.name` resolves to
    /// `pkg.submodule.name` when the symbol is re-exported through __init__.
    pub(crate) reexport_map: HashMap<String, String>,
    /// Import bindings: maps "module.local_name" → "resolved_qualname"
    /// e.g., "serpentine.state.Config" → "serpentine.config.Config"
    ///
    /// Built from scope_tree + definitions + imports subscriber outputs
    /// BEFORE any edge resolution. Used by resolve_name_legb() to follow
    /// what each name means in each module's namespace, just like Python's
    /// LEGB rule at the G (global/module) level.
    pub(crate) import_bindings: HashMap<String, String>,
    /// Edge index keyed by caller — built before CALLS pass of load_raw_bindings.
    /// Enables O(1) lookup in resolve_variable_type instead of O(E) scan.
    pub(crate) edge_caller_index: HashMap<String, Vec<EdgeData>>,
    /// Function return types: fn_qualname → class_qualname.
    /// Populated by the RETURNS pass in load_raw_bindings.
    /// Used by resolve_variable_type to resolve factory-function return types.
    pub(crate) function_return_types: HashMap<String, String>,
    /// All module qualnames for O(1) lookup in build_reexport_map,
    /// replacing the O(n) `.any(|k| k.starts_with(...))` scan.
    pub(crate) module_qualnames: HashSet<String>,
    /// Per-file ownership records for incremental updates.
    pub(crate) file_contributions: HashMap<PathBuf, FileContributions>,
    /// Files modified since the last graph build. Processed on next call.
    pub(crate) dirty_files: HashSet<PathBuf>,
    /// File currently being loaded. Set before each load call, cleared after.
    /// Used by load_* methods to attribute contributions without extra parameters.
    pub(crate) current_file: Option<PathBuf>,
    /// Cached hierarchical nodes from the last build. None when rebuild needed.
    pub(crate) cached_hierarchy: Option<Vec<NodeData>>,
}

impl Default for GraphBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphBuilder {
    pub fn new() -> Self {
        GraphBuilder {
            local_prefixes: HashSet::new(),
            lang_configs: Vec::new(),
            definitions: HashMap::new(),
            edges: HashSet::new(),
            raw_binding_edges: HashSet::new(),
            reexport_map: HashMap::new(),
            import_bindings: HashMap::new(),
            edge_caller_index: HashMap::new(),
            function_return_types: HashMap::new(),
            module_qualnames: HashSet::new(),
            file_contributions: HashMap::new(),
            dirty_files: HashSet::new(),
            current_file: None,
            cached_hierarchy: None,
        }
    }

    /// Record a node qualname as contributed by `current_file`.
    pub(crate) fn record_node_contribution(&mut self, qualname: &str) {
        if let Some(path) = self.current_file.clone() {
            self.file_contributions
                .entry(path)
                .or_default()
                .node_ids
                .insert(qualname.to_string());
        }
    }

    /// Record an edge as contributed by `current_file`.
    pub(crate) fn record_edge_contribution(&mut self, edge: EdgeData) {
        if let Some(path) = self.current_file.clone() {
            self.file_contributions
                .entry(path)
                .or_default()
                .edge_keys
                .insert(edge);
        }
    }

    /// Record a reexport_map key as contributed by `current_file`.
    pub(crate) fn record_reexport_contribution(&mut self, key: &str) {
        if let Some(path) = self.current_file.clone() {
            self.file_contributions
                .entry(path)
                .or_default()
                .reexport_keys
                .push(key.to_string());
        }
    }

    /// Record an import_bindings key as contributed by `current_file`.
    pub(crate) fn record_import_binding_contribution(&mut self, key: &str) {
        if let Some(path) = self.current_file.clone() {
            self.file_contributions
                .entry(path)
                .or_default()
                .import_binding_keys
                .push(key.to_string());
        }
    }

    /// Build the edge_caller_index from current edges. Call before the CALLS
    /// pass of load_raw_bindings so resolve_variable_type gets O(1) lookups.
    pub(crate) fn build_edge_caller_index(&mut self) {
        self.edge_caller_index.clear();
        let all_edges: Vec<EdgeData> = self.edges.iter().chain(self.raw_binding_edges.iter()).cloned().collect();
        for edge in all_edges {
            self.edge_caller_index
                .entry(edge.caller.clone())
                .or_default()
                .push(edge);
        }
    }

    /// Remove all raw_binding-derived edges from `self.edges` and clear the
    /// tracking set. Call before re-running `load_raw_bindings` globally.
    pub fn clear_raw_binding_edges(&mut self) {
        for edge in self.raw_binding_edges.drain() {
            self.edges.remove(&edge);
        }
    }

    /// Remove all contributions from `path` and retract their effects on graph state.
    pub fn retract_file(&mut self, path: &Path) {
        let Some(contrib) = self.file_contributions.remove(path) else {
            return;
        };
        for qualname in &contrib.node_ids {
            self.definitions.remove(qualname);
            self.module_qualnames.remove(qualname);
        }
        for edge in &contrib.edge_keys {
            self.edges.remove(edge);
        }
        for key in &contrib.reexport_keys {
            self.reexport_map.remove(key);
        }
        for key in &contrib.import_binding_keys {
            self.import_bindings.remove(key);
        }
        // raw_bindings removed with the FileContributions entry above.
        // Caller is responsible for calling clear_raw_binding_edges() and
        // re-running load_raw_bindings() globally after all retractions.
    }

    /// Filter out less-specific edges if more specific edges exist
    /// For example, if test_package.app.main -> math.sqrt (Calls) exists,
    /// remove test_package.app -> math (has-a)
    fn deduplicate_edges_set(edges: &mut HashSet<EdgeData>) {
        let edges_vec: Vec<EdgeData> = edges.iter().cloned().collect();
        let mut to_remove = HashSet::new();

        // Build: top_module → set of callers that reference it (any edge type)
        // This lets us check O(1) whether a descendant of a has-a edge's caller
        // already has a more-specific edge to the same top-level module.
        let mut module_callers: HashMap<String, HashSet<String>> = HashMap::new();
        for edge in &edges_vec {
            let top = edge.callee.split('.').next().unwrap_or(&edge.callee).to_string();
            module_callers.entry(top).or_default().insert(edge.caller.clone());
        }

        for edge in &edges_vec {
            if edge.edge_type != "has-a" {
                continue;
            }
            let top = edge.callee.split('.').next().unwrap_or(&edge.callee);
            let prefix = format!("{}.", edge.caller);
            if let Some(callers) = module_callers.get(top) {
                if callers.iter().any(|c| c.starts_with(&prefix)) {
                    to_remove.insert(edge.clone());
                }
            }
        }

        for edge in to_remove {
            edges.remove(&edge);
        }
    }

    /// Produce a graph snapshot from current state without consuming the builder.
    /// Clones edges and definitions, runs dedup/filter, and builds the hierarchy.
    pub fn snapshot(&self) -> DependencyGraph {
        let mut edges: HashSet<EdgeData> = self.edges.clone();
        edges.extend(self.raw_binding_edges.iter().cloned());

        Self::deduplicate_edges_set(&mut edges);

        let edges: Vec<EdgeData> = edges
            .into_iter()
            .filter(|edge| {
                !edge.callee.starts_with(&format!("{}.", edge.caller))
                    && !edge.caller.starts_with(&format!("{}.", edge.callee))
            })
            .collect();

        let root_nodes = Self::build_hierarchy(self.definitions.clone());

        let mut graph = DependencyGraph {
            nodes: root_nodes,
            edges,
            metadata: GraphMetadata::default(),
        };
        graph.compute_metadata();
        graph
    }

    /// Build hierarchical node structure from flat definitions map
    pub(crate) fn build_hierarchy(definitions: HashMap<String, NodeData>) -> Vec<NodeData> {
        let mut definitions = definitions;

        // Sort qualnames by depth (parents before children)
        let mut qualnames: Vec<String> = definitions.keys().cloned().collect();
        qualnames.sort_by(|a, b| {
            let depth_a = a.matches('.').count();
            let depth_b = b.matches('.').count();
            depth_a.cmp(&depth_b).then_with(|| a.cmp(b))
        });

        // Track which nodes have been added as children
        let mut added_as_child: HashSet<String> = HashSet::new();
        let mut root_nodes: Vec<NodeData> = Vec::new();

        // First pass: identify which nodes should be children
        for qualname in &qualnames {
            if let Some((parent_qualname, _)) = qualname.rsplit_once('.') {
                if definitions.contains_key(parent_qualname) {
                    added_as_child.insert(qualname.clone());
                }
            }
        }

        // Second pass: add children to parents (process deepest first)
        for qualname in qualnames.iter().rev() {
            if added_as_child.contains(qualname) {
                if let Some((parent_qualname, _)) = qualname.rsplit_once('.') {
                    if let Some(child_node) = definitions.remove(qualname) {
                        if let Some(parent_node) = definitions.get_mut(parent_qualname) {
                            parent_node.children.push(child_node);
                        }
                    }
                }
            }
        }

        // Collect remaining root nodes
        for qualname in &qualnames {
            if !added_as_child.contains(qualname) {
                if let Some(node) = definitions.remove(qualname) {
                    root_nodes.push(node);
                }
            }
        }

        root_nodes
    }
}
