//! JavaScript/TypeScript/JSX/TSX tree-sitter walker.
//!
//! All four JS-family variants share this single walker. Grammar-specific node kinds
//! (e.g. TypeScript `interface_declaration`) are only emitted when the `JsLang`
//! flag indicates a TypeScript variant; JS grammars never produce those node kinds
//! anyway, so the match arms are safe for all variants.
//!
//! Event model mirrors the Python walker:
//! - `EnterScope`/`ExitScope` for module, function, class, interface
//! - `DefineName` for variables and imports
//! - `ImportStatement` for ES module imports
//! - `CallExpression` for function/constructor calls

pub mod config;

use crate::events::{Event, ScopeType};
use tree_sitter::{Node, Tree};

// ============================================================================
// Language variant
// ============================================================================

/// Identifies which JS-family grammar was used to parse the file.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum JsLang {
    JavaScript,
    TypeScript,
    Tsx,
}

// ============================================================================
// Parse context
// ============================================================================

/// Project-level information resolved once per file and cached in `Ctx`.
struct ProjectInfo {
    /// Name of the directory containing `package.json` (e.g. `"frontend"`).
    root_name: String,
    /// Absolute path string of that directory (e.g. `"/path/to/frontend"`).
    root_path: String,
    /// Tsconfig path alias mappings: `(alias_prefix, target_dir)`.
    /// e.g. `("@app", "src/app")`.
    aliases: Vec<(String, String)>,
}

struct Ctx<'a> {
    source: &'a str,
    file_path: &'a str,
    /// Stack of (short name, qualname) for the current scope chain.
    scope_stack: Vec<(String, String)>,
    /// Project root info, used for module path derivation and import resolution.
    project_info: Option<ProjectInfo>,
}

impl<'a> Ctx<'a> {
    fn current_qualname(&self) -> String {
        self.scope_stack
            .last()
            .map(|(_, qn)| qn.as_str())
            .unwrap_or("")
            .to_string()
    }

    fn build_qualname(&self, name: &str) -> String {
        let parent = self.current_qualname();
        if parent.is_empty() {
            name.to_string()
        } else {
            format!("{}.{}", parent, name)
        }
    }

    fn get_text(&self, node: Node) -> &'a str {
        &self.source[node.start_byte()..node.end_byte()]
    }
}

// ============================================================================
// JSDoc extraction helpers
// ============================================================================

/// Extract a JSDoc comment (`/** ... */`) immediately preceding `node`.
/// Returns the cleaned comment text, or `None` if no JSDoc is present.
fn extract_jsdoc(node: Node, source: &str) -> Option<String> {
    let prev = node.prev_named_sibling()?;
    if prev.kind() != "comment" {
        return None;
    }
    let text = &source[prev.byte_range()];
    if !text.starts_with("/**") {
        return None;
    }
    Some(clean_jsdoc(text))
}

/// Extract a file-level JSDoc from the root node's first named child comment.
fn extract_module_jsdoc(root: Node, source: &str) -> Option<String> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "comment" {
            let text = &source[child.byte_range()];
            if text.starts_with("/**") {
                return Some(clean_jsdoc(text));
            }
            // Non-JSDoc comment at top — skip
            continue;
        }
        break;
    }
    None
}

/// Strip `/** ... */` delimiters and leading `* ` from each line.
fn clean_jsdoc(raw: &str) -> String {
    let inner = raw
        .trim_start_matches("/**")
        .trim_end_matches("*/")
        .trim_matches('\n');
    let lines: Vec<&str> = inner
        .lines()
        .map(|l| {
            let t = l.trim();
            t.strip_prefix("* ").or_else(|| t.strip_prefix('*')).unwrap_or(t)
        })
        .collect();
    // Drop leading/trailing blank lines
    let start = lines.iter().position(|l| !l.trim().is_empty()).unwrap_or(0);
    let end = lines.iter().rposition(|l| !l.trim().is_empty()).map(|i| i + 1).unwrap_or(lines.len());
    lines[start..end].join("\n")
}

// ============================================================================
// Module path derivation
// ============================================================================

