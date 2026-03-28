pub mod config;

use crate::events::{generate_node_id, Event, ScopeType};
use tree_sitter::{Node, Tree};

// ============================================================================
// Docstring extraction helpers
// ============================================================================

/// Extract the docstring from a function or class body node.
/// Looks for the first child of the body that is an `expression_statement`
/// containing a `string` literal.
fn extract_body_docstring(ctx: &ParseContext, node: Node) -> Option<String> {
    let body = node.child_by_field_name("body")?;
    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        match child.kind() {
            "decorator" | "comment" => continue,
            "expression_statement" => {
                let mut c2 = child.walk();
                for inner in child.children(&mut c2) {
                    if inner.kind() == "string" {
                        let raw = get_node_text(ctx, inner);
                        return Some(clean_docstring(&raw));
                    }
                }
                return None;
            }
            _ => return None,
        }
    }
    None
}

/// Extract a module-level docstring from the root `module` node.
fn extract_module_docstring(ctx: &ParseContext, root: Node) -> Option<String> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        match child.kind() {
            "comment" => continue,
            "expression_statement" => {
                let mut c2 = child.walk();
                for inner in child.children(&mut c2) {
                    if inner.kind() == "string" {
                        let raw = get_node_text(ctx, inner);
                        return Some(clean_docstring(&raw));
                    }
                }
                return None;
            }
            _ => return None,
        }
    }
    None
}

/// Strip outer quotes from a Python string literal and dedent the content.
fn clean_docstring(raw: &str) -> String {
    let inner = if (raw.starts_with("\"\"\"") && raw.ends_with("\"\"\"") && raw.len() >= 6)
        || (raw.starts_with("'''") && raw.ends_with("'''") && raw.len() >= 6)
    {
        &raw[3..raw.len() - 3]
    } else if (raw.starts_with('"') && raw.ends_with('"') && raw.len() >= 2)
        || (raw.starts_with('\'') && raw.ends_with('\'') && raw.len() >= 2)
    {
        &raw[1..raw.len() - 1]
    } else {
        raw
    };
    dedent(inner.trim_matches('\n'))
}

/// Remove uniform leading whitespace from all non-empty lines (textwrap.dedent).
fn dedent(s: &str) -> String {
    let lines: Vec<&str> = s.split('\n').collect();
    let indent = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    lines
        .iter()
        .map(|l| if l.len() >= indent { &l[indent..] } else { l.trim_start() })
        .collect::<Vec<_>>()
        .join("\n")
        .trim_end()
        .to_string()
}

pub fn parse(source: &str, tree: &Option<Tree>, file_path: &str) -> Vec<Event> {
    let mut events = Vec::new();

    // Emit source line events first so subscribers can capture the raw source
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
        let is_init = std::path::Path::new(file_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s == "__init__")
            .unwrap_or(false);
        let context = ParseContext {
            source,
            file_path,
            module_path,
            is_init,
        };

        // Emit module-level EnterScope so the docstring and scope range are captured.
        let module_qualname = context.module_path.join(".");
        let module_name = context.module_path.last().cloned().unwrap_or_else(|| "module".to_string());
        let module_docstring = extract_module_docstring(&context, root);
        events.push(Event::enter_scope_with_docstring(
            ScopeType::Module,
            module_name.clone(),
            module_qualname.clone(),
            vec![],
            vec![],
            module_docstring,
            root,
            file_path,
        ));

        walk_node_python(&context, root, &mut events);

        events.push(Event::exit_scope(
            ScopeType::Module,
            module_name,
            module_qualname,
            root,
            file_path,
        ));
    }
    events
}

/// Derive a Python module path from a file path.
///
/// Walks up the directory tree collecting only directories that are actual
/// Python packages (i.e., contain an `__init__.py` file).
///
/// e.g., "/path/to/project/src/serpentine/adapters/serialize.py" -> ["serpentine", "adapters", "serialize"]
///   (src/ has no __init__.py, so it is not included)
/// e.g., "/path/to/test_package/app.py" -> ["test_package", "app"]
pub fn derive_module_path(file_path: &str) -> Vec<String> {
    use std::path::Path;

    let path = Path::new(file_path);

    // Get the file stem (without .py extension)
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("__main__");

    // For __init__.py files, the module is the package itself (the parent directory).
    let is_init = stem == "__init__";

    // Walk up the directory tree collecting package names.
    // Include a directory only if it contains __init__.py (i.e., it is an actual Python package).
    let mut parts = Vec::new();
    let mut current = path.parent();

    while let Some(dir) = current {
        if let Some(dir_name) = dir.file_name().and_then(|s| s.to_str()) {
            // Stop at hidden directories (like .venv, .git)
            if dir_name.starts_with('.') {
                break;
            }

            // Stop at empty names (filesystem root)
            if dir_name.is_empty() {
                break;
            }

            // Stop at site-packages/dist-packages (installed packages boundary)
            if matches!(dir_name, "site-packages" | "dist-packages") {
                break;
            }

            // Stop if this directory is not a Python package (no __init__.py)
            if !dir.join("__init__.py").exists() {
                break;
            }

            // This is a Python package directory, include it
            parts.push(dir_name.to_string());
        } else {
            break;
        }

        current = dir.parent();
    }

    // Reverse since we collected from leaf to root
    parts.reverse();

    // Add the file stem (module name) — but NOT for __init__.py files,
    // since the package identity is the directory itself.
    if !is_init {
        parts.push(stem.to_string());
    }

    parts
}

struct ParseContext<'a> {
    source: &'a str,
    file_path: &'a str,
    module_path: Vec<String>,
    is_init: bool,
}

