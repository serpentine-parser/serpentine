use std::collections::HashMap;

use serde_json::Value;

use super::{GraphBuilder, NodeData, ObjectType, EdgeData};

/// Parse comma-separated top-level args from "Foo(a, b, c)" → ["a", "b", "c"].
/// Does NOT handle nested parens — only works for simple identifier args.
fn extract_call_args(call_text: &str) -> Vec<String> {
    let open = match call_text.find('(') { Some(i) => i, None => return vec![] };
    let close = match call_text.rfind(')') { Some(i) => i, None => return vec![] };
    if close <= open + 1 { return vec![]; }
    call_text[open + 1..close]
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn extract_identifier_refs(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|s: &&str| !s.is_empty())
        .filter(|s: &&str| s.chars().next().map(|c| !c.is_ascii_digit()).unwrap_or(false))
        .map(String::from)
        .collect()
}

fn fnv1a_hash(s: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in s.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x00000100000001b3);
    }
    format!("{:016x}", hash)
}

impl GraphBuilder {
    /// Load and process scope_tree subscriber output
    /// This creates the module/class/function hierarchy
    pub fn load_scope_tree(&mut self, data: &Value) {
        if let Some(files) = data.get("files").and_then(|f| f.as_array()) {
            for file in files {
                self.process_scope_node(file, true);
            }
        }
    }

    /// Resolve and create is-a edges from class inheritance.
    ///
    /// Must be called AFTER `load_import_bindings` so that base class names can
    /// be resolved through LEGB (e.g., `class B(Base)` where `Base` is imported).
    pub fn resolve_inheritance_edges(&mut self, data: &Value) {
        if let Some(files) = data.get("files").and_then(|f| f.as_array()) {
            for file in files {
                self.collect_inheritance_edges_resolved(file);
            }
        }
    }