/// Derive a dotted module qualname from a JS/TS file path.
///
/// Walks up from the file to find `package.json` (the project root), then
/// builds a path as `project_name.relative.path.stem`.
/// e.g. `frontend/src/app/App.tsx` → `["frontend", "src", "app", "App"]`.
///
/// `index.*` files take the name of their parent directory — the stem is
/// omitted so the module represents the directory itself.
pub fn derive_module_path(file_path: &str) -> Vec<String> {
    use std::path::{Component, Path};

    let path = Path::new(file_path);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("index");

    // Use the project root as anchor when available.
    if let Some(info) = find_project_info(file_path) {
        let prefix = format!("{}/", info.root_path);
        if file_path.starts_with(&prefix) {
            let rest = &file_path[prefix.len()..]; // e.g. "src/app/App.tsx"
            if !rest.starts_with("node_modules") {
                let rest_path = Path::new(rest);
                let rest_stem = rest_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(stem);
                let rest_dir = rest_path
                    .parent()
                    .and_then(|p| p.to_str())
                    .unwrap_or("");

                let mut parts = vec![info.root_name];
                if !rest_dir.is_empty() && rest_dir != "." {
                    for seg in rest_dir.split('/') {
                        if !seg.is_empty() {
                            parts.push(seg.to_string());
                        }
                    }
                }
                if rest_stem != "index" {
                    parts.push(rest_stem.to_string());
                }
                return parts;
            }
        }
    }

    // Fallback: collect normal directory components, strip src root.
    let dir_parts: Vec<String> = path
        .parent()
        .map(|p| {
            p.components()
                .filter_map(|c| match c {
                    Component::Normal(n) => n.to_str().map(String::from),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();

    let stops = ["node_modules", ".git", ".venv"];
    let trim_at = dir_parts
        .iter()
        .position(|p| stops.contains(&p.as_str()))
        .unwrap_or(dir_parts.len());
    let dir_parts = &dir_parts[..trim_at];

    // "app" excluded — it is a valid subdirectory name, not a source root.
    let src_roots = ["src", "lib", "packages", "source"];
    let start = dir_parts
        .iter()
        .rposition(|p| src_roots.contains(&p.as_str()))
        .map(|i| i + 1)
        .unwrap_or_else(|| {
            if dir_parts.is_empty() { 0 } else { dir_parts.len() - 1 }
        });

    let mut parts: Vec<String> = dir_parts[start..].to_vec();
    if stem != "index" {
        parts.push(stem.to_string());
    }
    if parts.is_empty() {
        parts.push(stem.to_string());
    }
    parts
}

/// Walk up from `file_path` to find the JS project root (directory containing
/// `package.json` or `tsconfig.json`) and collect tsconfig path aliases.
///
/// Stops immediately if `node_modules` is encountered so dependency files are
/// not attributed to a parent project.
fn find_project_info(file_path: &str) -> Option<ProjectInfo> {
    use std::path::Path;

    let mut dir = Path::new(file_path).parent()?;
    loop {
        if dir.file_name().and_then(|n| n.to_str()) == Some("node_modules") {
            return None;
        }

        let has_package_json = dir.join("package.json").exists();
        let tsconfig_path = dir.join("tsconfig.json");
        let has_tsconfig = tsconfig_path.exists();

        if has_package_json || has_tsconfig {
            let root_name = dir.file_name()?.to_str()?.to_string();
            let root_path = dir.to_str()?.to_string();

            let aliases = if has_tsconfig {
                read_tsconfig_aliases_from(&tsconfig_path).unwrap_or_default()
            } else {
                vec![]
            };

            return Some(ProjectInfo { root_name, root_path, aliases });
        }

        dir = dir.parent()?;
    }
}

/// Parse `compilerOptions.paths` from a `tsconfig.json` file.
///
/// Returns `(alias_prefix, target_dir)` pairs, e.g. `("@app", "src/app")`.
/// Single-character aliases like `@` are skipped (too generic).
fn read_tsconfig_aliases_from(path: &std::path::Path) -> Option<Vec<(String, String)>> {
    let content = std::fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let paths_obj = json.get("compilerOptions")?.get("paths")?.as_object()?;

    let mut aliases = Vec::new();
    for (alias, targets) in paths_obj {
        let alias_prefix = alias.strip_suffix("/*").unwrap_or(alias.as_str());
        if alias_prefix.len() <= 1 {
            continue;
        }
        if let Some(target_str) = targets
            .as_array()
            .and_then(|a| a.first())
            .and_then(|v| v.as_str())
        {
            let stripped = target_str.strip_prefix("./").unwrap_or(target_str);
            let target_dir = stripped
                .strip_suffix("/*")
                .unwrap_or(stripped)
                .trim_end_matches('/');
            // Strip file extensions from file-targeted aliases like "./src/store.ts" → "src/store"
            let target_dir = [".ts", ".tsx", ".js", ".jsx", ".mjs"]
                .iter()
                .fold(target_dir.to_string(), |s, ext| {
                    s.strip_suffix(ext).unwrap_or(&s).to_string()
                });
            aliases.push((alias_prefix.to_string(), target_dir));
        }
    }
    Some(aliases)
}

/// Resolve a JS import string to a canonical dotted module qualname.
///
/// Relative imports (`./foo`, `../bar/baz`) are resolved against the importing
/// file's path and then converted to the same dotted form as `derive_module_path`:
///   `../domains/cfg`  (from `frontend/src/app/store.ts`)
///     →  `frontend.src.domains.cfg`
///
/// For `@`-prefixed imports that match a known tsconfig alias, the alias is
/// expanded to the real directory path and prefixed with the project root name:
///   `@app/store`  →  `frontend.src.app.store`
///   `@ui/components/SearchBar`  →  `frontend.src.ui.components.SearchBar`
///
/// Unrecognised `@`-prefixed imports (scoped npm packages) fall through with
/// slashes replaced by dots so their IDs are consistent:
///   `@tanstack/react-query`  →  `@tanstack.react-query`
fn normalize_import(module: &str, file_path: &str, info: &Option<ProjectInfo>) -> String {
    // Relative import — resolve against the importing file's directory.
    if module.starts_with("./") || module.starts_with("../") {
        use std::path::Path;
        // Skip imports that point to non-source files (e.g. .css, .svg, .png).
        if let Some(ext) = Path::new(module).extension() {
            let ext = ext.to_string_lossy();
            if !matches!(ext.as_ref(), "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "py") {
                return String::new();
            }
        }
        if let Some(dir) = Path::new(file_path).parent() {
            // Build the resolved filesystem path (without extension).
            let resolved = dir.join(module);
            // Normalise away `..` and `.` components.
            let canonical: std::path::PathBuf = resolved
                .components()
                .fold(std::path::PathBuf::new(), |mut acc, c| {
                    use std::path::Component;
                    match c {
                        Component::ParentDir => { acc.pop(); }
                        Component::CurDir => {}
                        _ => acc.push(c),
                    }
                    acc
                });
            // derive_module_path expects a file path string; give it a fake
            // extension so the stem logic works correctly for non-index files.
            let as_str = canonical.to_string_lossy();
            return derive_module_path(&format!("{}.ts", as_str)).join(".");
        }
    }

    if !module.starts_with('@') {
        return module.to_string();
    }

    if let Some(info) = info {
        for (alias_prefix, target_dir) in &info.aliases {
            let alias_slash = format!("{}/", alias_prefix);
            let rest = if module.starts_with(&alias_slash) {
                &module[alias_slash.len()..] // "store" from "@app/store"
            } else if module == alias_prefix.as_str() {
                "" // exact alias with no trailing path
            } else {
                continue;
            };

            let mut parts = vec![info.root_name.clone()];
            for seg in target_dir.split('/') {
                if !seg.is_empty() {
                    parts.push(seg.to_string());
                }
            }
            for seg in rest.split('/') {
                if !seg.is_empty() {
                    parts.push(seg.to_string());
                }
            }
            return parts.join(".");
        }
    }

    // No alias matched — scoped npm package; normalise slashes for consistency.
    module.replace('/', ".")
}

// ============================================================================
// Public parse entry point
// ============================================================================

pub fn parse(source: &str, tree: &Option<Tree>, file_path: &str, _lang: JsLang) -> Vec<Event> {
    let mut events = Vec::new();

    // Source line events (for CodeSnippet subscriber)
    for (i, line) in source.lines().enumerate() {
        events.push(Event::SourceLine {
            file: file_path.to_string(),
            line_number: i + 1,
            text: line.to_string(),
        });
    }

    let Some(tree) = tree else {
        return events;
    };

    let root = tree.root_node();
    let project_info = find_project_info(file_path);
    let module_parts = derive_module_path(file_path);
    let module_name = module_parts.last().cloned().unwrap_or_else(|| "module".to_string());
    let module_qualname = module_parts.join(".");

    let mut ctx = Ctx {
        source,
        file_path,
        scope_stack: vec![(module_name.clone(), module_qualname.clone())],
        project_info,
    };

    // Enter module scope.
    let module_docstring = extract_module_jsdoc(root, source);
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

    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        walk_node(&mut ctx, child, &mut events);
    }

    ctx.scope_stack.pop();
    events.push(Event::exit_scope(
        ScopeType::Module,
        module_name,
        module_qualname,
        root,
        file_path,
    ));

    events
}

// ============================================================================
// Tree walker
// ============================================================================

/// Emit a UseName event if `node` is an identifier in a USE context (not a
/// definition site, import binding, member-expression property, etc.).
///
/// Filters mirror Python's `emit_identifier_events` logic adapted for the JS/TS AST.
fn emit_identifier_use(ctx: &Ctx, node: Node, events: &mut Vec<Event>) {
    if let Some(parent) = node.parent() {
        match parent.kind() {
            // Skip: declarator name (const FOO = ...)
            "variable_declarator" => {
                if parent.child_by_field_name("name").is_some_and(|n| n.id() == node.id()) {
                    return;
                }
            }
            // Skip: function / generator declaration name
            "function_declaration" | "generator_function_declaration"
            | "function" | "generator_function" => {
                if parent.child_by_field_name("name").is_some_and(|n| n.id() == node.id()) {
                    return;
                }
            }
            // Skip: class declaration name
            "class_declaration" | "class" | "abstract_class_declaration" => {
                if parent.child_by_field_name("name").is_some_and(|n| n.id() == node.id()) {
                    return;
                }
            }
            // Skip: method / property signature name
            "method_definition" | "method_signature" | "property_signature"
            | "public_field_definition" => {
                if parent.child_by_field_name("name").is_some_and(|n| n.id() == node.id()) {
                    return;
                }
            }
            // Skip: all import binding positions
            "import_clause" | "import_specifier" | "namespace_import" => return,
            // Skip: export specifier  (export { X })
            "export_specifier" => return,
            // Skip: labeled statement label
            "labeled_statement" => {
                if parent.child_by_field_name("label").is_some_and(|n| n.id() == node.id()) {
                    return;
                }
            }
            // Skip: member-expression property  (obj.PROP — not LEGB-resolvable)
            // The object part (obj) is NOT skipped and will fire separately.
            "member_expression" => {
                if parent.child_by_field_name("property").is_some_and(|n| n.id() == node.id()) {
                    return;
                }
            }
            // Skip: object literal key  { key: value }
            "pair" => {
                if parent.child_by_field_name("key").is_some_and(|n| n.id() == node.id()) {
                    return;
                }
            }
            // Skip: TypeScript type parameter declaration
            "type_parameter" => return,
            // Skip: TypeScript parameter names — the `name` field is a definition, not a use.
            // The `type` field IS walked (by the required_parameter arm in walk_node).
            "required_parameter" | "optional_parameter" => {
                if parent.child_by_field_name("name").is_some_and(|n| n.id() == node.id()) {
                    return;
                }
            }
            _ => {}
        }
    }
    let name = ctx.get_text(node);
    if name.is_empty()
        || name == "undefined"
        || name == "null"
        || name == "this"
        || name == "super"
    {
        return;
    }
    events.push(Event::use_name(name.to_string(), node, ctx.file_path));
}

/// Emit a Decorator event for a TypeScript/JS decorator node (`@expr`).
///
/// Handles three forms:
/// - `@Identifier`          → Decorator(name, is_call=false)
/// - `@member.expr`         → Decorator(member.expr, is_call=false)
/// - `@call_expression(…)`  → Decorator(callee, is_call=true, args)
fn emit_decorator(ctx: &mut Ctx, node: Node, events: &mut Vec<Event>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "identifier" => {
                let name = ctx.get_text(child).to_string();
                events.push(Event::decorator(name, false, vec![], node, ctx.file_path));
            }
            "member_expression" => {
                let name = ctx.get_text(child).to_string();
                events.push(Event::decorator(name, false, vec![], node, ctx.file_path));
            }
            "call_expression" => {
                let callee = child
                    .child_by_field_name("function")
                    .map(|n| ctx.get_text(n).to_string())
                    .unwrap_or_default();
                let mut args = Vec::new();
                if let Some(args_node) = child.child_by_field_name("arguments") {
                    let mut c = args_node.walk();
                    for arg in args_node.children(&mut c) {
                        match arg.kind() {
                            "(" | ")" | "," => {}
                            _ if arg.is_named() => {
                                let text = ctx.get_text(arg).to_string();
                                if !text.is_empty() {
                                    args.push(text);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                events.push(Event::decorator(callee, true, args, node, ctx.file_path));
            }
            _ => {}
        }
    }
}

fn walk_node(ctx: &mut Ctx, node: Node, events: &mut Vec<Event>) {
    match node.kind() {
        "function_declaration" | "generator_function_declaration" => {
            emit_function(ctx, node, events);
        }
        "class_declaration" | "abstract_class_declaration" => {
            emit_class(ctx, node, events);
        }
        "lexical_declaration" | "variable_declaration" => {
            emit_variable_declaration(ctx, node, events);
        }
        "import_statement" => {
            emit_import(ctx, node, events);
        }
        // POST-ORDER: recurse arguments first so inner call events fire before the
        // outer call — matches Python's emission order for correct PDG sequencing.
        "call_expression" | "new_expression" => {
            if let Some(args) = node.child_by_field_name("arguments") {
                let mut cursor = args.walk();
                for child in args.children(&mut cursor) {
                    walk_node(ctx, child, events);
                }
            }
            emit_call(ctx, node, events);
        }
        "export_statement" => {
            emit_export(ctx, node, events);
        }
        // ── Control flow ────────────────────────────────────────────────────────
        "if_statement" => {
            let condition = node
                .child_by_field_name("condition")
                .map(|c| ctx.get_text(c).to_string())
                .unwrap_or_default();
            events.push(Event::control_block("if", condition, node, ctx.file_path));
            if let Some(c) = node.child_by_field_name("condition") {
                walk_node(ctx, c, events);
            }
            if let Some(c) = node.child_by_field_name("consequence") {
                walk_node(ctx, c, events);
            }
            if let Some(c) = node.child_by_field_name("alternative") {
                walk_node(ctx, c, events);
            }
            events.push(Event::end_control_block("if", node, ctx.file_path));
        }
        "else_clause" => {
            events.push(Event::else_block("else", String::new(), node, ctx.file_path));
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                walk_node(ctx, child, events);
            }
            events.push(Event::end_control_block("else", node, ctx.file_path));
        }
        "while_statement" => {
            let condition = node
                .child_by_field_name("condition")
                .map(|c| ctx.get_text(c).to_string())
                .unwrap_or_default();
            events.push(Event::control_block("while", condition, node, ctx.file_path));
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                walk_node(ctx, child, events);
            }
            events.push(Event::end_control_block("while", node, ctx.file_path));
        }
        "do_statement" => {
            let condition = node
                .child_by_field_name("condition")
                .map(|c| ctx.get_text(c).to_string())
                .unwrap_or_default();
            events.push(Event::control_block("do-while", condition, node, ctx.file_path));
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                walk_node(ctx, child, events);
            }
            events.push(Event::end_control_block("do-while", node, ctx.file_path));
        }
        "try_statement" => {
            events.push(Event::control_block("try", String::new(), node, ctx.file_path));
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                walk_node(ctx, child, events);
            }
            events.push(Event::end_control_block("try", node, ctx.file_path));
        }
        "finally_clause" => {
            events.push(Event::else_block("finally", String::new(), node, ctx.file_path));
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                walk_node(ctx, child, events);
            }
            events.push(Event::end_control_block("finally", node, ctx.file_path));
        }
        "switch_statement" => {
            let value = node
                .child_by_field_name("value")
                .map(|v| ctx.get_text(v).to_string())
                .unwrap_or_default();
            events.push(Event::control_block("switch", value, node, ctx.file_path));
            if let Some(body) = node.child_by_field_name("body") {
                let mut cursor = body.walk();
                for child in body.children(&mut cursor) {
                    walk_node(ctx, child, events);
                }
            }
            events.push(Event::end_control_block("switch", node, ctx.file_path));
        }
        "switch_case" => {
            let value = node
                .child_by_field_name("value")
                .map(|v| ctx.get_text(v).to_string())
                .unwrap_or_default();
            events.push(Event::else_block("case", value, node, ctx.file_path));
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                walk_node(ctx, child, events);
            }
            events.push(Event::end_control_block("case", node, ctx.file_path));
        }
        "switch_default" => {
            events.push(Event::else_block("default", String::new(), node, ctx.file_path));
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                walk_node(ctx, child, events);
            }
            events.push(Event::end_control_block("default", node, ctx.file_path));
        }
        // ── Early exits ─────────────────────────────────────────────────────────
        "return_statement" => {
            let mut cursor = node.walk();
            let value_node = node
                .children(&mut cursor)
                .find(|c| c.kind() != "return" && c.is_named());
            let (value, value_type) = match value_node {
                Some(v) => (ctx.get_text(v).to_string(), js_classify_value_type(v.kind())),
                None => (String::new(), "none"),
            };
            events.push(Event::return_stmt(value, value_type, node, ctx.file_path));
            // Still recurse to capture calls/uses in the return expression.
            let mut cursor2 = node.walk();
            for child in node.children(&mut cursor2) {
                if child.kind() != "return" {
                    walk_node(ctx, child, events);
                }
            }
        }
        "throw_statement" => {
            let mut cursor = node.walk();
            let exc = node
                .children(&mut cursor)
                .find(|c| c.kind() != "throw" && c.is_named())
                .map(|n| ctx.get_text(n).to_string())
                .unwrap_or_default();
            events.push(Event::raise_statement(exc, node, ctx.file_path));
            let mut cursor2 = node.walk();
            for child in node.children(&mut cursor2) {
                if child.kind() != "throw" {
                    walk_node(ctx, child, events);
                }
            }
        }
        "break_statement" => {
            events.push(Event::break_statement(node, ctx.file_path));
        }
        "continue_statement" => {
            events.push(Event::continue_statement(node, ctx.file_path));
        }
        // ── Literals ────────────────────────────────────────────────────────────
        "number" => {
            events.push(Event::literal(
                ctx.get_text(node).to_string(),
                "number",
                node,
                ctx.file_path,
            ));
        }
        "string" | "template_string" => {
            events.push(Event::literal(
                ctx.get_text(node).to_string(),
                "string",
                node,
                ctx.file_path,
            ));
        }
        "true" | "false" => {
            events.push(Event::literal(
                ctx.get_text(node).to_string(),
                "boolean",
                node,
                ctx.file_path,
            ));
        }
        "null" => {
            events.push(Event::literal("null".to_string(), "null", node, ctx.file_path));
        }
        // ── Augmented assignment (+=, -=, etc.) ─────────────────────────────────
        // LHS is both read and written; emit UseName for the read, then Assignment.
        "augmented_assignment_expression" => {
            if let Some(left) = node.child_by_field_name("left") {
                if left.kind() == "identifier" {
                    let name = ctx.get_text(left).to_string();
                    events.push(Event::use_name(name, left, ctx.file_path));
                }
            }
            if let Some(right) = node.child_by_field_name("right") {
                walk_node(ctx, right, events);
            }
            if let (Some(left), Some(right)) = (
                node.child_by_field_name("left"),
                node.child_by_field_name("right"),
            ) {
                if left.kind() == "identifier" {
                    let name = ctx.get_text(left).to_string();
                    let qualname = ctx.build_qualname(&name);
                    let value = ctx.get_text(right).to_string();
                    let value_type = js_classify_value_type(right.kind());
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
        // ── for...of / for...in ─────────────────────────────────────────────────
        "for_in_statement" | "for_of_statement" => {
            emit_for_loop(ctx, node, events);
        }
        // ── catch clause ────────────────────────────────────────────────────────
        "catch_clause" => {
            emit_catch_clause(ctx, node, events);
        }
        // ── TypeScript-specific ─────────────────────────────────────────────────
        "interface_declaration" => {
            emit_interface(ctx, node, events);
        }
        "type_alias_declaration" => {
            emit_type_alias(ctx, node, events);
        }
        "enum_declaration" => {
            emit_enum(ctx, node, events);
        }
        // ── Member expressions (obj.prop) — emit AttributeAccess + recurse object only
        // The property is a member name, not a scope-resolvable reference; emit_identifier_use
        // already filters it out when walking via the default arm, but with this explicit arm
        // we skip the default recursion entirely and only walk `object`.
        "member_expression" => {
            let obj = node
                .child_by_field_name("object")
                .map(|n| ctx.get_text(n).to_string())
                .unwrap_or_default();
            let prop = node
                .child_by_field_name("property")
                .map(|n| ctx.get_text(n).to_string())
                .unwrap_or_default();
            events.push(Event::attribute_access(obj, prop, node, ctx.file_path));
            // Recurse into object only — property is not a scope name
            if let Some(obj_node) = node.child_by_field_name("object") {
                walk_node(ctx, obj_node, events);
            }
        }
        // ── Decorators (@foo, @foo.bar, @foo()) ─────────────────────────────────
        "decorator" => {
            emit_decorator(ctx, node, events);
        }
        // ── Await expression — pass through to value ─────────────────────────────
        "await_expression" => {
            if let Some(val) = node.child_by_field_name("value") {
                walk_node(ctx, val, events);
            }
        }
        // ── TypeScript typed parameters — walk type annotation only ──────────────
        // The parameter name is a definition (excluded in emit_identifier_use), so
        // we only recurse into the type child to capture type reference UseName events.
        "required_parameter" | "optional_parameter" => {
            if let Some(type_node) = node.child_by_field_name("type") {
                walk_node(ctx, type_node, events);
            }
        }
        // ── Identifier uses ─────────────────────────────────────────────────────
        // type_identifier covers TypeScript type references (const x: SomeType).
        // jsx_identifier covers JSX element names (<MyComponent />).
        "identifier" | "type_identifier" | "jsx_identifier" => {
            emit_identifier_use(ctx, node, events);
        }
        _ => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                walk_node(ctx, child, events);
            }
        }
    }
}

