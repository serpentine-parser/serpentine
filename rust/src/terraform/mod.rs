//! Terraform/HCL language walker for serpentine.
//!
//! Walks a tree-sitter parse tree for .tf source files and emits
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

    for (i, line) in source.lines().enumerate() {
        events.push(Event::SourceLine {
            file: file_path.to_string(),
            line_number: i + 1,
            text: line.to_string(),
        });
    }

    let Some(tree) = tree else { return events };

    let root = tree.root_node();
    let ctx = ParseContext { source, file_path };
    let module_name = derive_module_name(file_path);

    events.push(Event::enter_scope(
        ScopeType::Module,
        module_name.clone(),
        module_name.clone(),
        vec![],
        vec![],
        root,
        file_path,
    ));
    events.push(Event::define_name(
        module_name.clone(),
        module_name.clone(),
        "module",
        root,
        file_path,
    ));

    walk_body(&ctx, root, &module_name, &mut events);

    events.push(Event::exit_scope(
        ScopeType::Module,
        module_name.clone(),
        module_name,
        root,
        file_path,
    ));

    events
}

fn derive_module_name(file_path: &str) -> String {
    Path::new(file_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("main")
        .to_string()
}

// ============================================================================
// Parse Context
// ============================================================================

struct ParseContext<'a> {
    source: &'a str,
    file_path: &'a str,
}

impl<'a> ParseContext<'a> {
    fn text(&self, node: Node) -> &str {
        &self.source[node.start_byte()..node.end_byte()]
    }
}

// ============================================================================
// Top-level body walker
// ============================================================================

fn walk_body(ctx: &ParseContext, node: Node, current_scope: &str, events: &mut Vec<Event>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "body" => walk_body(ctx, child, current_scope, events),
            "block" => handle_block(ctx, child, current_scope, events),
            _ => {}
        }
    }
}

const TOP_LEVEL_BLOCKS: &[&str] = &[
    "resource", "module", "variable", "data", "locals", "output", "terraform", "provider",
];

fn handle_block(ctx: &ParseContext, block: Node, _parent_scope: &str, events: &mut Vec<Event>) {
    let mut block_type: Option<String> = None;
    let mut labels: Vec<String> = Vec::new();
    let mut body: Option<Node> = None;

    let mut cursor = block.walk();
    for child in block.children(&mut cursor) {
        match child.kind() {
            "identifier" if block_type.is_none() => {
                block_type = Some(ctx.text(child).to_string());
            }
            "string_lit" => {
                let raw = ctx.text(child);
                // Strip outer double-quotes
                let content = if raw.len() >= 2 && raw.starts_with('"') && raw.ends_with('"') {
                    &raw[1..raw.len() - 1]
                } else {
                    raw
                };
                labels.push(content.to_string());
            }
            "body" => {
                body = Some(child);
            }
            _ => {}
        }
    }

    let Some(block_type) = block_type else { return };

    if !TOP_LEVEL_BLOCKS.contains(&block_type.as_str()) {
        return;
    }

    let qualname = if labels.is_empty() {
        block_type.clone()
    } else {
        format!("{}.{}", block_type, labels.join("."))
    };

    // Emit ImportStatement for module source at file scope (before entering block scope).
    if block_type == "module" {
        if let Some(body_node) = body {
            if let Some(source_val) = find_string_attribute(ctx, body_node, "source") {
                let normalized = normalize_source_path(&source_val);
                events.push(Event::import_statement(
                    normalized,
                    vec![],
                    std::collections::HashMap::new(),
                    false,
                    block,
                    ctx.file_path,
                ));
            }
        }
    }

    let scope_type = match block_type.as_str() {
        "variable" => ScopeType::Class,
        _ => ScopeType::Class,
    };

    let node_type = match block_type.as_str() {
        "variable" => "variable",
        "module" => "module",
        _ => "class",
    };

    events.push(Event::enter_scope(
        scope_type.clone(),
        qualname.clone(),
        qualname.clone(),
        vec![],
        vec![],
        block,
        ctx.file_path,
    ));
    events.push(Event::define_name(
        qualname.clone(),
        qualname.clone(),
        node_type,
        block,
        ctx.file_path,
    ));

    if let Some(body_node) = body {
        walk_block_body(ctx, body_node, &qualname, events);
    }

    events.push(Event::exit_scope(
        scope_type,
        qualname.clone(),
        qualname,
        block,
        ctx.file_path,
    ));
}

