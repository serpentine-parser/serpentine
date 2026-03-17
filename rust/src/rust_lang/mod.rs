//! Rust language walker for serpentine.
//!
//! Walks a tree-sitter parse tree for Rust source files and emits
//! semantic events consumed by the subscriber pipeline.

pub mod config;

use crate::events::{Event, ScopeType};
use std::path::Path;
use tree_sitter::{Node, Tree};

// ============================================================================
// Entry Point
// ============================================================================

pub fn parse(source: &str, tree: &Option<Tree>, file_path: &str) -> Vec<Event> {
    let mut events = Vec::new();

    // Emit source line events first so subscribers can capture raw source
    for (i, line) in source.lines().enumerate() {
        events.push(Event::SourceLine {
            file: file_path.to_string(),
            line_number: i + 1,
            text: line.to_string(),
        });
    }

    if let Some(tree) = tree {
        let root = tree.root_node();
        let module_path = derive_module_path(file_path);
        let crate_root = module_path.first().cloned().unwrap_or_else(|| "crate".to_string());
        let current_module = module_path.join(".");

        let ctx = ParseContext {
            source,
            file_path,
            module_path: module_path.clone(),
            crate_root,
            current_module,
        };

        // Emit module-level scope so subscribers have an anchor for this file
        let module_name = module_path.last().cloned().unwrap_or_else(|| "module".to_string());
        let module_qualname = module_path.join(".");

        if is_valid_qualname(&module_qualname) {
            events.push(Event::enter_scope(
                ScopeType::Module,
                module_name.clone(),
                module_qualname.clone(),
                vec![],
                vec![],
                root,
                file_path,
            ));
            events.push(Event::define_name(
                module_name.clone(),
                module_qualname.clone(),
                "module",
                root,
                file_path,
            ));
        }

        walk_node(&ctx, root, &mut events);

        if is_valid_qualname(&module_qualname) {
            events.push(Event::exit_scope(
                ScopeType::Module,
                module_name,
                module_qualname,
                root,
                file_path,
            ));
        }
    }

    events
}

// ============================================================================
// Module Path Derivation
// ============================================================================

/// Derive a Rust module path from a file path.
///
/// Walks up to find `Cargo.toml`, uses its parent directory name as the crate root,
/// then converts the path relative to `src/` into a dotted module path.
///
/// Examples:
///   my_crate/src/lib.rs       → ["my_crate"]
///   my_crate/src/foo.rs       → ["my_crate", "foo"]
///   my_crate/src/foo/mod.rs   → ["my_crate", "foo"]
///   my_crate/src/foo/bar.rs   → ["my_crate", "foo", "bar"]
pub fn derive_module_path(file_path: &str) -> Vec<String> {
    let path = Path::new(file_path);

    // Walk up to find Cargo.toml
    let mut cargo_dir: Option<std::path::PathBuf> = None;
    let mut current = path.parent();
    while let Some(dir) = current {
        if dir.join("Cargo.toml").exists() {
            cargo_dir = Some(dir.to_path_buf());
            break;
        }
        current = dir.parent();
    }

    let crate_dir = match cargo_dir {
        Some(dir) => dir,
        None => {
            // Fallback: use file stem as module name
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown");
            return vec![stem.to_string()];
        }
    };

    // Crate name: directory containing Cargo.toml, hyphens → underscores
    let crate_name = crate_dir
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("crate")
        .replace('-', "_");

    // Prefer path relative to <crate>/src/, fall back to <crate>/
    let src_dir = crate_dir.join("src");
    let rel = if let Ok(r) = path.strip_prefix(&src_dir) {
        r.to_path_buf()
    } else if let Ok(r) = path.strip_prefix(&crate_dir) {
        r.to_path_buf()
    } else {
        path.to_path_buf()
    };

    let stem = rel.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown");

    let mut segments = vec![crate_name];

    // Add directory components between src/ and the file
    if let Some(parent) = rel.parent() {
        for component in parent.components() {
            if let std::path::Component::Normal(s) = component {
                if let Some(s) = s.to_str() {
                    if !s.is_empty() {
                        segments.push(s.to_string());
                    }
                }
            }
        }
    }

    // Special stems: main, lib, mod → directory is the module; don't append stem
    match stem {
        "main" | "lib" | "mod" => {}
        _ => segments.push(stem.to_string()),
    }

    segments
}

// ============================================================================
// Parse Context
// ============================================================================

struct ParseContext<'a> {
    source: &'a str,
    file_path: &'a str,
    module_path: Vec<String>,
    crate_root: String,
    current_module: String,
}

impl<'a> ParseContext<'a> {
    fn get_text(&self, node: Node) -> String {
        self.source[node.start_byte()..node.end_byte()].to_string()
    }
}