fn walk_node_python(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    let kind = node.kind();

    // Track if this node creates a scope (we need to emit exit after children)
    let scope_info = match kind {
        "class_definition" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = get_node_text(ctx, name_node);
                let qualname = build_qualname(ctx, node, &name);
                Some((ScopeType::Class, name, qualname))
            } else {
                None
            }
        }
        "function_definition" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = get_node_text(ctx, name_node);
                let qualname = build_qualname(ctx, node, &name);
                Some((ScopeType::Function, name, qualname))
            } else {
                None
            }
        }
        "lambda" => {
            let pos = node.start_position();
            let name = format!("<lambda:{}>", pos.row + 1);
            let qualname = build_qualname(ctx, node, &name);
            Some((ScopeType::Lambda, name, qualname))
        }
        _ => None,
    };

    // Track whether this is an assignment node — Assignment event is emitted POST
    // (after children are walked) so that the RHS expression events (CallExpression,
    // UseName) fire before the assignment node is created in the PDG.
    let is_assignment = kind == "assignment";
    let is_annotated_assignment = kind == "annotated_assignment";

    // Call events are also emitted POST so that nested calls (arguments evaluated
    // before the outer call) fire in the correct execution order.
    // e.g. car.drive(math.sqrt(25)): math.sqrt fires first (argument), then car.drive.
    let is_call = kind == "call";

    // Track control block info for emitting end events
    let control_block_info = match kind {
        "if_statement" | "for_statement" | "while_statement" | "try_statement"
        | "with_statement" | "async_for_statement" | "async_with_statement" => Some(kind),
        _ => None,
    };

    // Emit events based on node type
    match kind {
        "import_statement" | "import_from_statement" => emit_import_events(ctx, node, events),
        "class_definition" => emit_class_events(ctx, node, events),
        "function_definition" => emit_function_events(ctx, node, events),
        "assignment" => emit_assignment_define_events(ctx, node, events),
        "annotated_assignment" => emit_annotated_assignment_define_events(ctx, node, events),
        "type_alias_statement" => emit_type_alias_events(ctx, node, events),
        "identifier" => emit_identifier_events(ctx, node, events),
        "attribute" => emit_attribute_events(ctx, node, events),
        "if_statement" | "for_statement" | "while_statement" | "try_statement"
        | "with_statement" => emit_control_block_events(ctx, node, events, kind),
        "else_clause" => emit_else_block_events(ctx, node, events),
        "elif_clause" => emit_elif_block_events(ctx, node, events),
        "return_statement" => emit_return_events(ctx, node, events),
        "break_statement" => emit_break_events(ctx, node, events),
        "continue_statement" => emit_continue_events(ctx, node, events),
        "raise_statement" => emit_raise_events(ctx, node, events),
        "string" | "integer" | "float" | "true" | "false" | "none" => {
            emit_literal_events(ctx, node, events, kind)
        }
        "decorator" => emit_decorator_events(ctx, node, events),
        "match_statement" => emit_match_events(ctx, node, events),
        "case_clause" => emit_case_clause_events(ctx, node, events),
        "named_expression" => emit_named_expression_events(ctx, node, events),
        "list_comprehension" | "dictionary_comprehension" | "set_comprehension"
        | "generator_expression" => emit_comprehension_events(ctx, node, events, kind),
        "lambda" => emit_lambda_events(ctx, node, events),
        "augmented_assignment" => emit_augmented_assignment_events(ctx, node, events),
        "global_statement" | "nonlocal_statement" => {
            emit_global_nonlocal_events(ctx, node, events, kind)
        }
        "assert_statement" => emit_assert_events(ctx, node, events),
        "delete_statement" => emit_delete_events(ctx, node, events),
        "yield" => emit_yield_events(ctx, node, events),
        "async_for_statement" => emit_control_block_events(ctx, node, events, "async_for"),
        "async_with_statement" => emit_control_block_events(ctx, node, events, "async_with"),
        "except_clause" => emit_except_clause_events(ctx, node, events),
        _ => {}
    }

    // Walk all children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_node_python(ctx, child, events);
    }

    // Emit Assignment event after children so the RHS call/use events fire first,
    // giving correct PDG ordering: build_car → car, not car → build_car.
    if is_assignment {
        emit_assignment_post_event(ctx, node, events);
    }
    if is_annotated_assignment {
        emit_annotated_assignment_post_event(ctx, node, events);
    }

    // Emit Call event after children so nested argument calls fire first
    // (execution order: inner calls before outer call).
    if is_call {
        emit_call_events(ctx, node, events);
    }

    // Emit end_control_block after walking children
    if let Some(block_kind) = control_block_info {
        let block_type = match block_kind {
            "if_statement" => "if",
            "for_statement" => "for",
            "while_statement" => "while",
            "try_statement" => "try",
            "with_statement" => "with",
            "async_for_statement" => "async_for",
            "async_with_statement" => "async_with",
            _ => block_kind,
        };
        events.push(Event::end_control_block(block_type, node, ctx.file_path));
    }

    // Emit exit_scope after walking children
    if let Some((scope_type, name, qualname)) = scope_info {
        events.push(Event::exit_scope(
            scope_type,
            name,
            qualname,
            node,
            ctx.file_path,
        ));
    }
}

// ============================================================================
// Event Emitters - Each Python node type emits appropriate events
// ============================================================================

/// Emit events for import statements
///
/// Python import grammar (from grammar.txt):
/// ```
/// import_stmt: import_name | import_from
/// import_name: 'import' dotted_as_names
/// import_from: 'from' ('.' | '...')* dotted_name 'import' import_from_targets
///            | 'from' ('.' | '...')+ 'import' import_from_targets
/// import_from_targets: '(' import_from_as_names ','? ')' | import_from_as_names | '*'
/// import_from_as_names: import_from_as_name (',' import_from_as_name)*
/// import_from_as_name: NAME ['as' NAME]
/// dotted_as_names: dotted_as_name (',' dotted_as_name)*
/// dotted_as_name: dotted_name ['as' NAME]
/// dotted_name: NAME ('.' NAME)*
/// ```
///
/// Key insight: Imports create bindings (references) to external definitions,
/// NOT new local definitions. We emit ImportStatement events that the graph
/// builder uses to create edges between modules.
fn emit_import_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    let kind = node.kind();

    if kind == "import_statement" {
        // Handle: import x, import x.y, import x as y, import x, y, z
        emit_import_name_events(ctx, node, events);
    } else if kind == "import_from_statement" {
        // Handle: from x import y, from x import *, from . import y, etc.
        emit_import_from_events(ctx, node, events);
    }
}