// ============================================================================
// Scope emitters
// ============================================================================

fn emit_function(ctx: &mut Ctx, node: Node, events: &mut Vec<Event>) {
    let name = match node.child_by_field_name("name") {
        Some(n) => ctx.get_text(n).to_string(),
        None => return, // anonymous; handled by emit_variable_declarator
    };

    let qualname = ctx.build_qualname(&name);

    let params = node
        .child_by_field_name("parameters")
        .map(|p| collect_params(ctx, p))
        .unwrap_or_default();

    let docstring = extract_jsdoc(node, ctx.source);
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
        params,
        vec![],
        docstring,
        node,
        ctx.file_path,
    ));

    ctx.scope_stack.push((name.clone(), qualname.clone()));

    // Walk TypeScript typed parameter annotations and return type so that type
    // references (e.g. `SomeType` in `fn f(x: SomeType): ReturnType`) emit UseName.
    if let Some(params) = node.child_by_field_name("parameters") {
        let mut cursor = params.walk();
        for child in params.children(&mut cursor) {
            if matches!(child.kind(), "required_parameter" | "optional_parameter") {
                walk_node(ctx, child, events);
            }
        }
    }
    if let Some(rt) = node.child_by_field_name("return_type") {
        walk_node(ctx, rt, events);
    }

    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            walk_node(ctx, child, events);
        }
    }

    ctx.scope_stack.pop();
    events.push(Event::exit_scope(
        ScopeType::Function,
        name,
        qualname,
        node,
        ctx.file_path,
    ));
}