// ============================================================================
// Main Walker
// ============================================================================

fn walk_node(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    let kind = node.kind();

    // ── Special-cased nodes that fully handle their own subtree ──────────────

    if kind == "use_declaration" {
        emit_use_events(ctx, node, events);
        return;
    }

    // Qualified paths (e.g. `std::io::BufReader`, `crate::module::Type`).
    // Emit a single UseName for the full dotted path, then return early so
    // that we don't recurse into the sub-path children (which would produce
    // redundant partial-path events).
    if kind == "scoped_identifier" || kind == "scoped_type_identifier" {
        if let Some(parent) = node.parent() {
            let pk = parent.kind();
            // Skip if we're already a sub-path of an outer scoped node
            // (the outer one emits the complete path) or inside a use tree.
            if pk != "scoped_identifier"
                && pk != "scoped_type_identifier"
                && !is_inside_use_declaration(node)
            {
                let full_text = ctx.get_text(node).replace("::", ".");
                events.push(Event::use_name(full_text, node, ctx.file_path));
            }
        }
        return;
    }

    if kind == "match_arm" {
        // ElseBlock for the arm pattern, walk body, then EndControlBlock
        let pattern = node
            .child_by_field_name("pattern")
            .map(|n| ctx.get_text(n))
            .unwrap_or_default();
        events.push(Event::else_block("arm", pattern, node, ctx.file_path));
        if let Some(value) = node.child_by_field_name("value") {
            walk_node(ctx, value, events);
        }
        events.push(Event::end_control_block("arm", node, ctx.file_path));
        return;
    }

    if kind == "closure_expression" {
        // Treat closures as anonymous lambda scopes (mirrors Python lambda handling).
        let pos = node.start_position();
        let closure_name = format!("<closure:{}>", pos.row + 1);
        let qualname = build_qualname(ctx, node, &closure_name);

        // Collect closure parameter names and emit DefineName for each
        let mut params = Vec::new();
        if let Some(params_node) = node.child_by_field_name("parameters") {
            let mut cursor = params_node.walk();
            for child in params_node.children(&mut cursor) {
                if child.kind() == "closure_parameter" {
                    if let Some(pat) = child.child_by_field_name("pattern") {
                        let raw = ctx.get_text(pat);
                        let pname = raw.trim_start_matches("mut ").trim().to_string();
                        if !pname.is_empty() && pname != "_" {
                            params.push(pname.clone());
                            let param_qualname = build_qualname(ctx, child, &pname);
                            events.push(Event::define_name(
                                pname,
                                param_qualname,
                                "variable",
                                child,
                                ctx.file_path,
                            ));
                        }
                    }
                }
            }
        }

        events.push(Event::define_name(
            closure_name.clone(),
            qualname.clone(),
            "function",
            node,
            ctx.file_path,
        ));
        events.push(Event::enter_scope(
            ScopeType::Lambda,
            closure_name.clone(),
            qualname.clone(),
            params,
            vec![],
            node,
            ctx.file_path,
        ));

        if let Some(body) = node.child_by_field_name("body") {
            walk_node(ctx, body, events);
        }

        events.push(Event::exit_scope(
            ScopeType::Lambda,
            closure_name,
            qualname,
            node,
            ctx.file_path,
        ));
        return;
    }

    // ── Compute scope exit info (entry events emitted inline below) ──────────

    let scope_exit: Option<(ScopeType, String, String)> = match kind {
        "function_item" => node.child_by_field_name("name").map(|n| {
            let name = ctx.get_text(n);
            let qualname = build_qualname(ctx, node, &name);
            (ScopeType::Function, name, qualname)
        }),
        "struct_item" | "enum_item" => node.child_by_field_name("name").map(|n| {
            let name = ctx.get_text(n);
            let qualname = build_qualname(ctx, node, &name);
            (ScopeType::Class, name, qualname)
        }),
        "trait_item" => node.child_by_field_name("name").map(|n| {
            let name = ctx.get_text(n);
            let qualname = build_qualname(ctx, node, &name);
            (ScopeType::Interface, name, qualname)
        }),
        "impl_item" => node.child_by_field_name("type").map(|t| {
            let raw = ctx.get_text(t);
            let name = strip_generics(&raw);
            let qualname = build_qualname(ctx, node, &name);
            (ScopeType::Class, name, qualname)
        }),
        "mod_item" if node.child_by_field_name("body").is_some() => {
            node.child_by_field_name("name").map(|n| {
                let name = ctx.get_text(n);
                let qualname = build_qualname(ctx, node, &name);
                (ScopeType::Module, name, qualname)
            })
        }
        _ => None,
    };

    // ── Compute control block info ───────────────────────────────────────────

    let control_exit: Option<&str> = match kind {
        "if_expression" => Some("if"),
        "match_expression" => Some("match"),
        "loop_expression" => Some("loop"),
        "while_expression" => Some("while"),
        "for_expression" => Some("for"),
        _ => None,
    };

    // ── Emit entry events ────────────────────────────────────────────────────

    if let Some((ref scope_type, ref name, ref qualname)) = scope_exit {
        if is_valid_qualname(qualname) {
            let (params, bases) = match kind {
                "function_item" => {
                    let params = extract_fn_params(ctx, node);
                    (params, vec![])
                }
                "impl_item" => {
                    let bases = node
                        .child_by_field_name("trait")
                        .map(|t| vec![strip_generics(&ctx.get_text(t))])
                        .unwrap_or_default();
                    (vec![], bases)
                }
                _ => (vec![], vec![]),
            };

            // For impl_item we don't emit DefineName — struct already defined it
            if kind != "impl_item" {
                let node_type = match scope_type {
                    ScopeType::Function => "function",
                    ScopeType::Class => "class",
                    ScopeType::Interface => "interface",
                    ScopeType::Module => "module",
                    _ => "class",
                };
                events.push(Event::define_name(
                    name.clone(),
                    qualname.clone(),
                    node_type,
                    node,
                    ctx.file_path,
                ));
            }

            events.push(Event::enter_scope(
                scope_type.clone(),
                name.clone(),
                qualname.clone(),
                params,
                bases,
                node,
                ctx.file_path,
            ));
        }
    }

    if let Some(block_type) = control_exit {
        let condition = match kind {
            "if_expression" => node
                .child_by_field_name("condition")
                .map(|c| ctx.get_text(c))
                .unwrap_or_default(),
            "match_expression" => node
                .child_by_field_name("value")
                .map(|v| ctx.get_text(v))
                .unwrap_or_default(),
            "while_expression" => node
                .child_by_field_name("condition")
                .map(|c| ctx.get_text(c))
                .unwrap_or_default(),
            "for_expression" => {
                let pat = node
                    .child_by_field_name("pattern")
                    .map(|p| ctx.get_text(p))
                    .unwrap_or_default();
                let val = node
                    .child_by_field_name("value")
                    .map(|v| ctx.get_text(v))
                    .unwrap_or_default();
                format!("{} in {}", pat, val)
            }
            _ => String::new(),
        };
        events.push(Event::control_block(block_type, condition, node, ctx.file_path));
    }

    // ── Emit other node-specific events ─────────────────────────────────────

    match kind {
        "struct_item" => emit_struct_field_events(ctx, node, events),
        "enum_item" => emit_enum_variant_events(ctx, node, events),
        "mod_item" if node.child_by_field_name("body").is_none() => {
            emit_mod_file_ref(ctx, node, events);
        }
        "let_declaration" => emit_let_events(ctx, node, events),
        "assignment_expression" => emit_assignment_events(ctx, node, events),
        "compound_assignment_expr" => emit_compound_assignment_events(ctx, node, events),
        // `if let` / `while let` pattern — bind all identifiers introduced by the pattern.
        "let_condition" => emit_let_condition_events(ctx, node, events),
        "call_expression" => emit_call_events(ctx, node, events),
        "method_call_expression" => emit_method_call_events(ctx, node, events),
        "macro_invocation" => emit_macro_invocation_events(ctx, node, events),
        "return_expression" => emit_return_events(ctx, node, events),
        "break_expression" => {
            events.push(Event::break_statement(node, ctx.file_path));
        }
        "continue_expression" => {
            events.push(Event::continue_statement(node, ctx.file_path));
        }
        "try_expression" => {
            // expr? — models the implicit Err early-return path
            events.push(Event::return_stmt(
                "?".to_string(),
                "early_return",
                node,
                ctx.file_path,
            ));
        }
        "field_expression" => emit_field_expression_events(ctx, node, events),
        "identifier" | "type_identifier" => emit_identifier_events(ctx, node, events),
        "else_clause" => {
            // if/while else branch
            events.push(Event::else_block("else", String::new(), node, ctx.file_path));
        }
        "integer_literal" => {
            events.push(Event::literal(ctx.get_text(node), "integer", node, ctx.file_path));
        }
        "float_literal" => {
            events.push(Event::literal(ctx.get_text(node), "float", node, ctx.file_path));
        }
        "boolean_literal" => {
            events.push(Event::literal(ctx.get_text(node), "boolean", node, ctx.file_path));
        }
        "string_literal" | "char_literal" | "raw_string_literal" => {
            events.push(Event::literal(ctx.get_text(node), "string", node, ctx.file_path));
        }
        _ => {}
    }

    // ── Walk children ────────────────────────────────────────────────────────
    {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            walk_node(ctx, child, events);
        }
    }

    // ── Emit exit events (post-order) ────────────────────────────────────────

    if let Some(block_type) = control_exit {
        events.push(Event::end_control_block(block_type, node, ctx.file_path));
    }
    if let Some((scope_type, name, qualname)) = scope_exit {
        if is_valid_qualname(&qualname) {
            events.push(Event::exit_scope(scope_type, name, qualname, node, ctx.file_path));
        }
    }
}