/// Handle `import x`, `import x.y.z`, `import x as y`, `import a, b, c`
fn emit_import_name_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    let mut cursor = node.walk();
    let is_tc = is_in_type_checking_block(ctx, node);

    for child in node.children(&mut cursor) {
        match child.kind() {
            "dotted_name" => {
                // Simple import: `import x` or `import x.y.z`
                let module = get_node_text(ctx, child);
                events.push(Event::import_statement(module, vec![], std::collections::HashMap::new(), is_tc, node, ctx.file_path));
            }
            "aliased_import" => {
                // Aliased import: `import x as y`
                if let Some(dotted) = child.child_by_field_name("name") {
                    let module = get_node_text(ctx, dotted);
                    let mut aliases = std::collections::HashMap::new();
                    if let Some(alias_node) = child.child_by_field_name("alias") {
                        let alias = get_node_text(ctx, alias_node);
                        aliases.insert(module.clone(), alias);
                    }
                    events.push(Event::import_statement(module, vec![], aliases, is_tc, node, ctx.file_path));
                }
            }
            _ => {}
        }
    }
}

/// Handle `from x import y`, `from x import *`, `from . import y`, etc.
fn emit_import_from_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    // Extract the module being imported from
    let mut module_name = String::new();
    let mut relative_level = 0;
    let mut cursor = node.walk();

    // First pass: collect module name and relative level
    for child in node.children(&mut cursor) {
        match child.kind() {
            "dotted_name" => {
                // This could be the module name or an imported name
                // The module name comes before "import" keyword
                let text = get_node_text(ctx, child);
                if module_name.is_empty() && !has_passed_import_keyword(ctx, node, child) {
                    module_name = text;
                }
            }
            "relative_import" => {
                // Handle relative imports: from . import x, from .. import x
                let text = get_node_text(ctx, child);
                relative_level = text.chars().filter(|c| *c == '.').count();
                // The tree-sitter Python grammar has no named field for the dotted_name
                // inside relative_import — iterate children to find it.
                let mut ri_cursor = child.walk();
                for ri_child in child.children(&mut ri_cursor) {
                    if ri_child.kind() == "dotted_name" {
                        module_name = get_node_text(ctx, ri_child);
                        break;
                    }
                }
            }
            "import_prefix" => {
                // Dots for relative import level
                let text = get_node_text(ctx, child);
                relative_level = text.chars().filter(|c| *c == '.').count();
            }
            _ => {}
        }
    }

    // Build the full source module path
    let source_module = if relative_level > 0 {
        // Relative import - resolve against current module
        resolve_relative_import(ctx, relative_level, &module_name)
    } else {
        module_name.clone()
    };

    // Second pass: collect imported names and aliases
    let mut imported_names = Vec::new();
    let mut aliases = std::collections::HashMap::new();
    let mut cursor2 = node.walk();

    for child in node.children(&mut cursor2) {
        match child.kind() {
            "wildcard_import" => {
                // from x import *
                imported_names.push("*".to_string());
            }
            "dotted_name" | "identifier" => {
                // Check if this is an imported name (appears after "import" keyword)
                if has_passed_import_keyword(ctx, node, child) {
                    let name = get_node_text(ctx, child);
                    imported_names.push(name);
                }
            }
            "aliased_import" => {
                // from x import y as z - extract original name and alias
                if let Some(name_node) = child.child_by_field_name("name") {
                    let original = get_node_text(ctx, name_node);
                    imported_names.push(original.clone());
                    if let Some(alias_node) = child.child_by_field_name("alias") {
                        let alias = get_node_text(ctx, alias_node);
                        aliases.insert(original, alias);
                    }
                }
            }
            _ => {}
        }
    }

    let is_tc = is_in_type_checking_block(ctx, node);
    events.push(Event::import_statement(
        source_module,
        imported_names,
        aliases,
        is_tc,
        node,
        ctx.file_path,
    ));
}

/// Check if a node appears after the "import" keyword in the parent statement
fn has_passed_import_keyword(ctx: &ParseContext, parent: Node, node: Node) -> bool {
    let mut cursor = parent.walk();
    let mut found_import = false;

    for child in parent.children(&mut cursor) {
        if child.kind() == "import" {
            found_import = true;
            continue;
        }
        if found_import && child.id() == node.id() {
            return true;
        }
        // Also check inside lists like (a, b, c)
        if found_import
            && (child.kind() == "import_from_as_names"
                || child.kind() == "identifier"
                || child.kind() == "dotted_name"
                || child.kind() == "aliased_import")
        {
            // Check if our node is this child or a descendant
            if child.id() == node.id() || is_ancestor(child, node) {
                return true;
            }
        }
    }

    // Fallback: check byte positions
    // Find the "import" keyword position
    let parent_text = get_node_text(ctx, parent);
    if let Some(import_pos) = parent_text.find("import") {
        let import_end = parent.start_byte() + import_pos + 6;
        return node.start_byte() > import_end;
    }

    false
}

/// Check if potential_ancestor is an ancestor of node
fn is_ancestor(potential_ancestor: Node, node: Node) -> bool {
    let mut current = node.parent();
    while let Some(parent) = current {
        if parent.id() == potential_ancestor.id() {
            return true;
        }
        current = parent.parent();
    }
    false
}