// ============================================================================
// Block body and expression walking
// ============================================================================

fn walk_block_body(ctx: &ParseContext, body: Node, scope: &str, events: &mut Vec<Event>) {
    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        match child.kind() {
            "attribute" => {
                let mut ac = child.walk();
                for attr_child in child.children(&mut ac) {
                    if attr_child.kind() == "expression" {
                        walk_expr(ctx, attr_child, scope, events);
                    }
                }
            }
            "block" => {
                // Nested blocks (lifecycle, connection, etc.) — recurse without new scope
                let mut bc = child.walk();
                for nested in child.children(&mut bc) {
                    if nested.kind() == "body" {
                        walk_block_body(ctx, nested, scope, events);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Recursively walk an expression node, emitting UseName events for
/// variable and resource traversals. Recurses into all children by default
/// so that template_interpolation and other container nodes are transparently
/// traversed (see spec §3d).
fn walk_expr(ctx: &ParseContext, node: Node, _scope: &str, events: &mut Vec<Event>) {
    // Try to collect a traversal chain starting at this node.
    if let Some((parts, traversal_child_count)) = collect_traversal(ctx, node) {
        if parts.len() >= 2 {
            if let Some(use_name) = traversal_to_use_name(&parts) {
                events.push(Event::use_name(use_name, node, ctx.file_path));
            }
        }
        // Recurse into any remaining children not consumed by the traversal
        // (e.g., an index expression after `module.vpc.outputs[...]`).
        let children: Vec<Node> = {
            let mut c = node.walk();
            node.children(&mut c).filter(|n| n.is_named()).collect()
        };
        for child in children.iter().skip(traversal_child_count) {
            walk_expr(ctx, *child, _scope, events);
        }
        return;
    }

    // Default: recurse into all named children.
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.is_named() {
            walk_expr(ctx, child, _scope, events);
        }
    }
}

// ============================================================================
// Traversal detection
// ============================================================================

/// Collect a HCL traversal chain from a node (e.g. `var.ami_id`, `aws_subnet.main.id`).
///
/// Returns `(parts, children_consumed)` where `children_consumed` is the number
/// of *direct named children* of `node` that form the traversal. The caller can
/// skip those children and recurse into the rest (e.g. an index expression).
///
/// Handles two shapes:
///   Flat:   expression( variable_expr, get_attr, ... )
///   Nested: expression( expression(...), get_attr, ... )
fn collect_traversal(ctx: &ParseContext, node: Node) -> Option<(Vec<String>, usize)> {
    let named: Vec<Node> = {
        let mut c = node.walk();
        node.children(&mut c).filter(|n| n.is_named()).collect()
    };

    let first = named.first()?;

    match first.kind() {
        "variable_expr" => {
            let root = get_identifier_child(ctx, *first)?;
            let mut parts = vec![root];
            let mut consumed = 1usize;
            for child in named.iter().skip(1) {
                if child.kind() == "get_attr" {
                    if let Some(attr) = get_identifier_child(ctx, *child) {
                        parts.push(attr);
                        consumed += 1;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
            Some((parts, consumed))
        }
        "expression" => {
            // Nested traversal: the inner expression holds the base, then more get_attrs here.
            let (mut parts, _) = collect_traversal(ctx, *first)?;
            let mut consumed = 1usize; // the inner expression counts as one child
            for child in named.iter().skip(1) {
                if child.kind() == "get_attr" {
                    if let Some(attr) = get_identifier_child(ctx, *child) {
                        parts.push(attr);
                        consumed += 1;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
            Some((parts, consumed))
        }
        _ => None,
    }
}

/// Get the first `identifier` child's text from a node.
fn get_identifier_child(ctx: &ParseContext, node: Node) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            return Some(ctx.text(child).to_string());
        }
    }
    None
}

/// Convert a traversal parts list to a qualified UseName string.
///
/// | Root     | Example              | Emitted UseName               |
/// |----------|----------------------|-------------------------------|
/// | var      | var.ami_id           | variable.ami_id               |
/// | module   | module.vpc.outputs   | module.vpc                    |
/// | data     | data.aws_ami.ubuntu  | data.aws_ami.ubuntu           |
/// | aws_*    | aws_subnet.main.id   | resource.aws_subnet.main      |
fn traversal_to_use_name(parts: &[String]) -> Option<String> {
    match parts[0].as_str() {
        "var" if parts.len() >= 2 => Some(format!("variable.{}", parts[1..].join("."))),
        "module" if parts.len() >= 2 => Some(format!("module.{}", parts[1])),
        "data" if parts.len() >= 3 => Some(format!("data.{}.{}", parts[1], parts[2])),
        "local" | "locals" => None, // local.x — skip, no definition node for locals
        "path" | "self" | "each" | "count" | "terraform" => None, // built-in meta-references
        root if is_resource_type_root(root) && parts.len() >= 2 => {
            Some(format!("resource.{}.{}", parts[0], parts[1]))
        }
        _ => None,
    }
}

/// Returns true if the identifier looks like a Terraform provider resource type root
/// (e.g. `aws_instance`, `google_compute_instance`, `azurerm_resource_group`).
/// These are distinguished by having an underscore and not being a HCL keyword.
fn is_resource_type_root(name: &str) -> bool {
    name.contains('_')
        && !matches!(
            name,
            "var" | "local" | "locals" | "module" | "data" | "path"
                | "self" | "each" | "count" | "terraform" | "null_resource"
        )
        && name.chars().next().is_some_and(|c| c.is_ascii_lowercase())
}

// ============================================================================
// Attribute value helpers
// ============================================================================

/// Find the string value of the named attribute in a body node.
fn find_string_attribute(ctx: &ParseContext, body: Node, key: &str) -> Option<String> {
    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        if child.kind() != "attribute" {
            continue;
        }
        let mut ac = child.walk();
        let attr_children: Vec<Node> = child.children(&mut ac).collect();

        let has_key = attr_children
            .iter()
            .any(|c| c.kind() == "identifier" && ctx.text(*c) == key);

        if has_key {
            for ac in &attr_children {
                if ac.kind() == "expression" || ac.kind() == "string_lit" {
                    if let Some(s) = extract_string_value(ctx, *ac) {
                        return Some(s);
                    }
                }
            }
        }
    }
    None
}

/// Recursively extract the plain string content from a node tree.
/// Returns the first string_lit leaf value found, stripping outer quotes.
fn extract_string_value(ctx: &ParseContext, node: Node) -> Option<String> {
    if node.kind() == "string_lit" {
        let raw = ctx.text(node);
        let content = if raw.len() >= 2 && raw.starts_with('"') && raw.ends_with('"') {
            &raw[1..raw.len() - 1]
        } else {
            raw
        };
        return Some(content.to_string());
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(s) = extract_string_value(ctx, child) {
            return Some(s);
        }
    }
    None
}

/// Normalize a Terraform module source path to a dotted module identifier.
///
/// - `./modules/vpc`         → `modules.vpc`
/// - `../shared/networking`  → `shared.networking`
/// - `hashicorp/consul/aws`  → `hashicorp/consul/aws` (unchanged — registry path)
fn normalize_source_path(source: &str) -> String {
    if source.starts_with("./") || source.starts_with("../") {
        let stripped = source
            .trim_start_matches("./")
            .trim_start_matches("../");
        stripped.replace('/', ".")
    } else {
        // Registry module or absolute path — keep as-is; graph builder classifies as ThirdParty.
        source.to_string()
    }
}