// ============================================================================
// Node-Specific Emitters
// ============================================================================

/// Emit DefineName events for struct fields.
fn emit_struct_field_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    // Named struct: field_declaration_list → field_declaration(name, type)
    // Tuple struct: ordered_field_declaration_list → ordered_field_declaration
    if let Some(body) = node.child_by_field_name("body") {
        match body.kind() {
            "field_declaration_list" => {
                let mut cursor = body.walk();
                for child in body.children(&mut cursor) {
                    if child.kind() == "field_declaration" {
                        if let Some(name_node) = child.child_by_field_name("name") {
                            let name = ctx.get_text(name_node);
                            let qualname = build_qualname(ctx, child, &name);
                            if is_valid_qualname(&qualname) {
                                events.push(Event::define_name(
                                    name,
                                    qualname,
                                    "variable",
                                    child,
                                    ctx.file_path,
                                ));
                            }
                        }
                    }
                }
            }
            "ordered_field_declaration_list" => {
                let mut cursor = body.walk();
                let mut idx = 0usize;
                for child in body.children(&mut cursor) {
                    if child.kind() == "ordered_field_declaration" {
                        let name = idx.to_string();
                        let qualname = build_qualname(ctx, child, &name);
                        if is_valid_qualname(&qualname) {
                            events.push(Event::define_name(
                                name,
                                qualname,
                                "variable",
                                child,
                                ctx.file_path,
                            ));
                        }
                        idx += 1;
                    }
                }
            }
            _ => {}
        }
    }
}