fn emit_class(ctx: &mut Ctx, node: Node, events: &mut Vec<Event>) {
    let name = match node.child_by_field_name("name") {
        Some(n) => ctx.get_text(n).to_string(),
        None => return,
    };

    let qualname = ctx.build_qualname(&name);

    let bases = node
        .child_by_field_name("superclass")
        .map(|n| vec![ctx.get_text(n).to_string()])
        .unwrap_or_default();

    let docstring = extract_jsdoc(node, ctx.source);
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
        vec![],
        bases,
        docstring,
        node,
        ctx.file_path,
    ));

    ctx.scope_stack.push((name.clone(), qualname.clone()));

    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            match child.kind() {
                "method_definition" => emit_method(ctx, child, events),
                "public_field_definition" | "field_definition" => {
                    emit_class_field(ctx, child, events);
                }
                _ => {}
            }
        }
    }

    ctx.scope_stack.pop();
    events.push(Event::exit_scope(
        ScopeType::Class,
        name,
        qualname,
        node,
        ctx.file_path,
    ));
}

fn emit_method(ctx: &mut Ctx, node: Node, events: &mut Vec<Event>) {
    let name = match node.child_by_field_name("name") {
        Some(n) => ctx.get_text(n).to_string(),
        None => return,
    };

    let qualname = ctx.build_qualname(&name);

    let params = node
        .child_by_field_name("parameters")
        .map(|p| collect_params(ctx, p))
        .unwrap_or_default();

    let docstring = extract_jsdoc(node, ctx.source);
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
        params,
        vec![],
        docstring,
        node,
        ctx.file_path,
    ));

    ctx.scope_stack.push((name.clone(), qualname.clone()));

    // Walk TypeScript typed parameter annotations and return type
    if let Some(params) = node.child_by_field_name("parameters") {
        let mut cursor = params.walk();
        for child in params.children(&mut cursor) {
            if matches!(child.kind(), "required_parameter" | "optional_parameter") {
                walk_node(ctx, child, events);
            }
        }
    }
    if let Some(rt) = node.child_by_field_name("return_type") {
        walk_node(ctx, rt, events);
    }

    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            walk_node(ctx, child, events);
        }
    }

    ctx.scope_stack.pop();
    events.push(Event::exit_scope(
        ScopeType::Function,
        name,
        qualname,
        node,
        ctx.file_path,
    ));
}

