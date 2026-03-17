# Rust Analyzer — Rules and Invariants

## Graph edge completeness

The goal is 100% accurate static dependency resolution down to the variable/attribute level. Every cross-module reference must produce an edge. Import-level edges alone are not acceptable.

## Walker invariants — do not break these

### UseName emission is mandatory for all language walkers

Every language walker must emit `Event::use_name` for all identifier references in expression (USE) contexts. This feeds `load_uses` → `resolve_name_legb` → the `import_bindings` table → actual definition edges.

**JavaScript walker** (`src/javascript/mod.rs`):
- `walk_node` has an explicit `"identifier" | "type_identifier" | "jsx_identifier"` arm that calls `emit_identifier_use`
- `emit_identifier_use` filters out definition/binding contexts: variable declarator names, function/class/method names, import specifiers, member-expression properties (`obj.PROP`), object literal keys, type parameters
- **Do NOT remove this arm or collapse it into the `_ =>` default branch**

**Exclusion rules for JS identifiers** — these are definitions, NOT uses:
- `variable_declarator.name` field
- `function_declaration.name`, `function.name`, `generator_function.name`
- `class_declaration.name`, `class.name`
- `method_definition.name`, `method_signature.name`
- `property_signature.name`, `public_field_definition.name`
- any child of `import_clause`, `import_specifier`, `namespace_import`
- any child of `export_specifier`
- `labeled_statement.label` field
- `member_expression.property` field (these are attribute accesses, not scope names — LEGB can't resolve them)
- `pair.key` field (object literal keys)
- any child of `type_parameter`

`member_expression.object` (the `obj` in `obj.method`) IS emitted as UseName because it is a scope-resolvable reference.

### CallExpression already handles call targets

The `call_expression` arm in `walk_node` calls `emit_call` and then recurses only into `arguments`. The callee is consumed by `emit_call` → `Event::call_expression` → `load_raw_bindings` CALLS pass. Do NOT also emit UseName for the callee — `emit_identifier_use` is never called on the callee because `walk_node` is not called on it. Arguments ARE recursed, so identifier args fire UseName correctly.

### Duplicate edges are safe

`EdgeData` is stored in a `HashSet`. If UseName and CallExpression both produce the same edge, it deduplicates silently. Don't skip UseName emission to avoid "duplicates" — correctness matters more.

## Import normalization — JS aliases

`read_tsconfig_aliases_from` in `src/javascript/mod.rs` parses `tsconfig.json` `compilerOptions.paths`. Aliases may target a **file** (e.g. `"@store": ["./src/store.ts"]`) rather than a directory. The function MUST strip JS file extensions (`.ts`, `.tsx`, `.js`, `.jsx`, `.mjs`) from the final path segment before storing the `(alias, target_dir)` pair. Without this, `@store` resolves to `frontend.src.store.ts` which matches no node.

## Re-export map

`build_reexport_map` in `src/graph/loaders.rs` must use `self.lang_configs.iter().any(|cfg| cfg.is_reexport_file(file))` to identify re-export files. **Never** replace this with a hardcoded `file.ends_with("__init__.py")` check — that breaks JS `index.ts` and Rust `mod.rs` re-export resolution.

## LEGB resolution pipeline order

The pipeline in `lib.rs:build_dependency_graph` is ORDER-DEPENDENT:

1. `load_scope_tree` — populates `definitions`
2. `load_definitions` — enriches `definitions`
3. `build_reexport_map` — builds phantom→actual map from re-export files (requires `definitions`)
4. `load_import_bindings` — builds `import_bindings` table (requires `reexport_map`)
5. `load_uses` — LEGB resolution (requires `import_bindings`)
6. `load_raw_bindings` — ASSIGNED pass then CALLS pass (CALLS requires `import_bindings`)
7. `load_imports` — import-statement edges (requires `reexport_map`)

Do not reorder these steps.