/// Emit DefineName events for enum variants.
fn emit_enum_variant_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    if let Some(body) = node.child_by_field_name("body") {
        if body.kind() == "enum_variant_list" {
            let mut cursor = body.walk();
            for child in body.children(&mut cursor) {
                if child.kind() == "enum_variant" {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = ctx.get_text(name_node);
                        let qualname = build_qualname(ctx, child, &name);
                        if is_valid_qualname(&qualname) {
                            events.push(Event::define_name(
                                name,
                                qualname,
                                "variable",
                                child,
                                ctx.file_path,
                            ));
                        }
                    }
                }
            }
        }
    }
}

/// Emit ImportStatement for `mod foo;` (file-reference module declaration).
fn emit_mod_file_ref(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    if let Some(name_node) = node.child_by_field_name("name") {
        let name = ctx.get_text(name_node);
        // Build the sibling module path
        let parent_module = ctx.module_path.join(".");
        let sibling_module = if parent_module.is_empty() {
            name.clone()
        } else {
            format!("{}.{}", parent_module, name)
        };
        events.push(Event::import_statement(
            sibling_module,
            vec![],
            std::collections::HashMap::new(),
            false,
            node,
            ctx.file_path,
        ));
    }
}

/// Emit ImportStatement events for `use` declarations (all forms).
fn emit_use_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    if let Some(arg) = node.child_by_field_name("argument") {
        let imports = collect_use_items(ctx, arg, &[]);
        for (module, names, aliases) in imports {
            events.push(Event::import_statement(
                module,
                names,
                aliases,
                false,
                node,
                ctx.file_path,
            ));
        }
    }
}

/// Emit Assignment + DefineName for `let` bindings.
fn emit_let_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    let value_node = node.child_by_field_name("value");
    // Normalize `::` path separators to `.` so that the graph builder's
    // extract_callable / resolve_callee can match against dot-notation qualnames.
    let value_text = value_node
        .map(|v| ctx.get_text(v).replace("::", "."))
        .unwrap_or_default();
    let value_type = value_node.map(|v| classify_value_type(v.kind())).unwrap_or("none");

    if let Some(pat) = node.child_by_field_name("pattern") {
        emit_let_pattern(ctx, pat, node, &value_text, value_type, events);
    }
}