fn emit_class_field(ctx: &mut Ctx, node: Node, events: &mut Vec<Event>) {
    if let Some(name_node) = node.child_by_field_name("name") {
        let name = ctx.get_text(name_node).to_string();
        let qualname = ctx.build_qualname(&name);
        events.push(Event::define_name(name, qualname, "variable", node, ctx.file_path));
    }
}

fn emit_interface(ctx: &mut Ctx, node: Node, events: &mut Vec<Event>) {
    let name = match node.child_by_field_name("name") {
        Some(n) => ctx.get_text(n).to_string(),
        None => return,
    };

    let qualname = ctx.build_qualname(&name);

    // Collect base interfaces from `extends` clause
    let bases = collect_interface_bases(ctx, node);

    let docstring = extract_jsdoc(node, ctx.source);
    events.push(Event::define_name(
        name.clone(),
        qualname.clone(),
        "interface",
        node,
        ctx.file_path,
    ));
    events.push(Event::enter_scope_with_docstring(
        ScopeType::Interface,
        name.clone(),
        qualname.clone(),
        vec![],
        bases,
        docstring,
        node,
        ctx.file_path,
    ));

    ctx.scope_stack.push((name.clone(), qualname.clone()));

    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            match child.kind() {
                "method_signature" | "property_signature" | "index_signature" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let member_name = ctx.get_text(name_node).to_string();
                        let member_qualname = ctx.build_qualname(&member_name);
                        let node_type = if child.kind() == "method_signature" {
                            "function"
                        } else {
                            "variable"
                        };
                        events.push(Event::define_name(
                            member_name,
                            member_qualname,
                            node_type,
                            child,
                            ctx.file_path,
                        ));
                    }
                }
                _ => {}
            }
        }
    }

    ctx.scope_stack.pop();
    events.push(Event::exit_scope(
        ScopeType::Interface,
        name,
        qualname,
        node,
        ctx.file_path,
    ));
}

