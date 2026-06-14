use super::{GraphBuilder, NodeData, ObjectType, Origin};

/// Returns true if `ancestor` is a strict ancestor of `descendant` in the
/// dotted-name hierarchy (e.g. `serpentine` is an ancestor of `serpentine.state`).
/// A node must never have an edge pointing at its own ancestor — that would be
/// a self-referential package dependency.
pub(crate) fn is_ancestor(ancestor: &str, descendant: &str) -> bool {
    descendant.starts_with(&format!("{}.", ancestor))
}

impl GraphBuilder {
    /// Classify a module name
    pub(crate) fn classify_module(&self, name: &str) -> Origin {
        let top_level = name.split('.').next().unwrap_or(name);

        // Check stdlib first (before checking local prefixes)
        for config in &self.lang_configs {
            if config.is_stdlib(top_level) {
                return Origin::Standard;
            }
        }

        // local_prefixes holds every top-level local module name — O(1) lookup
        if self.local_prefixes.contains(top_level) {
            return Origin::Local;
        }

        Origin::ThirdParty
    }

    /// Resolve a name using LEGB (Local → Enclosing → Global → Built-in) rules.
    ///
    /// Given a scope and a simple name (no dots), walks up the scope hierarchy
    /// checking definitions and import bindings at each level. Returns the
    /// fully-qualified name of the resolved definition, or None if not found.
    ///
    /// This is the canonical name resolution function. All other resolution
    /// (resolve_callee, resolve_uses_target) delegates to this for simple names.
    pub(crate) fn resolve_name_legb(&self, scope: &str, name: &str) -> Option<String> {
        if name.is_empty() {
            return None;
        }

        // Thread-local scratch buffer: avoids a String allocation per scope level.
        // We build "prefix.name" in-place and call HashMap::get(&str), which only
        // allocates when the lookup actually succeeds (returning Some(clone)).
        thread_local! {
            static BUF: std::cell::RefCell<String> = std::cell::RefCell::new(String::with_capacity(256));
        }

        BUF.with(|cell| {
            let mut buf = cell.borrow_mut();

            // L + E + G: Walk up scope hierarchy from innermost to outermost.
            // Use rfind('.') to peel one component at a time — no Vec or join needed.
            let mut prefix = scope;
            loop {
                buf.clear();
                buf.push_str(prefix);
                buf.push('.');
                buf.push_str(name);

                // Check definitions first.
                // Skip Module-type: a submodule `foo.bar` does NOT mean `bar` is in
                // `foo`'s namespace — only an explicit import binding makes it so.
                if let Some(def) = self.definitions.get(buf.as_str()) {
                    if def.object_type != ObjectType::Module {
                        return Some(buf.clone());
                    }
                }

                // Check import bindings (module-level imported names)
                if let Some(resolved) = self.import_bindings.get(buf.as_str()) {
                    return Some(resolved.clone());
                }

                match prefix.rfind('.') {
                    Some(dot) => prefix = &prefix[..dot],
                    None => break,
                }
            }

            // B: Builtins
            for config in &self.lang_configs {
                if config.is_stdlib(name) {
                    return Some(name.to_string());
                }
            }

            if is_python_builtin(name) {
                return Some(format!("builtins.{}", name));
            }

            None
        })
    }

    /// Resolve a uses target (variable/constant name) to its qualname.
    pub(crate) fn resolve_uses_target(&self, scope: &str, name: &str) -> Option<String> {
        if name.is_empty() {
            return None;
        }

        // For dotted names, check if it exists as-is first
        if name.contains('.') {
            if self.definitions.contains_key(name) {
                return Some(name.to_string());
            }
            return None;
        }

        // Simple name — use LEGB
        self.resolve_name_legb(scope, name)
    }