/// Recursively emit DefineName + Assignment for let patterns.
fn emit_let_pattern(
    ctx: &ParseContext,
    pat: Node,
    binding_node: Node,
    value_text: &str,
    value_type: &str,
    events: &mut Vec<Event>,
) {
    match pat.kind() {
        "identifier" | "mutable_specifier" => {
            // Simple: `let x = ...` or `let mut x = ...`
            // mutable_specifier wraps the identifier in some grammars
            let name = ctx.get_text(pat);
            if name == "mut" {
                return; // just the keyword, not a name
            }
            let qualname = build_qualname(ctx, binding_node, &name);
            if is_valid_qualname(&qualname) {
                events.push(Event::define_name(
                    name.clone(),
                    qualname.clone(),
                    "variable",
                    pat,
                    ctx.file_path,
                ));
                events.push(Event::assignment(
                    name,
                    qualname,
                    value_text.to_string(),
                    value_type,
                    binding_node,
                    ctx.file_path,
                ));
            }
        }
        "tuple_pattern" | "tuple_struct_pattern" => {
            // Destructuring: `let (a, b) = ...`
            let mut cursor = pat.walk();
            for child in pat.children(&mut cursor) {
                if child.kind() == "identifier" {
                    let name = ctx.get_text(child);
                    let qualname = build_qualname(ctx, binding_node, &name);
                    if is_valid_qualname(&qualname) {
                        events.push(Event::define_name(
                            name.clone(),
                            qualname.clone(),
                            "variable",
                            child,
                            ctx.file_path,
                        ));
                        events.push(Event::assignment(
                            name,
                            qualname,
                            value_text.to_string(),
                            value_type,
                            binding_node,
                            ctx.file_path,
                        ));
                    }
                }
            }
        }
        _ => {
            // For complex patterns, just try to find identifiers
            let mut cursor = pat.walk();
            for child in pat.children(&mut cursor) {
                if child.kind() == "identifier" {
                    let name = ctx.get_text(child);
                    let qualname = build_qualname(ctx, binding_node, &name);
                    if is_valid_qualname(&qualname) {
                        events.push(Event::define_name(
                            name.clone(),
                            qualname.clone(),
                            "variable",
                            child,
                            ctx.file_path,
                        ));
                    }
                }
            }
        }
    }
}

/// Emit UseName (read) + Assignment for compound assignments (`+=`, `-=`, etc.).
fn emit_compound_assignment_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    if let Some(left) = node.child_by_field_name("left") {
        // LHS is both read and written — emit UseName for the read side.
        let left_text = ctx.get_text(left).replace("::", ".");
        events.push(Event::use_name(left_text.clone(), left, ctx.file_path));

        if let Some(right) = node.child_by_field_name("right") {
            let value = ctx.get_text(right).replace("::", ".");
            let value_type = classify_value_type(right.kind());
            let qualname = build_qualname(ctx, node, &left_text);
            events.push(Event::assignment(
                left_text,
                qualname,
                value,
                value_type,
                node,
                ctx.file_path,
            ));
        }
    }
}

/// Emit DefineName for all identifiers bound by the pattern in an `if let` /
/// `while let` condition (`if let Some(x) = expr`), and walk the value expression.
fn emit_let_condition_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    // Walk the value expression first so its use/call events appear before the binding.
    if let Some(value) = node.child_by_field_name("value") {
        walk_node(ctx, value, events);
    }
    // Bind all names introduced by the pattern.
    if let Some(pat) = node.child_by_field_name("pattern") {
        emit_let_pattern(ctx, pat, node, "", "none", events);
    }
}

/// Emit Assignment for `x = expr` (reassignment).
fn emit_assignment_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    if let Some(left) = node.child_by_field_name("left") {
        if let Some(right) = node.child_by_field_name("right") {
            let target = ctx.get_text(left);
            let value = ctx.get_text(right);
            let value_type = classify_value_type(right.kind());
            let qualname = build_qualname(ctx, node, &target);
            events.push(Event::assignment(
                target,
                qualname,
                value,
                value_type,
                node,
                ctx.file_path,
            ));
        }
    }
}

/// Emit CallExpression for `foo(args)` and `foo::bar(args)`.
fn emit_call_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    if let Some(func_node) = node.child_by_field_name("function") {
        let callee_raw = ctx.get_text(func_node);
        let callee = callee_raw.replace("::", ".");

        let arguments = collect_arguments(ctx, node);
        events.push(Event::call_expression(callee, arguments, node, ctx.file_path));
    }
}