fn emit_type_alias(ctx: &mut Ctx, node: Node, events: &mut Vec<Event>) {
    let name = match node.child_by_field_name("name") {
        Some(n) => ctx.get_text(n).to_string(),
        None => return,
    };

    let qualname = ctx.build_qualname(&name);

    // Always define the type alias name regardless of its shape.
    events.push(Event::define_name(
        name.clone(),
        qualname.clone(),
        "interface",
        node,
        ctx.file_path,
    ));

    // Only create interface scope nodes for object-shape types, not primitive aliases
    // e.g. `type ID = string` is skipped; `type Foo = { bar: string }` is included
    let value_node = match node.child_by_field_name("value") {
        Some(v) => v,
        None => return,
    };

    if value_node.kind() != "object_type" {
        return;
    }

    let docstring = extract_jsdoc(node, ctx.source);
    events.push(Event::enter_scope_with_docstring(
        ScopeType::Interface,
        name.clone(),
        qualname.clone(),
        vec![],
        vec![],
        docstring,
        node,
        ctx.file_path,
    ));

    ctx.scope_stack.push((name.clone(), qualname.clone()));

    // Walk object type members
    let mut cursor = value_node.walk();
    for child in value_node.children(&mut cursor) {
        match child.kind() {
            "method_signature" | "property_signature" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let field_name = ctx.get_text(name_node).to_string();
                    let field_qualname = ctx.build_qualname(&field_name);
                    let node_type = if child.kind() == "method_signature" { "function" } else { "variable" };
                    events.push(Event::define_name(
                        field_name,
                        field_qualname,
                        node_type,
                        child,
                        ctx.file_path,
                    ));
                }
            }
            _ => {}
        }
    }

    ctx.scope_stack.pop();
    events.push(Event::exit_scope(
        ScopeType::Interface,
        name,
        qualname,
        node,
        ctx.file_path,
    ));
}

fn emit_enum(ctx: &mut Ctx, node: Node, events: &mut Vec<Event>) {
    let name = match node.child_by_field_name("name") {
        Some(n) => ctx.get_text(n).to_string(),
        None => return,
    };

    let qualname = ctx.build_qualname(&name);

    // Enums are class-like in the graph
    let docstring = extract_jsdoc(node, ctx.source);
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
        vec![],
        vec![],
        docstring,
        node,
        ctx.file_path,
    ));

    ctx.scope_stack.push((name.clone(), qualname.clone()));

    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            if child.kind() == "enum_member" {
                // First child of enum_member is the property_identifier
                let mut mc = child.walk();
                for member_child in child.children(&mut mc) {
                    if matches!(member_child.kind(), "property_identifier" | "string") {
                        let member_name = ctx
                            .get_text(member_child)
                            .trim_matches(|c| c == '"' || c == '\'')
                            .to_string();
                        if !member_name.is_empty() {
                            let member_qualname = ctx.build_qualname(&member_name);
                            events.push(Event::define_name(
                                member_name,
                                member_qualname,
                                "variable",
                                member_child,
                                ctx.file_path,
                            ));
                        }
                        break;
                    }
                }
            }
        }
    }

    ctx.scope_stack.pop();
    events.push(Event::exit_scope(
        ScopeType::Class,
        name,
        qualname,
        node,
        ctx.file_path,
    ));
}

// ============================================================================
// Variable / import / call emitters
// ============================================================================

fn emit_variable_declaration(ctx: &mut Ctx, node: Node, events: &mut Vec<Event>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "variable_declarator" {
            emit_variable_declarator(ctx, child, events);
        }
    }
}