    /// Resolve a callee name to a full qualname of an actual definition
    pub(crate) fn resolve_callee(&self, scope: &str, callee_text: &str) -> Option<String> {
        if callee_text.is_empty() {
            return None;
        }

        // Fast path: no dot → simple LEGB lookup (avoids Vec allocation entirely)
        let Some(first_dot) = callee_text.find('.') else {
            return self.resolve_name_legb(scope, callee_text);
        };

        let first = &callee_text[..first_dot];

        // Handle "self.something" — resolve to class method/attribute
        if first == "self" {
            let parts: Vec<&str> = callee_text[first_dot + 1..].split('.').collect();
            return self.resolve_self_access(scope, &parts);
        }

        // LEGB Resolution for the first part
        let resolved_base = self.resolve_name_legb(scope, first)?;

        // Dotted call like foo.bar.baz() — resolve remaining parts without Vec
        let mut current = resolved_base;
        let mut rest = &callee_text[first_dot + 1..];
        loop {
            let (part, tail) = match rest.find('.') {
                Some(p) => (&rest[..p], Some(&rest[p + 1..])),
                None    => (rest, None),
            };

            let candidate = format!("{}.{}", current, part);
            if self.definitions.contains_key(&candidate) {
                current = candidate;
            } else {
                // current may be a typed variable — follow its type via has-a edges
                if let Some(type_qualname) = self.resolve_variable_type(&current) {
                    let type_candidate = format!("{}.{}", type_qualname, part);
                    if self.definitions.contains_key(&type_candidate) {
                        current = type_candidate;
                        rest = match tail { Some(t) => t, None => break };
                        continue;
                    }
                }
                // For external modules (not in local_prefixes), build a stub path.
                let top = current.split('.').next().unwrap_or(&current);
                if !self.local_prefixes.contains(top) {
                    current = format!("{}.{}", current, part);
                    rest = match tail { Some(t) => t, None => break };
                    continue;
                }
                return None;
            }

            rest = match tail { Some(t) => t, None => break };
        }

        Some(current)
    }

    /// Resolve self.method() or self.attr.method() access.
    ///
    /// For self.method(): look up method on the enclosing class.
    /// For self.attr.method(): resolve attr's type via has-a edges,
    ///   then look up method on that type.
    /// Returns None if anything can't be proven.
    fn resolve_self_access(&self, scope: &str, parts: &[&str]) -> Option<String> {
        // Find enclosing class from scope
        let scope_parts: Vec<&str> = scope.split('.').collect();
        let class_qualname = (0..scope_parts.len()).rev().find_map(|i| {
            let potential = scope_parts[..=i].join(".");
            self.definitions.get(&potential).and_then(|node| {
                if node.object_type == ObjectType::Class {
                    Some(potential)
                } else {
                    None
                }
            })
        })?;

        if parts.len() == 1 {
            // self.method() — direct class method lookup
            let method_qualname = format!("{}.{}", class_qualname, parts[0]);
            if self.definitions.contains_key(&method_qualname) {
                return Some(method_qualname);
            }
            return None;
        }

        // self.attr.method() — resolve attr type through has-a edges
        let attr_qualname = format!("{}.{}", class_qualname, parts[0]);
        if let Some(attr_type) = self.resolve_variable_type(&attr_qualname) {
            let method_qualname = format!("{}.{}", attr_type, parts[1]);
            if self.definitions.contains_key(&method_qualname) {
                return Some(method_qualname);
            }
        }

        // Can't prove it — return None
        None
    }

    /// Resolve a variable's type by following has-a edges to find a class definition.
    ///
    /// Given a variable qualname like "src.test_package.app.main.car", follows:
    ///   1. Direct: main.car --has-a--> Car (if assigned from Car())
    ///   2. Transitive: main.car --has-a--> build_car, then look at build_car's
    ///      return variable to find what class it returns.
    ///
    /// Uses edge_caller_index (built before the CALLS pass) for O(1) caller lookups.
    fn resolve_variable_type(&self, var_qualname: &str) -> Option<String> {
        let entries = self.edge_caller_index.get(var_qualname)?;

        for edge in entries {
            if edge.edge_type != "has-a" {
                continue;
            }
            let target = &edge.callee;

            if let Some(node) = self.definitions.get(target) {
                if node.object_type == ObjectType::Class {
                    return Some(target.clone());
                }

                if node.object_type == ObjectType::Function {
                    // Use recorded return type — no heuristics
                    if let Some(return_type) = self.function_return_types.get(target) {
                        return Some(return_type.clone());
                    }
                }
            }
        }
        None
    }

    /// Extract callable name from call expression like "Car(eng)" -> "Car".
    /// Handles JavaScript `new` expressions: "new Car(eng)" → "Car".
    /// Also strips Rust constructor suffix: "GraphBuilder.new" -> "GraphBuilder".
    pub(crate) fn extract_callable(&self, call_text: &str) -> String {
        let trimmed = call_text.trim();
        // Strip JavaScript `new` keyword: "new Foo(args)" → "Foo(args)"
        let text = if let Some(rest) = trimmed.strip_prefix("new") {
            if rest.starts_with(char::is_whitespace) {
                rest.trim_start()
            } else {
                trimmed
            }
        } else {
            trimmed
        };
        let base = text.split('(').next().unwrap_or(text).trim();
        // Strip Rust constructor convention: `Type.new` → `Type`
        if let Some(stripped) = base.strip_suffix(".new") {
            stripped.to_string()
        } else {
            base.to_string()
        }
    }