/// Emit AttributeAccess + CallExpression for `obj.method(args)`.
fn emit_method_call_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    let receiver = node
        .child_by_field_name("receiver")
        .map(|r| ctx.get_text(r))
        .unwrap_or_default();
    let method = node
        .child_by_field_name("name")
        .map(|m| ctx.get_text(m))
        .unwrap_or_default();

    if !receiver.is_empty() && !method.is_empty() {
        events.push(Event::attribute_access(
            receiver.clone(),
            method.clone(),
            node,
            ctx.file_path,
        ));
        let callee = format!("{}.{}", receiver, method);
        let arguments = collect_arguments(ctx, node);
        events.push(Event::call_expression(callee, arguments, node, ctx.file_path));
    }
}

/// Emit CallExpression or RaiseStatement for macro invocations.
fn emit_macro_invocation_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    // The macro name is in the `macro` field or first identifier child before `!`
    let macro_name = node
        .child_by_field_name("macro")
        .map(|m| ctx.get_text(m))
        .or_else(|| {
            // Walk children to find identifier/scoped_identifier before "!"
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                match child.kind() {
                    "identifier" | "scoped_identifier" => return Some(ctx.get_text(child)),
                    "!" => break,
                    _ => {}
                }
            }
            None
        })
        .unwrap_or_default();

    // Normalize path separators
    let macro_name = macro_name.replace("::", ".");

    match macro_name.as_str() {
        "panic" => {
            events.push(Event::raise_statement(
                "panic".to_string(),
                node,
                ctx.file_path,
            ));
        }
        "todo" | "unimplemented" => {
            events.push(Event::raise_statement(
                macro_name.clone(),
                node,
                ctx.file_path,
            ));
        }
        _ => {
            if !macro_name.is_empty() {
                events.push(Event::call_expression(
                    macro_name,
                    vec![],
                    node,
                    ctx.file_path,
                ));
            }
        }
    }
}

/// Emit Return for `return expr` or bare `return`.
fn emit_return_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    // return_expression has an optional value child (not a named field in all grammars)
    let mut cursor = node.walk();
    let value_node = node
        .children(&mut cursor)
        .find(|c| c.kind() != "return" && c.is_named());

    let (value, value_type) = match value_node {
        Some(v) => (ctx.get_text(v), classify_value_type(v.kind())),
        None => (String::new(), "none"),
    };

    events.push(Event::return_stmt(value, value_type, node, ctx.file_path));
}

/// Emit AttributeAccess for `obj.field`.
fn emit_field_expression_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    let obj = node
        .child_by_field_name("value")
        .map(|v| ctx.get_text(v))
        .unwrap_or_default();
    let field = node
        .child_by_field_name("field")
        .map(|f| ctx.get_text(f))
        .unwrap_or_default();

    if !obj.is_empty() && !field.is_empty() {
        events.push(Event::attribute_access(obj, field, node, ctx.file_path));
    }
}

/// Emit UseName for standalone identifier references.
fn emit_identifier_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    if let Some(parent) = node.parent() {
        let parent_kind = parent.kind();

        // Skip definition names
        match parent_kind {
            "function_item" | "struct_item" | "enum_item" | "trait_item" | "mod_item"
            | "type_item" | "type_alias" => {
                if let Some(name_field) = parent.child_by_field_name("name") {
                    if name_field.id() == node.id() {
                        return;
                    }
                }
            }
            "field_declaration" | "enum_variant" => {
                if let Some(name_field) = parent.child_by_field_name("name") {
                    if name_field.id() == node.id() {
                        return;
                    }
                }
            }
            // Part of a path — the outer node handles it
            "scoped_identifier" | "scoped_type_identifier" | "scoped_use_list" | "use_as_clause" => return,
            // Field access object/field — handled by field_expression emitter
            "field_expression" => return,
            // Method call receiver/name — handled by method_call emitter
            "method_call_expression" => return,
            // `self` / variadic parameters have no resolvable type references.
            "self_parameter" | "variadic_parameter" => return,
            // For regular parameters, only skip the *pattern* (the bound name), not the
            // type annotation — e.g. `fn f(x: Foo)`: skip `x` but emit UseName for `Foo`.
            "parameter" => {
                if let Some(pattern_field) = parent.child_by_field_name("pattern") {
                    if pattern_field.id() == node.id() {
                        return;
                    }
                }
            }
            // Closure parameters — skip the bound name (pattern field), which is a definition
            "closure_parameter" => {
                if let Some(pat) = parent.child_by_field_name("pattern") {
                    if pat.id() == node.id() {
                        return;
                    }
                }
            }
            // Assignment target already handled
            "assignment_expression" => {
                if let Some(left) = parent.child_by_field_name("left") {
                    if left.id() == node.id() {
                        return;
                    }
                }
            }
            _ => {}
        }

        // Skip anything inside a use_declaration tree
        if is_inside_use_declaration(node) {
            return;
        }

        let name = ctx.get_text(node);
        events.push(Event::use_name(name, node, ctx.file_path));
    }
}