/// Resolve a relative import to an absolute module path
fn resolve_relative_import(ctx: &ParseContext, level: usize, module_name: &str) -> String {
    // level 1 = from . import (current package)
    // level 2 = from .. import (parent package)
    // etc.

    let current_parts = &ctx.module_path;

    // For __init__.py, module_path is already the package (e.g. ["requests"]).
    // level=1 means "this package", so we should not subtract anything.
    // For regular modules, module_path includes the file stem (e.g. ["requests","sessions"]),
    // and level=1 means "go up to the package", so we subtract 1.
    let effective_level = if ctx.is_init && level > 0 { level - 1 } else { level };

    if current_parts.is_empty() || effective_level > current_parts.len() {
        // Can't resolve, return as-is with dots preserved for debugging
        let dots = ".".repeat(level);
        if module_name.is_empty() {
            return dots;
        }
        return format!("{}{}", dots, module_name);
    }

    // Go up 'effective_level' directories
    let base_len = current_parts.len().saturating_sub(effective_level);
    let base_parts: Vec<&str> = current_parts
        .iter()
        .take(base_len)
        .map(|s| s.as_str())
        .collect();

    if module_name.is_empty() {
        // from . import x - the module is the parent package itself
        base_parts.join(".")
    } else {
        // from .foo import x - combine base with the submodule
        if base_parts.is_empty() {
            module_name.to_string()
        } else {
            format!("{}.{}", base_parts.join("."), module_name)
        }
    }
}

/// Emit events for class definitions
fn emit_class_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    if let Some(name_node) = node.child_by_field_name("name") {
        let name = get_node_text(ctx, name_node);
        let qualname = build_qualname(ctx, node, &name);

        // Extract base class names from the superclasses argument list
        let mut bases = Vec::new();
        if let Some(superclasses_node) = node.child_by_field_name("superclasses") {
            let mut cursor = superclasses_node.walk();
            for base_child in superclasses_node.children(&mut cursor) {
                match base_child.kind() {
                    "identifier" => {
                        bases.push(get_node_text(ctx, base_child));
                    }
                    "attribute" => {
                        bases.push(get_node_text(ctx, base_child));
                    }
                    _ => {} // Skip commas, parens, keyword_argument
                }
            }
        }

        let docstring = extract_body_docstring(ctx, node);
        events.push(Event::define_name(
            name.clone(),
            qualname.clone(),
            "class",
            node,
            ctx.file_path,
        ));
        events.push(Event::enter_scope_with_docstring(
            ScopeType::Class,
            name.clone(),
            qualname.clone(),
            Vec::new(),
            bases,
            docstring,
            node,
            ctx.file_path,
        ));
        // exit_scope is emitted by walk_node_python after processing children
    }
}

/// Emit events for function definitions
fn emit_function_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    if let Some(name_node) = node.child_by_field_name("name") {
        let name = get_node_text(ctx, name_node);
        let qualname = build_qualname(ctx, node, &name);

        // Extract parameter names from the function's parameters node
        let mut parameters = Vec::new();
        if let Some(params_node) = node.child_by_field_name("parameters") {
            let mut cursor = params_node.walk();
            for child in params_node.children(&mut cursor) {
                match child.kind() {
                    "identifier" => {
                        let param_name = get_node_text(ctx, child);
                        if param_name != "self" && param_name != "cls" {
                            parameters.push(param_name);
                        }
                    }
                    "default_parameter" | "typed_parameter" | "typed_default_parameter" => {
                        // These have a "name" field child that is the parameter name
                        if let Some(param_name_node) = child.child_by_field_name("name") {
                            let param_name = get_node_text(ctx, param_name_node);
                            if param_name != "self" && param_name != "cls" {
                                parameters.push(param_name);
                            }
                        }
                    }
                    "list_splat_pattern" | "list_splat" => {
                        // *args
                        let mut cursor2 = child.walk();
                        for inner in child.children(&mut cursor2) {
                            if inner.kind() == "identifier" {
                                let param_name = get_node_text(ctx, inner);
                                if param_name != "self" && param_name != "cls" {
                                    parameters.push(format!("*{}", param_name));
                                }
                            }
                        }
                    }
                    "dictionary_splat_pattern" | "dictionary_splat" => {
                        // **kwargs
                        let mut cursor2 = child.walk();
                        for inner in child.children(&mut cursor2) {
                            if inner.kind() == "identifier" {
                                let param_name = get_node_text(ctx, inner);
                                if param_name != "self" && param_name != "cls" {
                                    parameters.push(format!("**{}", param_name));
                                }
                            }
                        }
                    }
                    _ => {} // Skip commas, parentheses, etc.
                }
            }
        }

        // Feature 7: Emit UseName for type annotations in parameters and return type.
        // These act as secondary type-reference edges (e.g., def f(x: Config) → uses Config).
        if let Some(params_node) = node.child_by_field_name("parameters") {
            let mut cursor = params_node.walk();
            for child in params_node.children(&mut cursor) {
                if matches!(child.kind(), "typed_parameter" | "typed_default_parameter") {
                    if let Some(type_node) = child.child_by_field_name("type") {
                        emit_use_names_recursive(ctx, type_node, events);
                    }
                }
            }
        }
        if let Some(return_type_node) = node.child_by_field_name("return_type") {
            emit_use_names_recursive(ctx, return_type_node, events);
        }

        let docstring = extract_body_docstring(ctx, node);
        events.push(Event::define_name(
            name.clone(),
            qualname.clone(),
            "function",
            node,
            ctx.file_path,
        ));
        events.push(Event::enter_scope_with_docstring(
            ScopeType::Function,
            name.clone(),
            qualname.clone(),
            parameters,
            vec![],
            docstring,
            node,
            ctx.file_path,
        ));
        // exit_scope is emitted by walk_node_python after processing children
    }
}