    /// Ensure all parent nodes exist in the hierarchy
    /// E.g., if we have test_package.app.Car, ensure test_package and test_package.app exist
    pub(crate) fn ensure_parent_nodes(&mut self, qualname: &str) {
        let parts: Vec<&str> = qualname.split('.').collect();

        // Build all ancestor qualnames (shallowest first so parents exist when
        // register_node_in_hierarchy checks for them below).
        for i in 1..parts.len() {
            let parent_qualname = parts[0..i].join(".");

            if !self.definitions.contains_key(&parent_qualname) {
                let object_type = ObjectType::Module;

                let mut parent_node = NodeData::new(&parent_qualname, object_type);

                if i == 1 {
                    parent_node.origin = Some(self.classify_module(&parent_qualname));
                }

                self.definitions.insert(parent_qualname.clone(), parent_node);
                self.register_node_in_hierarchy(&parent_qualname);
            }
        }
    }

    /// Ensure an import target exists - only creates external nodes, not local ones
    pub(crate) fn ensure_import_target(&mut self, target: &str) {
        // Don't create if it already exists (it's local)
        if self.definitions.contains_key(target) {
            return;
        }

        // If the top-level component is a known local prefix, this is a child of
        // a local module — don't create an external node. O(1) via HashSet.
        let top = target.split('.').next().unwrap_or(target);
        if self.local_prefixes.contains(top) {
            return;
        }

        // This is truly external, create the node
        self.ensure_external_node(target);
    }

    /// Ensure a node exists for external references (stdlib, third-party)
    pub(crate) fn ensure_external_node(&mut self, qualname: &str) {
        if self.definitions.contains_key(qualname) {
            return;
        }

        let top_level = qualname.split('.').next().unwrap_or(qualname);

        // Special handling for builtins like "builtins.print", "builtins.len", etc.
        if qualname.starts_with("builtins.") {
            // Create builtin parent module if it doesn't exist
            if !self.definitions.contains_key("builtins") {
                let mut builtins_module = NodeData::new("builtins", ObjectType::Module);
                builtins_module.origin = Some(Origin::Standard);
                self.definitions.insert("builtins".to_string(), builtins_module);
                self.register_node_in_hierarchy("builtins");
            }

            // Create the builtin function as a child of builtins
            let mut node = NodeData::new(qualname, ObjectType::Function);
            node.origin = Some(Origin::Standard);
            self.definitions.insert(qualname.to_string(), node);
            self.register_node_in_hierarchy(qualname);
            return;
        }

        // Determine object type based on what we know
        let object_type = if self.lang_configs.iter().any(|c| c.is_stdlib(top_level)) && !qualname.contains('.') {
            ObjectType::Module
        } else if qualname.contains('.') {
            // Sub-module or function like math.sqrt
            ObjectType::Unknown
        } else {
            ObjectType::Module
        };

        let mut node = NodeData::new(qualname, object_type);
        node.origin = Some(self.classify_module(qualname));
        self.definitions.insert(qualname.to_string(), node);
        self.ensure_parent_nodes(qualname);
        self.register_node_in_hierarchy(qualname);
    }

    /// Convert a file path to a module qualname
    pub(crate) fn file_to_module(&self, file_path: &str) -> String {
        self.lang_configs
            .iter()
            .find(|cfg| cfg.extensions().iter().any(|ext| file_path.ends_with(ext)))
            .map(|cfg| cfg.derive_module_path(file_path, ""))
            .unwrap_or_else(|| file_path.to_string())
    }
}

fn is_python_builtin(name: &str) -> bool {
    matches!(
        name,
        "print"
            | "len"
            | "range"
            | "str"
            | "int"
            | "list"
            | "dict"
            | "set"
            | "tuple"
            | "type"
            | "open"
            | "isinstance"
            | "issubclass"
            | "getattr"
            | "setattr"
            | "hasattr"
            | "delattr"
            | "super"
            | "property"
            | "staticmethod"
            | "classmethod"
            | "enumerate"
            | "zip"
            | "map"
            | "filter"
            | "sorted"
            | "reversed"
            | "min"
            | "max"
            | "sum"
            | "abs"
            | "round"
            | "hash"
            | "id"
            | "repr"
            | "format"
            | "input"
            | "bool"
            | "float"
            | "complex"
            | "bytes"
            | "bytearray"
            | "memoryview"
            | "object"
            | "frozenset"
            | "iter"
            | "next"
            | "callable"
            | "vars"
            | "dir"
            | "globals"
            | "locals"
            | "exec"
            | "eval"
            | "compile"
            | "breakpoint"
            | "Exception"
            | "ValueError"
            | "TypeError"
            | "KeyError"
            | "IndexError"
            | "AttributeError"
            | "ImportError"
            | "RuntimeError"
            | "StopIteration"
            | "NotImplementedError"
            | "FileNotFoundError"
            | "OSError"
            | "IOError"
            | "PermissionError"
    )
}