    fn collect_inheritance_edges_resolved(&mut self, node: &Value) {
        let scope_type = node.get("scope_type").and_then(|t| t.as_str()).unwrap_or("");
        let qualname = node.get("qualname").and_then(|q| q.as_str()).unwrap_or("");

        if scope_type == "class" && !qualname.is_empty() {
            // Derive the enclosing scope: strip the class name from its qualname
            let scope = qualname
                .rsplit_once('.')
                .map(|(parent, _)| parent)
                .unwrap_or(qualname);

            if let Some(bases) = node.get("bases").and_then(|b| b.as_array()) {
                for base in bases {
                    if let Some(base_name) = base.as_str() {
                        if base_name.is_empty() || base_name == "object" {
                            continue;
                        }
                        // Resolve base via LEGB so "A" → "mymod.A" and imported
                        // names like "Base" → "pkg.base.Base".
                        let resolved = self
                            .resolve_name_legb(scope, base_name)
                            .unwrap_or_else(|| base_name.to_string());
                        let top = resolved.split('.').next().unwrap_or(&resolved);
                        if !self.local_prefixes.contains(top) {
                            self.ensure_external_node(&resolved);
                        }
                        if self.definitions.contains_key(&resolved) {
                            let edge = EdgeData::new(qualname, &resolved, "is-a");
                            self.record_edge_contribution(edge.clone());
                            self.edges.insert(edge);
                        }
                    }
                }
            }
        }

        // Recurse into children
        if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
            for child in children {
                self.collect_inheritance_edges_resolved(child);
            }
        }
    }

    fn process_scope_node(&mut self, node: &Value, is_root: bool) {
        let name = node.get("name").and_then(|n| n.as_str()).unwrap_or("");
        let qualname = node
            .get("qualname")
            .and_then(|q| q.as_str())
            .unwrap_or(name);
        let scope_type = node
            .get("scope_type")
            .and_then(|t| t.as_str())
            .unwrap_or("unknown");

        if qualname.is_empty() {
            return;
        }

        // Accumulate all local top-level prefixes (one per root module)
        if is_root && scope_type == "module" {
            let top_level = qualname.split('.').next().unwrap_or(qualname);
            self.local_prefixes.insert(top_level.to_string());
        }

        let object_type = ObjectType::from(scope_type);
        let mut node_data = NodeData::new(qualname, object_type);

        // Set origin for root nodes
        if is_root {
            node_data.origin = Some(self.classify_module(qualname));
        }

        self.definitions.insert(qualname.to_string(), node_data);
        if scope_type == "module" {
            self.module_qualnames.insert(qualname.to_string());
        }
        self.record_node_contribution(qualname);
        self.ensure_parent_nodes(qualname);

        // Store function parameters for the CONSTRUCTOR-ARG pass.
        if let Some(params) = node.get("parameters").and_then(|p| p.as_array()) {
            if let Some(node_data) = self.definitions.get_mut(qualname) {
                node_data.parameters = params
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .filter(|p| p != "self" && p != "cls")
                    .collect();
            }
        }

        // Process children
        if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
            for child in children {
                self.process_scope_node(child, false);
            }
        }
    }

    /// Load and process definitions subscriber output
    /// This adds variable definitions and enriches existing nodes
    pub fn load_definitions(&mut self, data: &Value) {
        if let Some(definitions_by_scope) =
            data.get("definitions_by_scope").and_then(|d| d.as_object())
        {
            for (_scope, defs) in definitions_by_scope {
                if let Some(defs_array) = defs.as_array() {
                    for def in defs_array {
                        let qualname = def.get("qualname").and_then(|q| q.as_str()).unwrap_or("");
                        let def_type = def
                            .get("type")
                            .and_then(|t| t.as_str())
                            .unwrap_or("unknown");
                        let line = def.get("line").and_then(|l| l.as_u64()).unwrap_or(0) as usize;

                        if qualname.is_empty() {
                            continue;
                        }

                        // Update existing node or create new one
                        if let Some(node) = self.definitions.get_mut(qualname) {
                            node.position = Some((line, 0));
                        } else {
                            // New definition (variable, import, etc.)
                            let object_type = ObjectType::from(def_type);
                            let mut node = NodeData::new(qualname, object_type);
                            node.position = Some((line, 0));
                            self.definitions.insert(qualname.to_string(), node);
                            self.record_node_contribution(qualname);
                            self.ensure_parent_nodes(qualname);
                        }
                    }
                }
            }
        }
    }

    /// Load and process uses to create edges for variable/constant access
    pub fn load_uses(&mut self, data: &Value) {
        if let Some(uses_by_scope) = data.as_object() {
            for (scope, uses) in uses_by_scope {
                if let Some(uses_array) = uses.as_array() {
                    for use_item in uses_array {
                        let name = use_item.get("name").and_then(|n| n.as_str()).unwrap_or("");

                        // Skip dunder and standard excluded names
                        if name.starts_with("__") || name == "self" || name == "cls" {
                            continue;
                        }

                        // Try to resolve to a module-level definition
                        if let Some(resolved) = self.resolve_uses_target(scope, name) {
                            // Only create edge if both source and target are real definitions
                            if self.definitions.contains_key(scope)
                                && self.definitions.contains_key(&resolved)
                            {
                                // Check if target is not a parent of source (hierarchy already shows that)
                                if !scope.starts_with(&format!("{}.", resolved)) {
                                    let edge = EdgeData::new(scope, &resolved, "references");
                                    self.record_edge_contribution(edge.clone());
                                    self.edges.insert(edge);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Load decorator events and emit graph edges.
    ///
    /// Must run AFTER `load_import_bindings` (needs `import_bindings` for LEGB resolution).
    ///
    /// - Attribute-form decorator (`@obj.method`) where `obj` resolves to a local def
    ///   → `has-a` edge: `obj → decorated_fn` (containment/registration)
    /// - All other decorators → `references` edge: `decorated_fn → decorator_root` (use)
    pub fn load_decorators(&mut self, data: &Value) {
        let decorators = match data.get("decorators").and_then(|d| d.as_array()) {
            Some(arr) => arr.clone(),
            None => return,
        };
        for dec in &decorators {
            let decorated_fn = dec.get("decorated_fn").and_then(|v| v.as_str()).unwrap_or("");
            let root = dec.get("root").and_then(|v| v.as_str()).unwrap_or("");
            let is_attribute = dec.get("is_attribute").and_then(|v| v.as_bool()).unwrap_or(false);

            if root.is_empty() || decorated_fn.is_empty() {
                continue;
            }

            let resolved = self.resolve_name_legb(decorated_fn, root);

            if is_attribute {
                if let Some(ref local_def) = resolved {
                    if self.definitions.contains_key(local_def.as_str()) {
                        self.ensure_parent_nodes(decorated_fn);
                        let edge = EdgeData::new(local_def, decorated_fn, "has-a");
                        self.record_edge_contribution(edge.clone());
                        self.edges.insert(edge);
                        continue;
                    }
                }
            }

            // Fallback: references edge from decorated_fn → decorator root
            if let Some(ref local_def) = resolved {
                if self.definitions.contains_key(local_def.as_str())
                    && self.definitions.contains_key(decorated_fn)
                {
                    let edge = EdgeData::new(decorated_fn, local_def, "references");
                    self.record_edge_contribution(edge.clone());
                    self.edges.insert(edge);
                }
            }
        }
    }

    /// Scan all import data for __init__.py files and build a re-export map.
    ///
    /// When `server/__init__.py` does `from serpentine.server.app import create_app`,
    /// it re-exports `create_app` under the package's namespace. Any later import of
    /// `from serpentine.server import create_app` would produce a phantom edge target
    /// `src.serpentine.server.create_app`. The re-export map resolves that phantom to
    /// `src.serpentine.server.app.create_app` (the actual definition site).
    ///
    /// Must be called after `load_scope_tree` and `load_definitions` so that
    /// `self.definitions` is fully populated.
    pub fn build_reexport_map(&mut self, all_imports: &[Value]) {
        for data in all_imports {
            let imports = match data.get("imports").and_then(|i| i.as_array()) {
                Some(arr) => arr,
                None => continue,
            };

            for import in imports {
                let file = import.get("file").and_then(|f| f.as_str()).unwrap_or("");

                // Only re-export files (index.ts/js, __init__.py, mod.rs/lib.rs) re-export
                if !self.lang_configs.iter().any(|cfg| cfg.is_reexport_file(file)) {
                    continue;
                }

                let source_module = import
                    .get("source_module")
                    .and_then(|m| m.as_str())
                    .unwrap_or("");
                let imported_names = import
                    .get("imported_names")
                    .and_then(|n| n.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str())
                            .map(String::from)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();

                if source_module.is_empty()
                    || source_module.starts_with('.')
                    || imported_names.is_empty()
                {
                    continue;
                }

                // The package module for this __init__.py
                // e.g., serpentine/server/__init__.py → serpentine.server
                let package_module = self.file_to_module(file);

                let resolved_source = source_module.to_string();

                // Only build re-export entries for local sources.
                // Use module_qualnames for O(1) lookup instead of O(n) key scan.
                let source_is_local = self.definitions.contains_key(&resolved_source)
                    || self.module_qualnames.contains(&resolved_source);
                if !source_is_local {
                    continue;
                }

                for name in &imported_names {
                    if name == "*" {
                        continue;
                    }
                    let phantom = format!("{}.{}", package_module, name);
                    let actual = format!("{}.{}", resolved_source, name);

                    // Only record the mapping when the phantom doesn't exist as a
                    // real definition (it's truly a re-export) and the actual does.
                    if !self.definitions.contains_key(&phantom)
                        && self.definitions.contains_key(&actual)
                    {
                        self.record_reexport_contribution(&phantom);
                        self.reexport_map.insert(phantom, actual);
                    }
                }
            }
        }
    }

    /// Build the import binding table from import subscriber data.
    ///
    /// For each import statement, record what the imported name resolves to
    /// in the importing module's namespace. This is the "G" (Global/module)
    /// level of LEGB resolution.
    ///
    /// Must be called AFTER load_scope_tree, load_definitions, and build_reexport_map.
    /// Must be called BEFORE load_uses and load_raw_bindings.
    pub fn load_import_bindings(&mut self, data: &Value) {
        let imports = match data.get("imports").and_then(|i| i.as_array()) {
            Some(arr) => arr,
            None => return,
        };

        for import in imports {
            let source_module = import
                .get("source_module")
                .and_then(|m| m.as_str())
                .unwrap_or("");
            let imported_names = import
                .get("imported_names")
                .and_then(|n| n.as_array());
            let aliases = import
                .get("aliases")
                .and_then(|a| a.as_object());
            let file = import.get("file").and_then(|f| f.as_str()).unwrap_or("");

            if source_module.is_empty() || source_module.starts_with('.') {
                continue;
            }

            let importing_module = self.file_to_module(file);
            if importing_module.is_empty() {
                continue;
            }

            let resolved_source = source_module.to_string();

            if let Some(names) = imported_names {
                if names.is_empty() {
                    // `import foo` or `import foo.bar`
                    let top = resolved_source.split('.').next().unwrap_or(&resolved_source);
                    let local_name = aliases
                        .and_then(|a| a.get(&resolved_source))
                        .and_then(|v| v.as_str())
                        .unwrap_or(top);
                    let binding_key = format!("{}.{}", importing_module, local_name);
                    self.record_import_binding_contribution(&binding_key);
                    self.import_bindings.insert(binding_key, resolved_source.clone());
                } else {
                    // `from foo import bar, baz` or `from foo import bar as b`
                    for name_val in names {
                        let name = match name_val.as_str() {
                            Some(n) => n,
                            None => continue,
                        };
                        if name == "*" {
                            continue;
                        }
                        let local_name = aliases
                            .and_then(|a| a.get(name))
                            .and_then(|v| v.as_str())
                            .unwrap_or(name);
                        let raw_target = format!("{}.{}", resolved_source, name);
                        let resolved_target = self
                            .reexport_map
                            .get(&raw_target)
                            .cloned()
                            .unwrap_or(raw_target);
                        let binding_key = format!("{}.{}", importing_module, local_name);
                        self.record_import_binding_contribution(&binding_key);
                        self.import_bindings.insert(binding_key, resolved_target);
                    }
                }
            } else {
                // No imported_names field → bare `import foo`
                let top = resolved_source.split('.').next().unwrap_or(&resolved_source);
                let binding_key = format!("{}.{}", importing_module, top);
                self.record_import_binding_contribution(&binding_key);
                self.import_bindings.insert(binding_key, resolved_source.clone());
            }
        }

        // Post-process: follow import chains to their ultimate target.
        // Python forbids circular imports, so each chain terminates.
        // Depth limit of 20 is a safety net for malformed data only.
        let keys: Vec<String> = self.import_bindings.keys().cloned().collect();
        for key in keys {
            let Some(mut value) = self.import_bindings.get(&key).cloned() else { continue };
            for _ in 0..20 {
                match self.import_bindings.get(&value).cloned() {
                    Some(next) if next != value => value = next,
                    _ => break,
                }
            }
            self.import_bindings.insert(key, value);
        }
    }

    /// Imports create edges from the importing module to the imported module.
    /// External imports create nodes only if they don't match any local module.
    pub fn load_imports(&mut self, data: &Value) {
        let imports = match data.get("imports").and_then(|i| i.as_array()) {
            Some(arr) => arr,
            None => return,
        };

        for import in imports {
            let source_module = import
                .get("source_module")
                .and_then(|m| m.as_str())
                .unwrap_or("");
            let imported_names = import
                .get("imported_names")
                .and_then(|n| n.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .map(String::from)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let file = import.get("file").and_then(|f| f.as_str()).unwrap_or("");

            // Skip empty source modules (relative imports that couldn't be resolved)
            if source_module.is_empty() || source_module.starts_with('.') {
                continue;
            }

            // Get the importing module from the file path
            let importing_module = self.file_to_module(file);
            if importing_module.is_empty() {
                continue;
            }

            let resolved_module = source_module.to_string();

            // Only emit imports edges for project-local targets. Terraform (.tf) files
            // are exempt: their source paths are already normalized from "./" prefixes
            // by normalize_source_path, so all surviving sources are local-by-intent
            // even when the target file is not in the analyzed set.
            let is_terraform = file.ends_with(".tf");
            if !is_terraform {
                use crate::graph::Origin;
                if self.classify_module(&resolved_module) != Origin::Local {
                    continue;
                }
            }

            // Determine what to create an edge to
            if imported_names.is_empty() {
                // `import foo` or `import foo.bar` - edge to the module itself
                if !crate::graph::resolvers::is_ancestor(&resolved_module, &importing_module) {
                    self.ensure_import_target(&resolved_module);
                    if self.definitions.contains_key(&resolved_module) {
                        let edge = EdgeData::new(&importing_module, &resolved_module, "imports");
                        self.record_edge_contribution(edge.clone());
                        self.edges.insert(edge);
                    }
                }
            } else if imported_names.contains(&"*".to_string()) {
                // `from foo import *` - edge to the module
                if !crate::graph::resolvers::is_ancestor(&resolved_module, &importing_module) {
                    self.ensure_import_target(&resolved_module);
                    if self.definitions.contains_key(&resolved_module) {
                        let edge = EdgeData::new(&importing_module, &resolved_module, "imports");
                        self.record_edge_contribution(edge.clone());
                        self.edges.insert(edge);
                    }
                }
            } else {
                // `from foo import bar, baz` - create edges to each imported item
                for name in &imported_names {
                    let raw_target = format!("{}.{}", resolved_module, name);
                    // Follow re-export map: if raw_target is a phantom re-exported
                    // through __init__.py, resolve to the actual definition site.
                    let target = self
                        .reexport_map
                        .get(&raw_target)
                        .cloned()
                        .unwrap_or(raw_target);
                    if !crate::graph::resolvers::is_ancestor(&target, &importing_module) {
                        self.ensure_import_target(&target);
                        if self.definitions.contains_key(&target) {
                            let edge = EdgeData::new(&importing_module, &target, "imports");
                            self.record_edge_contribution(edge.clone());
                            self.edges.insert(edge);
                        }
                    }
                }
            }
        }
    }

    /// Load and process raw_bindings to create edges.
    ///
    /// Two-pass approach:
    ///   1. ASSIGNED pass — creates has-a edges (variable type annotations)
    ///   2. Build edge_caller_index from all current edges
    ///   3. CALLS pass — resolves call targets; uses edge_caller_index via
    ///      resolve_variable_type for O(1) self.attr type lookups
    pub fn load_raw_bindings(&mut self, data: &Value) {
        let bindings = match data {
            Value::Array(arr) => arr,
            _ => return,
        };

        // Snapshot edge set before this pass so we can compute the net additions.
        let edges_before: std::collections::HashSet<EdgeData> = self.edges.clone();

        // Pass 1: ASSIGNED — build has-a edges so the index is fully populated
        for binding in bindings {
            if binding.get("relationship").and_then(|r| r.as_str()) != Some("ASSIGNED") {
                continue;
            }
            let scope = binding.get("scope").and_then(|s| s.as_str()).unwrap_or("");
            let source = binding.get("source");
            let target = binding.get("target");

            if let (Some(source_obj), Some(target_obj)) = (source, target) {
                let source_qualname = source_obj
                    .get("qualname")
                    .and_then(|q| q.as_str())
                    .unwrap_or("");
                let target_text = target_obj
                    .get("text")
                    .and_then(|t| t.as_str())
                    .unwrap_or("");
                let target_category = target_obj
                    .get("category")
                    .and_then(|c| c.as_str())
                    .unwrap_or("");

                if !source_qualname.is_empty() && target_category == "call" {
                    let callable = self.extract_callable(target_text);
                    if let Some(resolved) = self.resolve_callee(scope, &callable) {
                        if !crate::graph::resolvers::is_ancestor(&resolved, source_qualname)
                            && (self.definitions.contains_key(source_qualname)
                                || self.definitions.contains_key(&resolved))
                        {
                            let top = resolved.split('.').next().unwrap_or(&resolved);
                            if !self.local_prefixes.contains(top) {
                                self.ensure_external_node(&resolved);
                            }
                            if self.definitions.contains_key(&resolved) {
                                self.edges.insert(EdgeData::new(source_qualname, &resolved, "has-a"));
                                // For factory functions, also emit a calls edge so the factory
                                // relationship is preserved after RETYPE replaces the has-a.
                                if self
                                    .definitions
                                    .get(&resolved)
                                    .map(|n| n.object_type == ObjectType::Function)
                                    .unwrap_or(false)
                                {
                                    self.edges.insert(EdgeData::new(source_qualname, &resolved, "calls"));
                                }
                            }
                        }
                    }

                    // Emit data-flow edges from call arguments to the LHS variable.
                    // For `x = f(a, b)` → `x --references--> a` and `x --references--> b`,
                    // showing which sibling variables x's construction depended on.
                    // Handles nested calls like `x = outer(inner(y))` → `x --references--> y`
                    // by recursively extracting identifiers from complex argument expressions.
                    // These are sibling→sibling edges and survive the ancestor filter.
                    let args = extract_call_args(target_text);
                    let scope_prefix = format!("{}.", scope);
                    for arg_text in &args {
                        let arg = arg_text.trim();
                        // For keyword args like "path=project_path", take only the value
                        let value_str = if let Some(idx) = arg.find('=') {
                            arg[idx + 1..].trim()
                        } else {
                            arg
                        };
                        if value_str.is_empty() {
                            continue;
                        }
                        // For simple identifiers resolve directly; for complex expressions
                        // (e.g. nested calls like `inner(y)`) extract all identifier tokens.
                        let idents: Vec<String> =
                            if value_str.chars().all(|c| c.is_alphanumeric() || c == '_') {
                                vec![value_str.to_string()]
                            } else {
                                extract_identifier_refs(value_str)
                            };
                        for ident in &idents {
                            if let Some(resolved_arg) = self.resolve_name_legb(scope, ident) {
                                // Only link to sibling local variables in the same function scope
                                if resolved_arg.starts_with(&scope_prefix)
                                    && self.definitions.contains_key(source_qualname)
                                    && self.definitions.contains_key(&resolved_arg)
                                {
                                    self.edges.insert(EdgeData::new(source_qualname, &resolved_arg, "references"));
                                }
                            }
                        }
                    }
                } else if target_category == "name" && !target_text.is_empty() {
                    if let Some(resolved) = self.resolve_name_legb(scope, target_text) {
                        let top = resolved.split('.').next().unwrap_or(&resolved);
                        if !self.local_prefixes.contains(top) {
                            self.ensure_external_node(&resolved);
                        }
                        if source_qualname != resolved
                            && !crate::graph::resolvers::is_ancestor(&resolved, source_qualname)
                            && (self.definitions.contains_key(source_qualname)
                                || self.definitions.contains_key(&resolved))
                        {
                            self.edges.insert(EdgeData::new(source_qualname, &resolved, "references"));
                        }
                    }
                } else if matches!(target_category, "expression" | "attribute" | "collection") && !target_text.is_empty() {
                    for name in extract_identifier_refs(target_text) {
                        if let Some(resolved) = self.resolve_name_legb(scope, &name) {
                            let top = resolved.split('.').next().unwrap_or(&resolved);
                            if !self.local_prefixes.contains(top) {
                                self.ensure_external_node(&resolved);
                            }
                            if source_qualname != resolved
                                && !crate::graph::resolvers::is_ancestor(&resolved, source_qualname)
                                && (self.definitions.contains_key(source_qualname)
                                    || self.definitions.contains_key(&resolved))
                            {
                                self.edges.insert(EdgeData::new(source_qualname, &resolved, "references"));
                            }
                        }
                    }
                }
            }
        }

        // Build edge index from has-a edges so resolve_variable_type is O(1)
        self.build_edge_caller_index();

        // Pass 2.5: RETURNS — resolve function return types from return statements.
        // Runs after edge_caller_index is built so has-a edges are queryable.
        // Populates function_return_types used by resolve_variable_type in the CALLS pass.
        let mut fn_return_types: HashMap<String, Vec<String>> = HashMap::new();
        for binding in bindings {
            if binding.get("relationship").and_then(|r| r.as_str()) != Some("RETURNS") {
                continue;
            }
            let scope = binding.get("scope").and_then(|s| s.as_str()).unwrap_or("");
            let return_text = binding
                .get("target").and_then(|t| t.get("text")).and_then(|t| t.as_str())
                .unwrap_or("");

            if scope.is_empty() || return_text.is_empty() {
                continue;
            }

            // Build the qualified name of the returned variable: scope.var_name
            let return_var_qualname = format!("{}.{}", scope, return_text);

            if let Some(entries) = self.edge_caller_index.get(&return_var_qualname) {
                for edge in entries.clone() {
                    if edge.edge_type == "has-a" {
                        if let Some(node) = self.definitions.get(&edge.callee) {
                            if node.object_type == ObjectType::Class {
                                fn_return_types
                                    .entry(scope.to_string())
                                    .or_default()
                                    .push(edge.callee.clone());
                            }
                        }
                    }
                }
            }

            // Also handle direct constructor-call returns: `return ClassName(...)`.
            // The variable-lookup path above only works for `return some_var` where
            // `some_var` already has a has-a edge. For a direct return like
            // `return Starlette(routes=routes)` there is no intermediate variable,
            // so we extract the callee name directly from the return expression.
            //
            // External classes (e.g. starlette.applications.Starlette) are typed
            // ObjectType::Unknown by ensure_external_node. Use naming convention
            // (last segment starts uppercase) as a fallback to identify them as
            // class constructors — this is PEP 8 / PascalCase convention.
            if return_text.contains('(') {
                let callable = self.extract_callable(return_text);
                if let Some(resolved) = self.resolve_callee(scope, &callable) {
                    let last_seg = resolved.split('.').next_back().unwrap_or(&resolved);
                    let is_class_like = self
                        .definitions
                        .get(&resolved)
                        .map(|n| n.object_type == ObjectType::Class)
                        .unwrap_or(false)
                        || last_seg.chars().next().map(|c| c.is_uppercase()).unwrap_or(false);
                    if is_class_like {
                        fn_return_types
                            .entry(scope.to_string())
                            .or_default()
                            .push(resolved);
                    }
                }
            }
        }

        // Commit to function_return_types: only when all returns agree on one class.
        // Ambiguous (multiple different return types) → don't insert.
        for (fn_qualname, types) in fn_return_types {
            let unique: std::collections::HashSet<_> = types.iter().collect();
            if unique.len() == 1 {
                self.function_return_types.insert(fn_qualname, types.into_iter().next().unwrap());
            }
        }

        // Pass 2.55: RETYPE — fix has-a edges where the callee is a factory function.
        //
        // When `car = build_car()` is analyzed in the ASSIGNED pass, it creates
        // `main.car --has-a--> build_car` (pointing at the function, not the returned type).
        // Now that function_return_types is populated, we can replace those edges with
        // `main.car --has-a--> Car`.
        //
        // For ambiguous return types (function absent from function_return_types) the edge
        // to the function is removed with no replacement — keeping it would be misleading.
        {
            let edges_to_retype: Vec<EdgeData> = self
                .edges
                .iter()
                .filter(|e| {
                    e.edge_type == "has-a"
                        && self
                            .definitions
                            .get(&e.callee)
                            .map(|d| d.object_type == ObjectType::Function)
                            .unwrap_or(false)
                })
                .cloned()
                .collect();

            for old_edge in &edges_to_retype {
                if let Some(new_callee) = self.function_return_types.get(&old_edge.callee) {
                    self.edges.remove(old_edge);
                    self.edges.insert(EdgeData::new(
                        &old_edge.caller,
                        new_callee,
                        "has-a",
                    ));
                }
                // No known return type — keep the edge as-is (may be a decorator registration)
            }

            if !edges_to_retype.is_empty() {
                self.build_edge_caller_index();
            }
        }

        // Pass 2.6: CONSTRUCTOR-ARG — propagate call-site argument types to Class.__init__ params.
        //
        // For each ASSIGNED binding where the RHS is a Class constructor call:
        //   Car(eng) → arg[0] = "eng" → resolve in scope → type = Engine
        //           → param[0] of Car.__init__ = "engine"
        //           → record Car.__init__.engine --has-a--> Engine in edge_caller_index
        //
        // Only handles simple identifier arguments (skips expressions like "a + b").
        let mut constructor_arg_edges: Vec<EdgeData> = Vec::new();
        for binding in bindings {
            if binding.get("relationship").and_then(|r| r.as_str()) != Some("ASSIGNED") {
                continue;
            }
            let scope = binding.get("scope").and_then(|s| s.as_str()).unwrap_or("");
            let target_text = binding
                .get("target").and_then(|t| t.get("text")).and_then(|t| t.as_str()).unwrap_or("");
            let target_category = binding
                .get("target").and_then(|t| t.get("category")).and_then(|c| c.as_str()).unwrap_or("");

            if target_category != "call" || target_text.is_empty() {
                continue;
            }

            let callable = self.extract_callable(target_text);
            let resolved_callee = match self.resolve_callee(scope, &callable) {
                Some(r) => r,
                None => continue,
            };

            // Only Class constructors
            let is_class = self.definitions.get(&resolved_callee)
                .map(|n| n.object_type == ObjectType::Class).unwrap_or(false);
            if !is_class {
                continue;
            }

            let init_qualname = format!("{}.__init__", resolved_callee);
            let params: Vec<String> = self.definitions.get(&init_qualname)
                .map(|n| n.parameters.clone()).unwrap_or_default();
            if params.is_empty() {
                continue;
            }

            let args = extract_call_args(target_text);

            for (i, param_name) in params.iter().enumerate() {
                let arg_text = match args.get(i) { Some(a) => a.trim(), None => break };
                // Only simple identifiers
                if !arg_text.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    continue;
                }
                let resolved_arg = match self.resolve_name_legb(scope, arg_text) {
                    Some(r) => r,
                    None => continue,
                };
                if let Some(entries) = self.edge_caller_index.get(&resolved_arg) {
                    for edge in entries.clone() {
                        if edge.edge_type == "has-a" {
                            if let Some(node) = self.definitions.get(&edge.callee) {
                                if node.object_type == ObjectType::Class {
                                    let param_qualname = format!("{}.{}", init_qualname, param_name);
                                    constructor_arg_edges.push(EdgeData {
                                        caller: param_qualname,
                                        callee: edge.callee.clone(),
                                        edge_type: "has-a".to_string(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
        for e in constructor_arg_edges {
            self.edge_caller_index.entry(e.caller.clone()).or_default().push(e);
        }

        // Pass 2.75: PARAM-TYPE — propagate parameter types into class attribute has-a edges.
        // For `self.X = param` inside __init__, if param has a known type (has-a → Class),
        // record Class.X --has-a--> TypeClass so resolve_self_access can follow it.
        for binding in bindings {
            if binding.get("relationship").and_then(|r| r.as_str()) != Some("ASSIGNED") {
                continue;
            }
            let scope = binding.get("scope").and_then(|s| s.as_str()).unwrap_or("");
            let source_text = binding.get("source").and_then(|s| s.get("text")).and_then(|t| t.as_str()).unwrap_or("");
            let target_text = binding.get("target").and_then(|t| t.get("text")).and_then(|t| t.as_str()).unwrap_or("");

            // Only handle self.X = param_name inside __init__
            if !scope.ends_with(".__init__") { continue; }
            if !source_text.starts_with("self.") { continue; }
            let attr_name = &source_text["self.".len()..];
            if attr_name.is_empty() || attr_name.contains('.') { continue; }
            if target_text.is_empty() { continue; }

            // Class qualname = scope without ".__init__"
            let class_qualname = &scope[..scope.len() - ".__init__".len()];

            // Param qualname inside __init__ = scope.param_name
            let param_qualname = format!("{}.{}", scope, target_text);

            // Look up type of the param via has-a edges
            if let Some(entries) = self.edge_caller_index.get(&param_qualname).cloned() {
                let mut new_edges: Vec<EdgeData> = Vec::new();
                for edge in entries {
                    if edge.edge_type == "has-a" {
                        if let Some(type_node) = self.definitions.get(&edge.callee) {
                            if type_node.object_type == ObjectType::Class {
                                // Add class.attr --has-a--> TypeClass into edge_caller_index
                                let class_attr_qualname = format!("{}.{}", class_qualname, attr_name);
                                new_edges.push(EdgeData {
                                    caller: class_attr_qualname,
                                    callee: edge.callee.clone(),
                                    edge_type: "has-a".to_string(),
                                });
                            }
                        }
                    }
                }
                for new_edge in new_edges {
                    self.edge_caller_index
                        .entry(new_edge.caller.clone())
                        .or_default()
                        .push(new_edge);
                }
            }
        }

        // Pass 2: CALLS — resolve call targets using the populated edge index
        for binding in bindings {
            if binding.get("relationship").and_then(|r| r.as_str()) != Some("CALLS") {
                continue;
            }
            let scope = binding.get("scope").and_then(|s| s.as_str()).unwrap_or("");
            let target = binding.get("target");

            if let Some(target_obj) = target {
                let callee_text = target_obj
                    .get("text")
                    .and_then(|t| t.as_str())
                    .unwrap_or("");

                if let Some(resolved) = self.resolve_callee(scope, callee_text) {
                    // For dotted callees like `obj.method`, use the resolved receiver
                    // variable (obj → mymod.obj) as the edge source so the edge reads
                    // "obj calls method" rather than "module calls method".
                    let caller: String = if callee_text.contains('.') {
                        let first = callee_text.split('.').next().unwrap_or("");
                        if !first.is_empty() {
                            if let Some(recv) = self.resolve_name_legb(scope, first) {
                                let top = recv.split('.').next().unwrap_or(&recv);
                                if self.local_prefixes.contains(top)
                                    && self.definitions.contains_key(&recv)
                                {
                                    recv
                                } else {
                                    scope.to_string()
                                }
                            } else {
                                scope.to_string()
                            }
                        } else {
                            scope.to_string()
                        }
                    } else {
                        scope.to_string()
                    };

                    if (self.definitions.contains_key(scope) || caller != scope)
                        && !crate::graph::resolvers::is_ancestor(&resolved, &caller)
                    {
                        let top = resolved.split('.').next().unwrap_or(&resolved);
                        if !self.local_prefixes.contains(top) {
                            self.ensure_external_node(&resolved);
                        }
                        if self.definitions.contains_key(&resolved) {
                            self.edges.insert(EdgeData::new(&caller, &resolved, "calls"));
                        }
                    }

                    // For method calls on local variables (`obj.method(arg)`), emit
                    // `obj --references--> arg` for each local-variable argument.
                    // This captures the data-flow dependency: obj "uses" its arguments.
                    // Guard: only fire when the receiver (`caller`) is a local variable in
                    // the current scope — i.e. its qualname starts with the scope prefix.
                    // Without this, `ClassName.static_method(arg)` from another module
                    // falsely emits `ClassName --references--> arg`.
                    let scope_prefix = format!("{}.", scope);
                    if caller != scope && callee_text.contains('.') && caller.starts_with(&scope_prefix) {
                        let call_args: Vec<String> = target_obj
                            .get("arguments")
                            .and_then(|a| a.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default();
                        for arg_text in &call_args {
                            let idents: Vec<String> =
                                if arg_text.chars().all(|c| c.is_alphanumeric() || c == '_') {
                                    vec![arg_text.clone()]
                                } else {
                                    extract_identifier_refs(arg_text)
                                };
                            for ident in &idents {
                                if let Some(resolved_arg) = self.resolve_name_legb(scope, ident) {
                                    if resolved_arg.starts_with(&scope_prefix)
                                        && self.definitions.contains_key(&caller)
                                        && self.definitions.contains_key(&resolved_arg)
                                    {
                                        self.edges.insert(EdgeData::new(&caller, &resolved_arg, "references"));
                                    }
                                }
                            }
                        }
                    }
                } else if callee_text.contains('.') {
                    // resolve_callee returned None (method not in definitions), but if the
                    // receiver is a known local variable we can still emit
                    // `recv --references--> arg` to capture the data-flow dependency.
                    let first = callee_text.split('.').next().unwrap_or("");
                    if !first.is_empty() {
                        if let Some(recv) = self.resolve_name_legb(scope, first) {
                            let top = recv.split('.').next().unwrap_or(&recv);
                            if self.local_prefixes.contains(top) && self.definitions.contains_key(&recv) {
                                let call_args: Vec<String> = target_obj
                                    .get("arguments")
                                    .and_then(|a| a.as_array())
                                    .map(|arr| {
                                        arr.iter()
                                            .filter_map(|v| v.as_str().map(String::from))
                                            .collect()
                                    })
                                    .unwrap_or_default();
                                let scope_prefix = format!("{}.", scope);
                                for arg_text in &call_args {
                                    let idents: Vec<String> =
                                        if arg_text.chars().all(|c| c.is_alphanumeric() || c == '_') {
                                            vec![arg_text.clone()]
                                        } else {
                                            extract_identifier_refs(arg_text)
                                        };
                                    for ident in &idents {
                                        if let Some(resolved_arg) = self.resolve_name_legb(scope, ident) {
                                            if resolved_arg.starts_with(&scope_prefix)
                                                && self.definitions.contains_key(&resolved_arg)
                                            {
                                                self.edges.insert(EdgeData::new(&recv, &resolved_arg, "references"));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Record all net-new edges from this pass for incremental invalidation.
        // This includes edges added by ASSIGNED, CALLS, and surviving RETYPE edges,
        // but excludes per-file edges that existed before this call.
        self.raw_binding_edges = self.edges.difference(&edges_before).cloned().collect();
    }

    /// Load PDG data and attach to function and module nodes
    pub fn load_pdgs(&mut self, data: &Value) {
        if let Some(pdgs) = data.get("pdgs").and_then(|c| c.as_object()) {
            for (qualname, pdg_data) in pdgs {
                // Attach complete PDG (nodes + edges) to the corresponding node
                if let Some(node) = self.definitions.get_mut(qualname) {
                    match node.object_type {
                        ObjectType::Function | ObjectType::Module => {
                            // Store the complete pdg object with both nodes and edges
                            node.pdg = Some(pdg_data.clone());
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /// Load code snippet data and populate `code_block` on each node.
    ///
    /// Uses the source lines and scope ranges collected by the CodeSnippetSubscriber
    /// to extract the relevant source code for each definition.
    pub fn load_code_snippets(&mut self, data: &Value) {
        // Parse source_lines: { "file_path": ["line1", "line2", ...] }
        let source_lines: HashMap<String, Vec<String>> = data
            .get("source_lines")
            .and_then(|s| s.as_object())
            .map(|obj| {
                obj.iter()
                    .map(|(file, lines)| {
                        let line_vec: Vec<String> = lines
                            .as_array()
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default();
                        (file.clone(), line_vec)
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Parse scope_ranges: { "qualname": { "file": "...", "start_line": N, "end_line": M } }
        if let Some(ranges) = data.get("scope_ranges").and_then(|s| s.as_object()) {
            for (qualname, range_val) in ranges {
                let file = range_val.get("file").and_then(|f| f.as_str()).unwrap_or("");
                let start_line = range_val
                    .get("start_line")
                    .and_then(|l| l.as_u64())
                    .unwrap_or(0) as usize;
                let end_line = range_val
                    .get("end_line")
                    .and_then(|l| l.as_u64())
                    .unwrap_or(0) as usize;

                if start_line == 0 || end_line == 0 || file.is_empty() {
                    continue;
                }

                if let Some(node) = self.definitions.get_mut(qualname) {
                    node.file_path = Some(file.to_string());
                    if let Some(lines) = source_lines.get(file) {
                        // Convert 1-indexed lines to 0-indexed vec access
                        let start_idx = start_line.saturating_sub(1);
                        let end_idx = end_line.min(lines.len());
                        if start_idx < end_idx {
                            let code = lines[start_idx..end_idx].join("\n");
                            if !code.is_empty() {
                                node.content_hash = Some(fnv1a_hash(&code));
                                node.code_block = Some(code);
                            }
                        }
                    }
                }
            }
        }

        // Parse docstrings: { "qualname": "text" }
        if let Some(docstrings) = data.get("docstrings").and_then(|d| d.as_object()) {
            for (qualname, text_val) in docstrings {
                if let Some(text) = text_val.as_str() {
                    if let Some(node) = self.definitions.get_mut(qualname) {
                        node.docstring = Some(text.to_string());
                    }
                }
            }
        }
    }
}