/// PRE phase: emit only DefineName for assignments (scope/definition tracking).
/// The Assignment event is emitted POST (after RHS children are walked) via
/// `emit_assignment_post_event`, so call nodes on the RHS appear before the
/// assignment node in the PDG.
fn emit_assignment_define_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    if let Some(left_node) = node.child_by_field_name("left") {
        match left_node.kind() {
            "identifier" => {
                let name = get_node_text(ctx, left_node);
                let qualname = build_qualname(ctx, node, &name);
                let start_pos = left_node.start_position();
                let end_line = node.end_position().row + 1;
                events.push(Event::DefineName {
                    node_id: generate_node_id(ctx.file_path, left_node),
                    name: name.clone(),
                    qualname: qualname.clone(),
                    node_type: "variable".to_string(),
                    file: ctx.file_path.to_string(),
                    line: start_pos.row + 1,
                    end_line,
                    column: start_pos.column,
                });
            }
            "pattern_list" | "tuple_pattern" => {
                let mut cursor = left_node.walk();
                for part in left_node.children(&mut cursor) {
                    if part.kind() == "identifier" {
                        let name = get_node_text(ctx, part);
                        let qualname = build_qualname(ctx, node, &name);
                        events.push(Event::define_name(
                            name,
                            qualname,
                            "variable",
                            part,
                            ctx.file_path,
                        ));
                    }
                }
            }
            _ => {}
        }
    }
}

/// POST phase: emit Assignment event after RHS children have been walked.
fn emit_assignment_post_event(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    if let Some(left_node) = node.child_by_field_name("left") {
        if let Some(right_node) = node.child_by_field_name("right") {
            let value = get_node_text(ctx, right_node);
            let value_type = classify_value_type(right_node.kind());
            match left_node.kind() {
                "identifier" => {
                    let name = get_node_text(ctx, left_node);
                    let qualname = build_qualname(ctx, node, &name);
                    events.push(Event::assignment(
                        name,
                        qualname,
                        value,
                        value_type,
                        node,
                        ctx.file_path,
                    ));
                }
                "pattern_list" | "tuple_pattern" => {
                    let target = get_node_text(ctx, left_node);
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
                _ => {}
            }
        }
    }
}

/// PRE phase: emit DefineName for annotated assignments (`x: Type = value`).
fn emit_annotated_assignment_define_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    if let Some(name_node) = node.child_by_field_name("name") {
        if name_node.kind() == "identifier" {
            let name = get_node_text(ctx, name_node);
            let qualname = build_qualname(ctx, node, &name);
            events.push(Event::define_name(name, qualname, "variable", name_node, ctx.file_path));
        }
    }
}

/// POST phase: emit Assignment event for annotated assignments that have a value.
fn emit_annotated_assignment_post_event(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    let name_node = match node.child_by_field_name("name") {
        Some(n) if n.kind() == "identifier" => n,
        _ => return,
    };
    let value_node = match node.child_by_field_name("value") {
        Some(v) => v,
        None => return, // declaration-only annotation, no RHS
    };
    let name = get_node_text(ctx, name_node);
    let qualname = build_qualname(ctx, node, &name);
    let value = get_node_text(ctx, value_node);
    let value_type = classify_value_type(value_node.kind());
    events.push(Event::assignment(name, qualname, value, value_type, node, ctx.file_path));
}

/// Emit DefineName for a Python 3.12 `type Alias = ...` statement.
fn emit_type_alias_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    if let Some(name_node) = node.child_by_field_name("name") {
        if name_node.kind() == "identifier" {
            let name = get_node_text(ctx, name_node);
            let qualname = build_qualname(ctx, node, &name);
            events.push(Event::define_name(name, qualname, "variable", name_node, ctx.file_path));
        }
    }
    // UseName for type expression identifiers is handled by normal recursion.
}

/// Classify the type of a value expression
fn classify_value_type(kind: &str) -> &'static str {
    match kind {
        "string" | "integer" | "float" | "true" | "false" | "none" => "literal",
        "call" => "call",
        "identifier" => "name",
        "attribute" => "attribute",
        "list" | "tuple" | "dictionary" | "set" => "collection",
        _ => "expression",
    }
}

/// Emit events for identifier usage
fn emit_identifier_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    // Only emit use_name if this identifier is not part of a definition
    if let Some(parent) = node.parent() {
        let parent_kind = parent.kind();

        // Skip if this is a definition context
        if matches!(parent_kind, "function_definition" | "class_definition") {
            if let Some(name_field) = parent.child_by_field_name("name") {
                if name_field.id() == node.id() {
                    return; // This is the name being defined, not used
                }
            }
        }

        // Skip if this is part of an assignment target
        if parent_kind == "assignment" {
            if let Some(left) = parent.child_by_field_name("left") {
                if left.id() == node.id() {
                    return; // This is being assigned to, not used
                }
            }
        }

        // Skip if this is the name field of an annotated assignment (e.g., `x: Type = value`)
        if parent_kind == "annotated_assignment" {
            if let Some(name_field) = parent.child_by_field_name("name") {
                if name_field.id() == node.id() {
                    return;
                }
            }
        }

        // Skip if this is a keyword argument name (e.g., `target` in `f(target=x)`)
        if parent_kind == "keyword_argument" {
            if let Some(name_field) = parent.child_by_field_name("name") {
                if name_field.id() == node.id() {
                    return;
                }
            }
        }

        // Skip if this is the name field of a type alias statement (Python 3.12 `type X = ...`)
        if parent_kind == "type_alias_statement" {
            if let Some(name_field) = parent.child_by_field_name("name") {
                if name_field.id() == node.id() {
                    return;
                }
            }
        }

        // Skip if this is the bound variable in an except clause (e.g., `e` in `except E as e:`)
        if parent_kind == "except_clause" {
            if let Some(name_field) = parent.child_by_field_name("name") {
                if name_field.id() == node.id() {
                    return;
                }
            }
        }

        // Skip if this is part of an import
        if matches!(
            parent_kind,
            "import_statement" | "import_from_statement" | "dotted_name"
        ) {
            return;
        }

        // Skip if this is part of an attribute access (will be handled separately)
        if parent_kind == "attribute" {
            return;
        }

        // Otherwise, this is a use
        let name = get_node_text(ctx, node);
        events.push(Event::use_name(name, node, ctx.file_path));
    }
}