fn emit_variable_declarator(ctx: &mut Ctx, node: Node, events: &mut Vec<Event>) {
    let name_node = match node.child_by_field_name("name") {
        Some(n) => n,
        None => return,
    };

    if name_node.kind() == "identifier" {
        let name = ctx.get_text(name_node).to_string();

        // If the value is a function/arrow, create a named function scope.
        if let Some(value_node) = node.child_by_field_name("value") {
            match value_node.kind() {
                "function" | "arrow_function" | "generator_function" => {
                    let qualname = ctx.build_qualname(&name);

                    let params = value_node
                        .child_by_field_name("parameters")
                        .or_else(|| value_node.child_by_field_name("parameter"))
                        .map(|p| collect_params(ctx, p))
                        .unwrap_or_default();

                    // JSDoc precedes the variable declaration (parent of this declarator)
                    let decl_node = node.parent().unwrap_or(node);
                    let docstring = extract_jsdoc(decl_node, ctx.source);
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
                        params,
                        vec![],
                        docstring,
                        value_node,
                        ctx.file_path,
                    ));

                    ctx.scope_stack.push((name.clone(), qualname.clone()));

                    // Arrow function body can be an expression or a statement block.
                    if let Some(body) = value_node.child_by_field_name("body") {
                        if body.kind() == "statement_block" {
                            let mut cursor = body.walk();
                            for child in body.children(&mut cursor) {
                                walk_node(ctx, child, events);
                            }
                        } else {
                            // Expression body (e.g. `() => someCall()`): walk it to
                            // capture UseName / CallExpression events.
                            walk_node(ctx, body, events);
                        }
                    }

                    ctx.scope_stack.pop();
                    events.push(Event::exit_scope(
                        ScopeType::Function,
                        name,
                        qualname,
                        value_node,
                        ctx.file_path,
                    ));
                    return;
                }
                _ => {}
            }
        }

        // Simple variable binding.
        let qualname = ctx.build_qualname(&name);
        events.push(Event::define_name(name.clone(), qualname.clone(), "variable", node, ctx.file_path));
        if let Some(value_node) = node.child_by_field_name("value") {
            // Walk RHS first (post-order: inner call events fire before the assignment event)
            walk_node(ctx, value_node, events);
            // Emit Assignment so the graph builder can create has-a / calls edges from
            // this variable node to the RHS target (mirrors Python's emit_assignment_post_event).
            let value_text = ctx.get_text(value_node).to_string();
            let value_type = js_classify_value_type(value_node.kind());
            events.push(Event::assignment(name, qualname, value_text, value_type, node, ctx.file_path));
        }
    } else {
        // Destructuring pattern — collect all bound names and walk the RHS.
        let bindings = collect_binding_names(ctx, name_node);
        let value_node = node.child_by_field_name("value");
        // Walk RHS first
        if let Some(v) = value_node {
            walk_node(ctx, v, events);
        }
        // Emit DefineName + Assignment for each bound name, pointing to the shared RHS.
        let (value_text, value_type) = match value_node {
            Some(v) => (ctx.get_text(v).to_string(), js_classify_value_type(v.kind())),
            None => (String::new(), "none"),
        };
        for (bound_name, binding_node) in bindings {
            let qualname = ctx.build_qualname(&bound_name);
            events.push(Event::define_name(
                bound_name.clone(),
                qualname.clone(),
                "variable",
                binding_node,
                ctx.file_path,
            ));
            if !value_text.is_empty() {
                events.push(Event::assignment(
                    bound_name,
                    qualname,
                    value_text.clone(),
                    value_type,
                    node,
                    ctx.file_path,
                ));
            }
        }
    }
}

fn emit_import(ctx: &mut Ctx, node: Node, events: &mut Vec<Event>) {
    let mut module = String::new();
    let mut names: Vec<String> = Vec::new();
    let mut aliases: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "string" => {
                module = ctx
                    .get_text(child)
                    .trim_matches(|c| c == '"' || c == '\'' || c == '`')
                    .to_string();
            }
            "import_clause" => {
                let mut c = child.walk();
                for clause_child in child.children(&mut c) {
                    match clause_child.kind() {
                        "identifier" => {
                            // Default import: `import Foo from 'mod'`
                            names.push(ctx.get_text(clause_child).to_string());
                        }
                        "named_imports" => {
                            // Named imports: `import { Foo, Bar } from 'mod'`
                            // Also handles `import { Foo as F } from 'mod'`
                            let mut nc = clause_child.walk();
                            for spec in clause_child.children(&mut nc) {
                                if spec.kind() == "import_specifier" {
                                    if let Some(nm) = spec.child_by_field_name("name") {
                                        let original = ctx.get_text(nm).to_string();
                                        names.push(original.clone());
                                        if let Some(alias_node) = spec.child_by_field_name("alias") {
                                            let alias = ctx.get_text(alias_node).to_string();
                                            aliases.insert(original, alias);
                                        }
                                    }
                                }
                            }
                        }
                        "namespace_import" => {
                            // `import * as Foo from 'mod'`
                            names.push("*".to_string());
                            // Capture alias for `* as Name`
                            if let Some(alias_node) = clause_child.child_by_field_name("name") {
                                let alias = ctx.get_text(alias_node).to_string();
                                aliases.insert("*".to_string(), alias);
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    if !module.is_empty() {
        // Resolve path-alias imports to canonical project-rooted qualnames so
        // they match the module paths produced by `derive_module_path`.
        // e.g. `@app/store` → `frontend.src.app.store`.
        let module = normalize_import(&module, ctx.file_path, &ctx.project_info);
        events.push(Event::import_statement(module, names, aliases, false, node, ctx.file_path));
    }
}

fn emit_call(ctx: &mut Ctx, node: Node, events: &mut Vec<Event>) {
    // call_expression: function field; new_expression: constructor field
    let callee_node = node
        .child_by_field_name("function")
        .or_else(|| node.child_by_field_name("constructor"));

    if let Some(func_node) = callee_node {
        let callee = ctx.get_text(func_node).to_string();

        let mut arguments = Vec::new();
        if let Some(args_node) = node.child_by_field_name("arguments") {
            let mut cursor = args_node.walk();
            for child in args_node.children(&mut cursor) {
                match child.kind() {
                    "(" | ")" | "," => {}
                    _ => {
                        let text = ctx.get_text(child).to_string();
                        if !text.is_empty() {
                            arguments.push(text);
                        }
                    }
                }
            }
        }

        events.push(Event::call_expression(callee, arguments, node, ctx.file_path));
    }
}

fn emit_export(ctx: &mut Ctx, node: Node, events: &mut Vec<Event>) {
    // Check for re-export: `export { X } from 'source'` or `export * from 'source'`
    // These have a `string` child for the source module.
    let mut source_module: Option<String> = None;
    let mut export_names: Vec<String> = Vec::new();
    let mut has_export_clause = false;
    let mut has_namespace_export = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "string" => {
                source_module = Some(
                    ctx.get_text(child)
                        .trim_matches(|c| c == '"' || c == '\'' || c == '`')
                        .to_string(),
                );
            }
            "export_clause" => {
                has_export_clause = true;
                let mut ec = child.walk();
                for spec in child.children(&mut ec) {
                    if spec.kind() == "export_specifier" {
                        if let Some(nm) = spec.child_by_field_name("name") {
                            export_names.push(ctx.get_text(nm).to_string());
                        }
                    }
                }
            }
            "namespace_export" => {
                has_namespace_export = true;
                export_names.push("*".to_string());
            }
            _ => {}
        }
    }

    if let Some(src) = source_module {
        // This is a re-export statement — emit an import event for the dependency.
        if has_export_clause || has_namespace_export {
            let resolved = normalize_import(&src, ctx.file_path, &ctx.project_info);
            events.push(Event::import_statement(resolved, export_names, std::collections::HashMap::new(), false, node, ctx.file_path));
            return;
        }
    }

    // Not a re-export — walk child declarations normally.
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "function_declaration" | "generator_function_declaration" => {
                emit_function(ctx, child, events);
            }
            "class_declaration" | "abstract_class_declaration" => {
                emit_class(ctx, child, events);
            }
            "lexical_declaration" | "variable_declaration" => {
                emit_variable_declaration(ctx, child, events);
            }
            "interface_declaration" => {
                emit_interface(ctx, child, events);
            }
            "type_alias_declaration" => {
                emit_type_alias(ctx, child, events);
            }
            "enum_declaration" => {
                emit_enum(ctx, child, events);
            }
            _ => {}
        }
    }
}

/// Emit DefineName for the loop variable in `for...of` / `for...in` statements
/// and recurse into the iterable expression and loop body.
///
/// Grammar:
///   for_in_statement: `for (left in right) body`
///   for_of_statement: `for (left of right) body`
/// The `left` field holds the binding (identifier or destructuring pattern).
/// The optional `var`/`let`/`const` keyword is a sibling, not part of `left`.
fn emit_for_loop(ctx: &mut Ctx, node: Node, events: &mut Vec<Event>) {
    // Walk the `right` (iterable) first to emit use/call events for it.
    if let Some(right) = node.child_by_field_name("right") {
        walk_node(ctx, right, events);
    }

    // Bind the loop variable(s) from the `left` field.
    if let Some(left) = node.child_by_field_name("left") {
        let bindings = collect_binding_names(ctx, left);
        for (name, binding_node) in bindings {
            let qualname = ctx.build_qualname(&name);
            events.push(Event::define_name(
                name,
                qualname,
                "variable",
                binding_node,
                ctx.file_path,
            ));
        }
    }

    // Recurse into the loop body.
    if let Some(body) = node.child_by_field_name("body") {
        walk_node(ctx, body, events);
    }
}

/// Emit DefineName for the caught exception variable in a `catch` clause and
/// recurse into the clause body.
///
/// Grammar:
///   catch_clause: `catch (parameter?) body`
/// The `parameter` field is the bound variable (can be a pattern).
fn emit_catch_clause(ctx: &mut Ctx, node: Node, events: &mut Vec<Event>) {
    if let Some(param) = node.child_by_field_name("parameter") {
        let bindings = collect_binding_names(ctx, param);
        for (name, binding_node) in bindings {
            let qualname = ctx.build_qualname(&name);
            events.push(Event::define_name(
                name,
                qualname,
                "variable",
                binding_node,
                ctx.file_path,
            ));
        }
    }

    // Recurse into body.
    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            walk_node(ctx, child, events);
        }
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn collect_params(ctx: &Ctx, params_node: Node) -> Vec<String> {
    let mut params = Vec::new();
    let mut cursor = params_node.walk();
    for child in params_node.children(&mut cursor) {
        match child.kind() {
            "identifier" => {
                params.push(ctx.get_text(child).to_string());
            }
            "assignment_pattern" => {
                // `x = default` — use the left-hand identifier
                if let Some(left) = child.child_by_field_name("left") {
                    if left.kind() == "identifier" {
                        params.push(ctx.get_text(left).to_string());
                    }
                }
            }
            "rest_pattern" => {
                // `...args` — find the inner identifier
                let mut rc = child.walk();
                for rest_child in child.children(&mut rc) {
                    if rest_child.kind() == "identifier" {
                        params.push(ctx.get_text(rest_child).to_string());
                        break;
                    }
                }
            }
            // TypeScript: `name: Type` or `name?: Type`
            "required_parameter" | "optional_parameter" => {
                if let Some(pattern) = child.child_by_field_name("pattern") {
                    if pattern.kind() == "identifier" {
                        params.push(ctx.get_text(pattern).to_string());
                    }
                }
            }
            _ => {}
        }
    }
    params
}

/// Classify a JS/TS value expression node kind into a semantic category.
fn js_classify_value_type(kind: &str) -> &'static str {
    match kind {
        "number" | "string" | "template_string" | "true" | "false" | "null" => "literal",
        "call_expression" | "new_expression" | "await_expression" => "call",
        "identifier" => "name",
        "member_expression" | "subscript_expression" => "attribute",
        _ => "expression",
    }
}

