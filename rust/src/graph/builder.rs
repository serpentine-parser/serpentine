use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use indexmap::{IndexMap, IndexSet};

use super::{EdgeData, GraphMetadata, LanguageConfig, NodeData};
use crate::subscribers::RawBinding;

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
    /// Typed raw bindings from this file's subscriber output, stored for
    /// global re-merge when any file is dirtied.
    pub(crate) raw_bindings: Vec<RawBinding>,
    /// FNV-like hash of this file's raw_bindings used to detect unchanged
    /// bindings on incremental builds and skip the global re-run.
    pub(crate) raw_bindings_hash: u64,
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
    /// Cached JSON snapshot from the last completed build. Cleared when any
    /// file is retracted (i.e., on the first dirty-file build after a change).
    pub(crate) cached_snapshot: Option<String>,
    /// Incremental hierarchy: parent qualname → ordered child qualnames.
    /// Updated on every definitions insert and cleared on retract_file.
    pub(crate) hierarchy_children: IndexMap<String, Vec<String>>,
    /// Root qualnames (no parent in definitions), in insertion order.
    pub(crate) hierarchy_roots: IndexSet<String>,
    /// Per-node JSON fragment cache (structural fields only, excluding children).
    /// Entries are invalidated in retract_file when a node is removed.
    /// snapshot() takes &mut self so no Mutex is needed.
    pub(crate) node_json_cache: HashMap<String, String>,
    /// Per-subtree JSON cache: top-level module qualname → full subtree JSON string.
    /// A subtree is dirty (and absent here) when any node within it was retracted
    /// or re-added since the last snapshot. Clean subtrees are emitted directly
    /// without re-traversing the node hierarchy.
    pub(crate) subtree_json_cache: HashMap<String, String>,
    /// Top-level module qualnames whose subtree JSON must be rebuilt on next snapshot.
    /// Populated by retract_file and register_node_in_hierarchy; cleared after snapshot.
    pub(crate) dirty_subtrees: HashSet<String>,
    /// Cached serialized edges JSON (the "edges" section of the snapshot).
    /// Invalidated when raw_binding_edges change (i.e., when clear_raw_binding_edges
    /// is called). Reused when raw_bindings haven't changed on an incremental build.
    pub(crate) cached_edges_json: Option<String>,
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
            cached_snapshot: None,
            hierarchy_children: IndexMap::new(),
            hierarchy_roots: IndexSet::new(),
            node_json_cache: HashMap::new(),
            subtree_json_cache: HashMap::new(),
            dirty_subtrees: HashSet::new(),
            cached_edges_json: None,
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

    /// Register a newly-inserted node in the incremental hierarchy.
    /// Must be called AFTER the node's ancestors exist in `self.definitions`
    /// (i.e., after `ensure_parent_nodes` returns).
    pub(crate) fn register_node_in_hierarchy(&mut self, qualname: &str) {
        // Mark the top-level subtree as dirty so snapshot rebuilds it
        let root = qualname.split('.').next().unwrap_or(qualname);
        self.dirty_subtrees.insert(root.to_string());
        self.subtree_json_cache.remove(root);

        if let Some((parent, _)) = qualname.rsplit_once('.') {
            if self.definitions.contains_key(parent) {
                let children = self.hierarchy_children.entry(parent.to_string()).or_default();
                if !children.iter().any(|c| c == qualname) {
                    children.push(qualname.to_string());
                }
                return;
            }
        }
        self.hierarchy_roots.insert(qualname.to_string());
    }

    /// Build the edge_caller_index from current edges. Call before the CALLS
    /// pass of load_raw_bindings so resolve_variable_type gets O(1) lookups.
    pub(crate) fn build_edge_caller_index(&mut self) {
        self.edge_caller_index.clear();
        // Only has-a edges are ever queried from this index (resolve_variable_type,
        // constructor-arg, param-type passes all filter for edge_type == "has-a").
        // Indexing only has-a reduces the index by ~10-20× vs all edge types.
        for edge in self.edges.iter().chain(self.raw_binding_edges.iter()) {
            if edge.edge_type == "has-a" {
                self.edge_caller_index
                    .entry(edge.caller.clone())
                    .or_default()
                    .push(edge.clone());
            }
        }
    }

    /// Remove all raw_binding-derived edges from `self.edges` and clear the
    /// tracking set. Call before re-running `load_raw_bindings` globally.
    pub fn clear_raw_binding_edges(&mut self) {
        for edge in self.raw_binding_edges.drain() {
            self.edges.remove(&edge);
        }
        self.cached_edges_json = None;
    }

    /// Remove all contributions from `path` and retract their effects on graph state.
    pub fn retract_file(&mut self, path: &Path) {
        self.cached_snapshot = None;
        let Some(contrib) = self.file_contributions.remove(path) else {
            return;
        };
        for qualname in &contrib.node_ids {
            self.definitions.remove(qualname);
            self.module_qualnames.remove(qualname);
            // Remove from incremental hierarchy
            self.hierarchy_roots.shift_remove(qualname.as_str());
            if let Some((parent, _)) = qualname.rsplit_once('.') {
                if let Some(children) = self.hierarchy_children.get_mut(parent) {
                    children.retain(|c| c != qualname.as_str());
                }
            }
            self.hierarchy_children.shift_remove(qualname.as_str());
            // Invalidate the per-node JSON fragment cache entry
            self.node_json_cache.remove(qualname.as_str());
            // Mark the top-level subtree dirty so snapshot rebuilds it
            let root = qualname.split('.').next().unwrap_or(qualname);
            self.dirty_subtrees.insert(root.to_string());
            self.subtree_json_cache.remove(root);
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

    /// True if one endpoint is a direct ancestor of the other in the dotted hierarchy.
    /// Replaces `starts_with(&format!("{}.", x))` with a zero-allocation check.
    #[inline]
    fn is_ancestor_edge(e: &EdgeData) -> bool {
        fn one_is_ancestor(parent: &str, child: &str) -> bool {
            child.len() > parent.len()
                && child.as_bytes().get(parent.len()) == Some(&b'.')
                && child.starts_with(parent)
        }
        one_is_ancestor(&e.caller, &e.callee) || one_is_ancestor(&e.callee, &e.caller)
    }

    /// Return the set of has-a edges that are redundant because a more-specific
    /// edge from a descendant caller already covers the same top-level module.
    ///
    /// O(N log N): builds sorted caller lists per top-level module, then uses
    /// binary search to check for prefix matches — avoids the O(N×M) inner
    /// `any(starts_with)` scan of the previous implementation.
    ///
    /// Operates entirely on borrowed references; no EdgeData is cloned.
    fn find_redundant_edges<'a>(all_edges: &[&'a EdgeData]) -> HashSet<&'a EdgeData> {
        // Build sorted caller list per top-level callee module
        let mut module_callers: HashMap<&str, Vec<&str>> = HashMap::new();
        for edge in all_edges {
            let top = edge.callee.split('.').next().unwrap_or(edge.callee.as_str());
            module_callers.entry(top).or_default().push(edge.caller.as_str());
        }
        for callers in module_callers.values_mut() {
            callers.sort_unstable();
            callers.dedup();
        }

        let mut redundant: HashSet<&EdgeData> = HashSet::new();
        for &edge in all_edges {
            if edge.edge_type != "has-a" {
                continue;
            }
            let top = edge.callee.split('.').next().unwrap_or(edge.callee.as_str());
            if let Some(callers) = module_callers.get(top) {
                let caller = edge.caller.as_str();
                // Binary search: find first entry that could start with "caller."
                // Strings starting with "caller." sort immediately after "caller" and
                // before any string that is lexicographically >= "caller" + char > '.'.
                let idx = callers.partition_point(|&c| c <= caller);
                // Check if the entry at idx starts with "caller." without allocating
                if callers.get(idx).is_some_and(|&c| {
                    c.len() > caller.len()
                        && c.as_bytes().get(caller.len()) == Some(&b'.')
                        && c.starts_with(caller)
                }) {
                    redundant.insert(edge);
                }
            }
        }
        redundant
    }

    /// Produce a graph snapshot JSON string from current state without consuming the builder.
    /// Uses the incremental hierarchy and per-node JSON fragment cache for fast incremental
    /// serialization: only nodes retracted since the last snapshot need re-serialization.
    pub fn snapshot(&mut self) -> String {
        // Build edge JSON (cached separately from nodes so it survives node-only changes)
        let edges_json = if let Some(ref cached) = self.cached_edges_json {
            cached.clone()
        } else {
            // Collect edge refs without cloning any EdgeData. Deduplicate by content
            // since self.edges and raw_binding_edges can contain value-equivalent items.
            let mut seen: HashSet<(&str, &str, &str)> = HashSet::new();
            let all_edges: Vec<&EdgeData> = self.edges.iter()
                .chain(self.raw_binding_edges.iter())
                .filter(|e| seen.insert((e.caller.as_str(), e.callee.as_str(), e.edge_type.as_str())))
                .collect();

            let redundant = Self::find_redundant_edges(&all_edges);
            let edges: Vec<&EdgeData> = all_edges.into_iter()
                .filter(|&e| !redundant.contains(e))
                .filter(|e| !Self::is_ancestor_edge(e))
                .collect();

            let j = serde_json::to_string(&edges).unwrap_or_default();
            self.cached_edges_json = Some(j.clone());
            j
        };

        let edge_count = {
            // Count from the JSON cheaply by counting top-level objects
            // (serde already produced it; for metadata just recount the filtered edges)
            // Re-derive from edges_json length would be wrong; just recompute edge count.
            // We already know the edges: use the cached_edges_json length is wrong.
            // Parse is too expensive. Store edge_count alongside cached_edges_json instead.
            // For now, count commas at top level — edges_json is `[{...},{...},...]`
            if edges_json == "[]" || edges_json == "null" { 0usize }
            else { edges_json.bytes().filter(|&b| b == b'{').count() }
        };

        let mut out = String::with_capacity(8 * 1024 * 1024);
        out.push_str("{\"nodes\":[");

        // Emit nodes using per-subtree cache. Dirty subtrees are rebuilt and re-cached;
        // clean subtrees are emitted directly from cache without any node traversal.
        let mut first_root = true;
        let roots: Vec<String> = self.hierarchy_roots.iter().cloned().collect();
        for root in &roots {
            if !self.definitions.contains_key(root.as_str()) {
                continue;
            }
            if !first_root { out.push(','); }
            first_root = false;

            if let Some(cached_subtree) = self.subtree_json_cache.get(root.as_str()) {
                // Clean subtree: emit cached JSON directly
                out.push_str(cached_subtree);
            } else {
                // Dirty subtree: rebuild and cache
                let subtree_start = out.len();
                self.write_node_json(&mut out, root);
                let subtree = out[subtree_start..].to_string();
                self.subtree_json_cache.insert(root.clone(), subtree);
            }
        }
        self.dirty_subtrees.clear();

        out.push_str("],\"edges\":");
        out.push_str(&edges_json);
        let metadata = self.compute_metadata_inline(edge_count);
        out.push_str(",\"metadata\":");
        out.push_str(&serde_json::to_string(&metadata).unwrap_or_default());
        out.push('}');
        out
    }

    /// Recursively write the JSON for a single node and its children into `out`.
    /// Uses `node_json_cache` to avoid re-serializing unchanged node fragments.
    fn write_node_json(&mut self, out: &mut String, qualname: &str) {
        // We need node data but can't hold a borrow of self.definitions while also
        // borrowing self.node_json_cache mutably. Serialize fresh if cache miss,
        // using a temporary clone of the node to avoid the borrow conflict.
        let fragment = if let Some(frag) = self.node_json_cache.get(qualname) {
            frag.clone()
        } else {
            let frag = self.definitions.get(qualname)
                .map(|n| serde_json::to_string(n).unwrap_or_default())
                .unwrap_or_default();
            if !frag.is_empty() {
                self.node_json_cache.insert(qualname.to_string(), frag.clone());
            }
            frag
        };

        if fragment.is_empty() { return; }

        // Strip the closing `}` and inject `"children":[...]`
        let base = fragment.trim_end_matches('}');
        out.push_str(base);
        out.push_str(",\"children\":[");
        let kids: Vec<String> = self.hierarchy_children
            .get(qualname).cloned()
            .unwrap_or_default();
        let mut first = true;
        for child in &kids {
            if self.definitions.contains_key(child.as_str()) {
                if !first { out.push(','); }
                first = false;
                self.write_node_json(out, child);
            }
        }
        out.push_str("]}");
    }

    /// Compute graph metadata directly from definitions (no DependencyGraph traversal).
    fn compute_metadata_inline(&self, edge_count: usize) -> GraphMetadata {
        let mut node_types: HashMap<String, usize> = HashMap::new();
        for node in self.definitions.values() {
            let type_str = format!("{:?}", node.object_type).to_lowercase();
            *node_types.entry(type_str).or_insert(0) += 1;
        }
        GraphMetadata {
            node_count: self.definitions.len(),
            edge_count,
            node_types,
        }
    }

}