/// Emit events for function/method calls
fn emit_call_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    if let Some(function_node) = node.child_by_field_name("function") {
        let callee = get_node_text(ctx, function_node);

        // Extract top-level argument expressions
        let mut arguments = Vec::new();
        if let Some(args_node) = node.child_by_field_name("arguments") {
            let mut cursor = args_node.walk();
            for child in args_node.children(&mut cursor) {
                match child.kind() {
                    // Skip punctuation and the parens themselves
                    "(" | ")" | "," => {}
                    // For keyword arguments, capture "key=value" text
                    _ => {
                        let text = get_node_text(ctx, child);
                        if !text.is_empty() {
                            arguments.push(text);
                        }
                    }
                }
            }
        }

        events.push(Event::call_expression(
            callee,
            arguments,
            node,
            ctx.file_path,
        ));
    }
}

/// Emit events for attribute access (e.g., obj.attr)
fn emit_attribute_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    let object = node
        .child_by_field_name("object")
        .map(|n| get_node_text(ctx, n))
        .unwrap_or_default();

    let attribute = node
        .child_by_field_name("attribute")
        .map(|n| get_node_text(ctx, n))
        .unwrap_or_default();

    events.push(Event::attribute_access(
        object,
        attribute,
        node,
        ctx.file_path,
    ));
}

/// Emit DefineName events for all identifiers within a pattern/target node.
/// Handles simple identifiers and tuple/pattern unpacking.
fn emit_define_identifiers_in(ctx: &ParseContext, target_node: Node, scope_node: Node, events: &mut Vec<Event>) {
    match target_node.kind() {
        "identifier" => {
            let name = get_node_text(ctx, target_node);
            let qualname = build_qualname(ctx, scope_node, &name);
            events.push(Event::define_name(name, qualname, "variable", target_node, ctx.file_path));
        }
        "pattern_list" | "tuple_pattern" => {
            let mut cursor = target_node.walk();
            for child in target_node.children(&mut cursor) {
                if child.kind() == "identifier" {
                    let name = get_node_text(ctx, child);
                    let qualname = build_qualname(ctx, scope_node, &name);
                    events.push(Event::define_name(name, qualname, "variable", child, ctx.file_path));
                }
            }
        }
        _ => {}
    }
}

/// Emit events for control flow blocks
fn emit_control_block_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>, kind: &str) {
    let block_type = match kind {
        "if_statement" => "if",
        "for_statement" => "for",
        "while_statement" => "while",
        "try_statement" => "try",
        "with_statement" => "with",
        "async_for_statement" | "async_for" => "async_for",
        "async_with_statement" | "async_with" => "async_with",
        _ => kind,
    };

    // Extract the condition/expression based on block type
    let condition = match kind {
        "if_statement" | "while_statement" => {
            // if/while have a "condition" field
            node.child_by_field_name("condition")
                .map(|n| get_node_text(ctx, n))
                .unwrap_or_default()
        }
        "for_statement" | "async_for_statement" | "async_for" => {
            // for has "left" (loop var) and "right" (iterable)
            let left = node
                .child_by_field_name("left")
                .map(|n| get_node_text(ctx, n))
                .unwrap_or_default();
            let right = node
                .child_by_field_name("right")
                .map(|n| get_node_text(ctx, n))
                .unwrap_or_default();
            format!("{} in {}", left, right)
        }
        "with_statement" | "async_with_statement" | "async_with" => {
            // Extract the context expression
            let mut cursor = node.walk();
            let result = node
                .children(&mut cursor)
                .find(|c| c.kind() == "with_clause" || c.kind() == "with_item")
                .map(|n| get_node_text(ctx, n))
                .unwrap_or_default();
            result
        }
        "try_statement" => "try".to_string(),
        _ => String::new(),
    };

    events.push(Event::control_block(
        block_type,
        condition,
        node,
        ctx.file_path,
    ));

    // Feature 3: Bind for-loop variables so they are resolvable inside the loop body.
    if matches!(kind, "for_statement" | "async_for_statement" | "async_for") {
        if let Some(left_node) = node.child_by_field_name("left") {
            emit_define_identifiers_in(ctx, left_node, node, events);
        }
    }

    // Feature 4: Bind with-statement `as` variables.
    if matches!(kind, "with_statement" | "async_with_statement" | "async_with") {
        let mut outer_cursor = node.walk();
        for child in node.children(&mut outer_cursor) {
            if child.kind() == "with_clause" {
                let mut clause_cursor = child.walk();
                for with_item in child.children(&mut clause_cursor) {
                    if with_item.kind() == "with_item" {
                        if let Some(alias_node) = with_item.child_by_field_name("alias") {
                            emit_define_identifiers_in(ctx, alias_node, node, events);
                        }
                    }
                }
            } else if child.kind() == "with_item" {
                if let Some(alias_node) = child.child_by_field_name("alias") {
                    emit_define_identifiers_in(ctx, alias_node, node, events);
                }
            }
        }
    }
}

/// Emit events for return statements
fn emit_return_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    // Get the return value if present
    let mut cursor = node.walk();
    let value_node = node.children(&mut cursor).find(|c| c.kind() != "return");

    let (value, value_type) = if let Some(val_node) = value_node {
        let val_text = get_node_text(ctx, val_node);
        let val_type = classify_value_type(val_node.kind());
        (val_text, val_type)
    } else {
        (String::new(), "none")
    };

    events.push(Event::return_stmt(value, value_type, node, ctx.file_path));
}

/// Emit events for break statements
fn emit_break_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    events.push(Event::break_statement(node, ctx.file_path));
}

/// Emit events for continue statements
fn emit_continue_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    events.push(Event::continue_statement(node, ctx.file_path));
}