/// Recursively collect all identifier bindings introduced by a destructuring pattern node.
///
/// Handles: `object_pattern`, `array_pattern`, `rest_pattern`, `pair_pattern`,
/// `assignment_pattern` (default values), and bare `identifier` / `shorthand_property_identifier_pattern`.
fn collect_binding_names<'a>(ctx: &Ctx<'a>, node: Node<'a>) -> Vec<(String, Node<'a>)> {
    let mut out = Vec::new();
    collect_binding_names_inner(ctx, node, &mut out);
    out
}

fn collect_binding_names_inner<'a>(
    ctx: &Ctx<'a>,
    node: Node<'a>,
    out: &mut Vec<(String, Node<'a>)>,
) {
    match node.kind() {
        // Simple binding: `x` in `const x = ...` or `const { x } = ...`
        "identifier" | "shorthand_property_identifier_pattern" => {
            let name = ctx.get_text(node).to_string();
            if !name.is_empty() && name != "_" {
                out.push((name, node));
            }
        }
        // Object / array destructuring containers
        "object_pattern" | "array_pattern" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                collect_binding_names_inner(ctx, child, out);
            }
        }
        // `...rest`
        "rest_pattern" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "identifier" {
                    let name = ctx.get_text(child).to_string();
                    if !name.is_empty() {
                        out.push((name, child));
                    }
                }
            }
        }
        // `{ key: pattern }` — bind the value-side pattern
        "pair_pattern" => {
            if let Some(value) = node.child_by_field_name("value") {
                collect_binding_names_inner(ctx, value, out);
            }
        }
        // `x = default` in a destructuring context — bind the left side
        "assignment_pattern" => {
            if let Some(left) = node.child_by_field_name("left") {
                collect_binding_names_inner(ctx, left, out);
            }
        }
        _ => {}
    }
}

fn collect_interface_bases(ctx: &Ctx, node: Node) -> Vec<String> {
    let mut bases = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        // `extends_type_clause` contains the base interface names
        if child.kind() == "extends_type_clause" {
            let mut c = child.walk();
            for base in child.children(&mut c) {
                if matches!(base.kind(), "type_identifier" | "identifier") {
                    bases.push(ctx.get_text(base).to_string());
                }
            }
        }
    }
    bases
}
