use std::collections::{HashMap, HashSet};

use super::{DependencyGraph, EdgeData, GraphMetadata, LanguageConfig, NodeData};

/// Builder that combines subscriber outputs into a DependencyGraph
pub struct GraphBuilder {
    /// Local module top-level prefixes (e.g., {"src", "frontend"})
    pub(crate) local_prefixes: HashSet<String>,
    /// Language configs used for stdlib classification
    pub(crate) lang_configs: Vec<Box<dyn LanguageConfig>>,
    /// All known definitions indexed by qualname (only real definitions!)
    pub(crate) definitions: HashMap<String, NodeData>,
    /// Dependency edges (deduplicated)
    pub(crate) edges: HashSet<EdgeData>,
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
            reexport_map: HashMap::new(),
            import_bindings: HashMap::new(),
            edge_caller_index: HashMap::new(),
            function_return_types: HashMap::new(),
        }
    }

    /// Build the edge_caller_index from current edges. Call before the CALLS
    /// pass of load_raw_bindings so resolve_variable_type gets O(1) lookups.
    pub(crate) fn build_edge_caller_index(&mut self) {
        self.edge_caller_index.clear();
        for edge in &self.edges {
            self.edge_caller_index
                .entry(edge.caller.clone())
                .or_default()
                .push(edge.clone());
        }
    }

    /// Filter out less-specific edges if more specific edges exist
    /// For example, if test_package.app.main -> math.sqrt (Calls) exists,
    /// remove test_package.app -> math (has-a)
    fn deduplicate_edges(&mut self) {
        let edges_vec: Vec<EdgeData> = self.edges.iter().cloned().collect();
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

        // Remove less-specific edges
        for edge in to_remove {
            self.edges.remove(&edge);
        }
    }

    /// Build the final graph from accumulated data
    pub fn build(self) -> DependencyGraph {
        let mut builder = self;

        // Deduplicate edges before filtering
        builder.deduplicate_edges();

        // Extract edges, filtering out parent->child and child->parent relationships
        // (the hierarchy already shows containment)
        let edges: Vec<EdgeData> = builder
            .edges
            .into_iter()
            .filter(|edge| {
                // Remove edges where one node is an ancestor/descendant of the other
                !edge.callee.starts_with(&format!("{}.", edge.caller))
                    && !edge.caller.starts_with(&format!("{}.", edge.callee))
            })
            .collect();

        // Build hierarchical node structure
        let root_nodes = Self::build_hierarchy(builder.definitions);

        // Create final graph
        let mut graph = DependencyGraph {
            nodes: root_nodes,
            edges,
            metadata: GraphMetadata::default(),
        };

        graph.compute_metadata();
        graph
    }

    /// Build hierarchical node structure from flat definitions map
    fn build_hierarchy(definitions: HashMap<String, NodeData>) -> Vec<NodeData> {
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