/// Emit events for raise statements
fn emit_raise_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    // Get the exception expression if present
    let mut cursor = node.walk();
    let exception_node = node.children(&mut cursor).find(|c| c.kind() != "raise");

    let exception = if let Some(exc_node) = exception_node {
        get_node_text(ctx, exc_node)
    } else {
        String::new()
    };

    events.push(Event::raise_statement(exception, node, ctx.file_path));
}

/// Emit events for literal values (strings, numbers, booleans, None)
fn emit_literal_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>, kind: &str) {
    let value = get_node_text(ctx, node);
    let literal_type = match kind {
        "string" => "string",
        "integer" => "integer",
        "float" => "float",
        "true" | "false" => "boolean",
        "none" => "none",
        _ => "unknown",
    };

    events.push(Event::literal(value, literal_type, node, ctx.file_path));
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Get the text content of a node
fn get_node_text(ctx: &ParseContext, node: Node) -> String {
    ctx.source[node.start_byte()..node.end_byte()].to_string()
}

/// Build qualified name by walking up the TreeSitter parent chain
/// This leverages TreeSitter's native parent tracking instead of manual stacks
fn build_qualname(ctx: &ParseContext, node: Node, name: &str) -> String {
    let mut parts = Vec::new();

    // Add the current node's name first
    if name != "module" && !name.is_empty() {
        parts.push(name.to_string());
    }

    // Walk up the parent chain to build the qualified name
    let mut current = node.parent();
    while let Some(parent) = current {
        let parent_kind = parent.kind();

        // Only add names from scope-creating nodes (classes and functions)
        if is_scope_creating_node(parent_kind) {
            if let Some(parent_name_node) = parent.child_by_field_name("name") {
                let parent_name = ctx.source
                    [parent_name_node.start_byte()..parent_name_node.end_byte()]
                    .to_string();
                parts.insert(0, parent_name);
            }
        }

        current = parent.parent();
    }

    // Add module path if present
    if !ctx.module_path.is_empty() {
        let mut full_parts = ctx.module_path.clone();
        full_parts.extend(parts);
        return full_parts.join(".");
    }

    if parts.is_empty() {
        name.to_string()
    } else {
        parts.join(".")
    }
}

/// Check if a node type creates a scope for qualified names
fn is_scope_creating_node(kind: &str) -> bool {
    matches!(kind, "class_definition" | "function_definition")
}

/// Emit events for else clauses
fn emit_else_block_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    events.push(Event::else_block(
        "else",
        String::new(),
        node,
        ctx.file_path,
    ));
}

/// Emit events for elif clauses
fn emit_elif_block_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    // elif has a condition child
    let condition = node
        .child_by_field_name("condition")
        .map(|n| get_node_text(ctx, n))
        .unwrap_or_default();
    events.push(Event::else_block("elif", condition, node, ctx.file_path));
}

// ============================================================================
// Phase 3 & 4: New Emit Functions
// ============================================================================

/// Check if a node is inside an `if TYPE_CHECKING:` block
fn is_in_type_checking_block(ctx: &ParseContext, node: Node) -> bool {
    let mut current = node.parent();
    while let Some(parent) = current {
        if parent.kind() == "if_statement" {
            if let Some(cond) = parent.child_by_field_name("condition") {
                let cond_text = get_node_text(ctx, cond);
                if cond_text == "TYPE_CHECKING" {
                    return true;
                }
            }
        }
        current = parent.parent();
    }
    false
}

/// Emit events for decorator expressions
fn emit_decorator_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    let mut cursor = node.walk();
    // A decorator node has one child: the expression (@expr)
    for child in node.children(&mut cursor) {
        match child.kind() {
            "identifier" => {
                let name = get_node_text(ctx, child);
                events.push(Event::decorator(name, false, vec![], node, ctx.file_path));
            }
            "attribute" => {
                let name = get_node_text(ctx, child);
                events.push(Event::decorator(name, false, vec![], node, ctx.file_path));
            }
            "call" => {
                let callee = child
                    .child_by_field_name("function")
                    .map(|n| get_node_text(ctx, n))
                    .unwrap_or_default();
                let mut args = Vec::new();
                if let Some(args_node) = child.child_by_field_name("arguments") {
                    let mut c = args_node.walk();
                    for arg in args_node.children(&mut c) {
                        match arg.kind() {
                            "(" | ")" | "," => {}
                            _ => {
                                let text = get_node_text(ctx, arg);
                                if !text.is_empty() {
                                    args.push(text);
                                }
                            }
                        }
                    }
                }
                events.push(Event::decorator(callee, true, args, node, ctx.file_path));
            }
            _ => {}
        }
    }
}

/// Emit events for match statements (Python 3.10+)
fn emit_match_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    let subject = node
        .child_by_field_name("subject")
        .map(|n| get_node_text(ctx, n))
        .unwrap_or_default();
    events.push(Event::control_block("match", subject, node, ctx.file_path));
}

/// Emit events for case clauses within match statements
fn emit_case_clause_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    let pattern = node
        .child_by_field_name("pattern")
        .map(|n| get_node_text(ctx, n))
        .unwrap_or_default();
    events.push(Event::else_block("case", pattern, node, ctx.file_path));
}

/// Emit events for walrus operator := (named_expression)
fn emit_named_expression_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    if let Some(name_node) = node.child_by_field_name("name") {
        let name = get_node_text(ctx, name_node);
        let qualname = build_qualname(ctx, node, &name);
        events.push(Event::define_name(
            name.clone(),
            qualname.clone(),
            "variable",
            name_node,
            ctx.file_path,
        ));
        if let Some(value_node) = node.child_by_field_name("value") {
            let value = get_node_text(ctx, value_node);
            let value_type = classify_value_type(value_node.kind());
            events.push(Event::assignment(
                name,
                qualname,
                value,
                value_type,
                node,
                ctx.file_path,
            ));
        }
    }
}