// ============================================================================
// `use` Tree Collector
// ============================================================================

/// Recursively collect `(module_dotted_path, imported_names)` pairs from a use tree node.
///
/// `prefix` accumulates path segments from outer scoped_use_list nodes.
fn collect_use_items(
    ctx: &ParseContext,
    node: Node,
    prefix: &[String],
) -> Vec<(String, Vec<String>, std::collections::HashMap<String, String>)> {
    match node.kind() {
        "identifier" => {
            let name = ctx.get_text(node);
            if prefix.is_empty() {
                vec![(String::new(), vec![name], std::collections::HashMap::new())]
            } else {
                let module = prefix.join(".");
                vec![(module, vec![name], std::collections::HashMap::new())]
            }
        }
        "self" => {
            // `use foo::self` — importing the module itself under the current prefix
            if !prefix.is_empty() {
                let mut parts = prefix.to_vec();
                let name = parts.pop().unwrap_or_default();
                let module = parts.join(".");
                vec![(module, vec![name], std::collections::HashMap::new())]
            } else {
                vec![(ctx.current_module.clone(), vec![], std::collections::HashMap::new())]
            }
        }
        "crate" => {
            // `use crate;` — unusual, just import the crate root
            let module = prefix.join(".");
            vec![(module, vec![ctx.crate_root.clone()], std::collections::HashMap::new())]
        }
        "scoped_identifier" => {
            // `a::b::C` — last segment is name, rest is module
            let raw_segs = get_path_segments(ctx, node);
            let mut all = prefix.to_vec();
            all.extend(raw_segs);
            let normalized = normalize_path_segments(all, &ctx.crate_root, &ctx.current_module);
            if normalized.is_empty() {
                return vec![];
            }
            let mut parts = normalized;
            let name = parts.pop().unwrap_or_default();
            let module = parts.join(".");
            vec![(module, vec![name], std::collections::HashMap::new())]
        }
        "scoped_use_list" => {
            // `a::b::{C, D}` — extend prefix with the path, recurse into list
            let mut new_prefix = prefix.to_vec();
            if let Some(path_node) = node.child_by_field_name("path") {
                let path_segs = get_path_segments(ctx, path_node);
                new_prefix.extend(path_segs);
                new_prefix = normalize_path_segments(
                    new_prefix,
                    &ctx.crate_root,
                    &ctx.current_module,
                );
            }
            node.child_by_field_name("list")
                .map(|list| collect_use_items(ctx, list, &new_prefix))
                .unwrap_or_default()
        }
        "use_list" => {
            // `{C, D, e::F}` — process each item with current prefix
            let mut results = Vec::new();
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                match child.kind() {
                    "{" | "}" | "," | "line_comment" | "block_comment" => {}
                    _ => results.extend(collect_use_items(ctx, child, prefix)),
                }
            }
            results
        }
        "use_as_clause" => {
            // `a::B as C` — import original path, bind locally as alias C
            if let Some(path_node) = node.child_by_field_name("path") {
                let mut items = collect_use_items(ctx, path_node, prefix);
                if let Some(alias_node) = node.child_by_field_name("alias") {
                    let alias = ctx.get_text(alias_node);
                    for (_, names, aliases) in items.iter_mut() {
                        for name in names.iter() {
                            aliases.insert(name.clone(), alias.clone());
                        }
                    }
                }
                items
            } else {
                vec![]
            }
        }
        "use_wildcard" => {
            // `a::b::*` — the entire node text includes `::*` at the end
            let text = ctx.get_text(node);
            let base = text
                .trim_end_matches('*')
                .trim_end_matches(':')
                .trim_end_matches(':');
            let module = if base.is_empty() {
                prefix.join(".")
            } else {
                let mut segs = prefix.to_vec();
                segs.extend(base.split("::").map(|s| s.to_string()));
                let segs =
                    normalize_path_segments(segs, &ctx.crate_root, &ctx.current_module);
                segs.join(".")
            };
            vec![(module, vec!["*".to_string()], std::collections::HashMap::new())]
        }
        _ => vec![],
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Build a qualified name for `name` by walking up the parent chain.
///
/// Finds enclosing scope-creating nodes (functions, structs, enums, traits,
/// impls, inline mods) and prepends their names, then prepends the module path.
fn build_qualname(ctx: &ParseContext, node: Node, name: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    if !name.is_empty() {
        parts.push(name.to_string());
    }

    let mut current = node.parent();
    while let Some(parent) = current {
        match parent.kind() {
            "function_item" | "struct_item" | "enum_item" | "trait_item" | "mod_item" => {
                if let Some(name_node) = parent.child_by_field_name("name") {
                    parts.insert(0, ctx.get_text(name_node));
                }
            }
            "impl_item" => {
                if let Some(type_node) = parent.child_by_field_name("type") {
                    parts.insert(0, strip_generics(&ctx.get_text(type_node)));
                }
            }
            _ => {}
        }
        current = parent.parent();
    }

    if !ctx.module_path.is_empty() {
        let mut full = ctx.module_path.clone();
        full.extend(parts);
        return full.join(".");
    }
    parts.join(".")
}

/// Extract parameter names from a `function_item` node.
fn extract_fn_params(ctx: &ParseContext, node: Node) -> Vec<String> {
    let mut params = Vec::new();
    if let Some(params_node) = node.child_by_field_name("parameters") {
        let mut cursor = params_node.walk();
        for child in params_node.children(&mut cursor) {
            match child.kind() {
                "parameter" => {
                    if let Some(pat) = child.child_by_field_name("pattern") {
                        let text = ctx.get_text(pat);
                        // Strip leading `mut ` if present
                        let name = text.trim_start_matches("mut ").trim().to_string();
                        if !name.is_empty() {
                            params.push(name);
                        }
                    }
                }
                "self_parameter" => {
                    params.push("self".to_string());
                }
                _ => {}
            }
        }
    }
    params
}

/// Collect top-level argument texts from a call or method_call arguments node.
fn collect_arguments(ctx: &ParseContext, node: Node) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(args_node) = node.child_by_field_name("arguments") {
        let mut cursor = args_node.walk();
        for child in args_node.children(&mut cursor) {
            match child.kind() {
                "(" | ")" | "," | "attribute_item" => {}
                _ if child.is_named() => {
                    args.push(ctx.get_text(child));
                }
                _ => {}
            }
        }
    }
    args
}