/// Emit events for comprehension expressions (list/dict/set/generator)
fn emit_comprehension_events(
    ctx: &ParseContext,
    node: Node,
    events: &mut Vec<Event>,
    kind: &str,
) {
    let scope_name = format!(
        "<{}>",
        match kind {
            "list_comprehension" => "listcomp",
            "dictionary_comprehension" => "dictcomp",
            "set_comprehension" => "setcomp",
            "generator_expression" => "genexpr",
            _ => kind,
        }
    );
    let qualname = build_qualname(ctx, node, &scope_name);
    // Emit both enter and exit immediately — comprehensions are not tracked via scope_info
    events.push(Event::enter_scope(
        ScopeType::Comprehension,
        scope_name.clone(),
        qualname.clone(),
        vec![],
        vec![],
        node,
        ctx.file_path,
    ));
    events.push(Event::exit_scope(
        ScopeType::Comprehension,
        scope_name,
        qualname,
        node,
        ctx.file_path,
    ));
}

/// Emit events for lambda expressions
/// Note: enter_scope is emitted here; exit_scope is emitted by walk_node_python
/// via the scope_info tracking for "lambda" nodes.
fn emit_lambda_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    let pos = node.start_position();
    let scope_name = format!("<lambda:{}>", pos.row + 1);
    let qualname = build_qualname(ctx, node, &scope_name);

    // Extract lambda parameters
    let mut parameters = Vec::new();
    if let Some(params_node) = node.child_by_field_name("parameters") {
        let mut cursor = params_node.walk();
        for child in params_node.children(&mut cursor) {
            match child.kind() {
                "identifier" => {
                    let param_name = get_node_text(ctx, child);
                    parameters.push(param_name);
                }
                "default_parameter" => {
                    if let Some(param_name_node) = child.child_by_field_name("name") {
                        parameters.push(get_node_text(ctx, param_name_node));
                    }
                }
                _ => {}
            }
        }
    }

    events.push(Event::define_name(
        scope_name.clone(),
        qualname.clone(),
        "function",
        node,
        ctx.file_path,
    ));
    // Only emit enter_scope here; exit_scope is deferred to walk_node_python via scope_info
    events.push(Event::enter_scope(
        ScopeType::Lambda,
        scope_name,
        qualname,
        parameters,
        vec![],
        node,
        ctx.file_path,
    ));
}

/// Emit events for augmented assignments (+=, -=, etc.)
fn emit_augmented_assignment_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    if let Some(left_node) = node.child_by_field_name("left") {
        if left_node.kind() == "identifier" {
            let name = get_node_text(ctx, left_node);
            let qualname = build_qualname(ctx, node, &name);
            // Augmented assignment reads AND writes the target
            events.push(Event::use_name(name.clone(), left_node, ctx.file_path));
            if let Some(right_node) = node.child_by_field_name("right") {
                let value = get_node_text(ctx, right_node);
                let value_type = classify_value_type(right_node.kind());
                events.push(Event::assignment(
                    name,
                    qualname,
                    value,
                    value_type,
                    node,
                    ctx.file_path,
                ));
            }
        }
    }
}

/// Emit events for global/nonlocal statements
fn emit_global_nonlocal_events(
    ctx: &ParseContext,
    node: Node,
    events: &mut Vec<Event>,
    kind: &str,
) {
    let decl_type = if kind == "global_statement" {
        "global"
    } else {
        "nonlocal"
    };
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            let name = get_node_text(ctx, child);
            let qualname = build_qualname(ctx, node, &name);
            events.push(Event::define_name(
                name,
                qualname,
                decl_type,
                child,
                ctx.file_path,
            ));
        }
    }
}

/// Emit events for assert statements
fn emit_assert_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    let condition = node
        .child_by_field_name("condition")
        .map(|n| get_node_text(ctx, n))
        .unwrap_or_default();
    events.push(Event::control_block("assert", condition, node, ctx.file_path));
    events.push(Event::end_control_block("assert", node, ctx.file_path));
}

/// Emit events for delete statements
fn emit_delete_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            let name = get_node_text(ctx, child);
            events.push(Event::use_name(name, child, ctx.file_path));
        }
    }
}

/// Emit events for yield expressions
fn emit_yield_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    let mut cursor = node.walk();
    let children: Vec<Node> = node.children(&mut cursor).collect();

    // Check if this is `yield from` by looking for "from" keyword child
    let is_from = children.iter().any(|c| c.kind() == "from");

    // Get the value (the expression after yield or yield from)
    let value = children
        .iter()
        .find(|c| !matches!(c.kind(), "yield" | "from"))
        .map(|n| get_node_text(ctx, *n))
        .unwrap_or_default();

    events.push(Event::yield_expression(value, is_from, node, ctx.file_path));
}

/// Emit events for except clauses (exception variable binding + type use).
fn emit_except_clause_events(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    // except SomeException as e: -> bind 'e' as a variable
    if let Some(name_node) = node.child_by_field_name("name") {
        if name_node.kind() == "identifier" {
            let name = get_node_text(ctx, name_node);
            let qualname = build_qualname(ctx, node, &name);
            events.push(Event::define_name(
                name,
                qualname,
                "variable",
                name_node,
                ctx.file_path,
            ));
        }
    }

    // Emit UseName for the exception type(s): `except ValueError as e:` → UseName("ValueError")
    // Also handles tuples: `except (ValueError, TypeError):` → UseName for each
    if let Some(type_node) = node.child_by_field_name("type") {
        emit_use_names_recursive(ctx, type_node, events);
    }
}

/// Recursively emit UseName for every identifier/type_identifier within a node.
/// Used for type expressions where we want to capture all referenced type names.
fn emit_use_names_recursive(ctx: &ParseContext, node: Node, events: &mut Vec<Event>) {
    match node.kind() {
        "identifier" | "type_identifier" => {
            let name = get_node_text(ctx, node);
            events.push(Event::use_name(name, node, ctx.file_path));
        }
        _ => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                emit_use_names_recursive(ctx, child, events);
            }
        }
    }
}