/// Recursively extract segments from a scoped_identifier or identifier node.
///
/// Returns raw segments (crate/super/self not yet resolved).
fn get_path_segments(ctx: &ParseContext, node: Node) -> Vec<String> {
    match node.kind() {
        "identifier" | "self" | "super" | "crate" => {
            vec![ctx.get_text(node)]
        }
        "scoped_identifier" => {
            let mut parts = Vec::new();
            if let Some(path) = node.child_by_field_name("path") {
                parts.extend(get_path_segments(ctx, path));
            }
            if let Some(name) = node.child_by_field_name("name") {
                parts.push(ctx.get_text(name));
            }
            parts
        }
        _ => vec![ctx.get_text(node)],
    }
}

/// Normalize path segments, resolving `crate`, `super`, and `self`.
fn normalize_path_segments(
    segments: Vec<String>,
    crate_root: &str,
    current_module: &str,
) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();
    for seg in segments {
        match seg.as_str() {
            "crate" => {
                result.clear();
                result.push(crate_root.to_string());
            }
            "super" => {
                if result.is_empty() {
                    // Resolve against current module
                    let mut parts: Vec<String> =
                        current_module.split('.').map(|s| s.to_string()).collect();
                    parts.pop();
                    result = parts;
                } else {
                    result.pop();
                }
            }
            "self" if result.is_empty() => {
                result = current_module.split('.').map(|s| s.to_string()).collect();
            }
            _ => result.push(seg),
        }
    }
    result
}

/// Reject qualnames that contain non-identifier characters.
/// Valid qualnames contain only [a-zA-Z0-9_.] — no parens, quotes, newlines, etc.
fn is_valid_qualname(qualname: &str) -> bool {
    !qualname.is_empty()
        && qualname.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.')
        && !qualname.starts_with('.')
        && !qualname.ends_with('.')
}

/// Strip generic parameters (`<T>`, `<T: Clone>`) from a type name.
fn strip_generics(name: &str) -> String {
    if let Some(pos) = name.find('<') {
        name[..pos].trim().to_string()
    } else {
        name.trim().to_string()
    }
}

/// Classify a value expression node kind into a semantic category.
fn classify_value_type(kind: &str) -> &'static str {
    match kind {
        "integer_literal" | "float_literal" | "boolean_literal" | "string_literal"
        | "char_literal" | "raw_string_literal" => "literal",
        "call_expression" | "method_call_expression" | "macro_invocation" => "call",
        "identifier" => "name",
        "field_expression" | "scoped_identifier" => "attribute",
        _ => "expression",
    }
}

/// Return true if `node` is anywhere inside a `use_declaration` subtree.
fn is_inside_use_declaration(node: Node) -> bool {
    let mut current = node.parent();
    while let Some(parent) = current {
        if parent.kind() == "use_declaration" {
            return true;
        }
        current = parent.parent();
    }
    false
}
