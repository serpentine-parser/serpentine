//! Serpentine Analyzer - Multi-language source code analysis.
//!
//! This crate provides Python bindings for analyzing source code files.
//! It uses tree-sitter for parsing and a message bus architecture to
//! distribute semantic events to subscribers.

// Allow non_local_definitions warning from PyO3 macro (fixed in newer PyO3 versions)
#![allow(non_local_definitions)]

mod events;
mod graph;
mod javascript;
mod message_bus;
mod python;
mod rust_lang;
mod subscribers;
mod terraform;

use crate::javascript::{parse as parse_javascript, JsLang};
use crate::javascript::config::JsConfig;
use crate::message_bus::{MessageBus, SubscriberResult};
use crate::graph::GraphBuilder;
use crate::python::parse as parse_python;
use crate::python::config::PythonConfig;
use crate::rust_lang::parse as parse_rust;
use crate::terraform::parse as parse_terraform;
use crate::terraform::config::TerraformConfig;
use crate::subscribers::{
    DecoratorsSubscriberFactory, PdgSubscriberFactory, CodeSnippetSubscriberFactory,
    DefinitionsSubscriberFactory, EventCounterSubscriberFactory, ImportsSubscriberFactory,
    RawBindingsSubscriberFactory, ScopeTreeSubscriberFactory, UsesSubscriberFactory,
};

use rayon::prelude::*;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

use tree_sitter::{Language, Parser, Tree};

use tree_sitter_javascript::LANGUAGE as JAVASCRIPT_LANGUAGE;
use tree_sitter_python::LANGUAGE as PYTHON_LANGUAGE;
use tree_sitter_rust::LANGUAGE as RUST_LANGUAGE;
use tree_sitter_typescript::{LANGUAGE_TSX, LANGUAGE_TYPESCRIPT};


// ============================================================================
// Language Support
// ============================================================================

/// Supported source languages.
#[derive(Clone, Copy)]
enum Lang {
    Python,
    /// Plain JavaScript or JSX (tree-sitter-javascript handles both)
    JavaScript,
    /// TypeScript (.ts)
    TypeScript,
    /// TypeScript with JSX (.tsx)
    Tsx,
    /// Rust (.rs)
    Rust,
    /// Terraform/HCL (.tf)
    Terraform,
}

impl Lang {
    /// Detect language from file extension.
    fn from_extension(path: &Path) -> Option<Self> {
        match path.extension().and_then(|s| s.to_str()) {
            Some("py") => Some(Lang::Python),
            Some("js") | Some("jsx") | Some("mjs") | Some("cjs") => Some(Lang::JavaScript),
            Some("ts") => Some(Lang::TypeScript),
            Some("tsx") => Some(Lang::Tsx),
            Some("rs") => Some(Lang::Rust),
            Some("tf") => Some(Lang::Terraform),
            _ => None,
        }
    }

    /// Get the tree-sitter language for parsing.
    fn language(&self) -> Language {
        match self {
            Lang::Python => PYTHON_LANGUAGE.into(),
            Lang::JavaScript => JAVASCRIPT_LANGUAGE.into(),
            Lang::TypeScript => LANGUAGE_TYPESCRIPT.into(),
            Lang::Tsx => LANGUAGE_TSX.into(),
            Lang::Rust => RUST_LANGUAGE.into(),
            Lang::Terraform => tree_sitter_hcl::LANGUAGE.into(),
        }
    }

    /// Map to the JsLang variant used by the JS/TS walker.
    fn js_lang(&self) -> Option<JsLang> {
        match self {
            Lang::JavaScript => Some(JsLang::JavaScript),
            Lang::TypeScript => Some(JsLang::TypeScript),
            Lang::Tsx => Some(JsLang::Tsx),
            Lang::Python | Lang::Rust | Lang::Terraform => None,
        }
    }
}

// ============================================================================
// File Entry - Tracks a single file's state
// ============================================================================

/// Tracks parsing state for a single source file.
struct FileEntry {
    parser: Parser,
    tree: Option<Tree>,
    source: String,
    source_hash: u64,
    message_bus: MessageBus,
    lang: Lang,
    file_path: String,
    /// Cached subscriber results from last parse
    cached_results: Vec<SubscriberResult>,
}

impl FileEntry {
    fn new(lang: Lang, source: String, file_path: String) -> Self {
        let mut parser = Parser::new();
        parser.set_language(&lang.language()).unwrap();
        let source_hash = Self::compute_hash(&source);
        let tree = parser.parse(&source, None);

        // Create message bus with subscriber factories
        let mut message_bus = MessageBus::new();
        message_bus.register(EventCounterSubscriberFactory::new("counter"));
        message_bus.register(ScopeTreeSubscriberFactory::new("scope_tree"));
        message_bus.register(DefinitionsSubscriberFactory::new("definitions"));
        message_bus.register(UsesSubscriberFactory::new("uses"));
        message_bus.register(RawBindingsSubscriberFactory::new("raw_bindings"));
        message_bus.register(ImportsSubscriberFactory::new("imports"));
        message_bus.register(DecoratorsSubscriberFactory::new("decorators"));
        message_bus.register(PdgSubscriberFactory::new("pdg"));
        message_bus.register(CodeSnippetSubscriberFactory::new("code_snippet"));

        FileEntry {
            parser,
            tree,
            source,
            source_hash,
            lang,
            message_bus,
            file_path,
            cached_results: Vec::new(),
        }
    }

    /// Restore a FileEntry from serialized subscriber results without re-parsing.
    /// The source is left empty and source_hash is 0; a future update() call will
    /// detect any content change and trigger a normal re-parse.
    fn from_results_json(lang: Lang, file_path: String, results_json: &str) -> Result<Self, String> {
        let mut parser = Parser::new();
        parser.set_language(&lang.language()).unwrap();

        let mut message_bus = MessageBus::new();
        message_bus.register(EventCounterSubscriberFactory::new("counter"));
        message_bus.register(ScopeTreeSubscriberFactory::new("scope_tree"));
        message_bus.register(DefinitionsSubscriberFactory::new("definitions"));
        message_bus.register(UsesSubscriberFactory::new("uses"));
        message_bus.register(RawBindingsSubscriberFactory::new("raw_bindings"));
        message_bus.register(ImportsSubscriberFactory::new("imports"));
        message_bus.register(DecoratorsSubscriberFactory::new("decorators"));
        message_bus.register(PdgSubscriberFactory::new("pdg"));
        message_bus.register(CodeSnippetSubscriberFactory::new("code_snippet"));

        let values: Vec<serde_json::Value> =
            serde_json::from_str(results_json).map_err(|e| format!("results JSON parse error: {e}"))?;

        let mut cached_results = Vec::with_capacity(values.len());
        for v in values {
            let subscriber_name = v["subscriber"]
                .as_str()
                .ok_or_else(|| "missing 'subscriber' field in cached result".to_string())?
                .to_string();
            let data = v["data"].clone();
            cached_results.push(SubscriberResult { subscriber_name, data });
        }

        Ok(FileEntry {
            parser,
            tree: None,
            source: String::new(),
            source_hash: 0,
            lang,
            message_bus,
            file_path,
            cached_results,
        })
    }

    fn compute_hash(content: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        hasher.finish()
    }

    /// Update the file with new source content.
    /// Returns true if content changed and was re-parsed.
    fn update(&mut self, source: String) -> Result<bool, String> {
        let new_hash = Self::compute_hash(&source);

        if new_hash == self.source_hash {
            return Ok(false);
        }

        self.source_hash = new_hash;
        self.tree = self.parser.parse(&source, None);
        self.source = source;

        self.process_and_cache()?;
        Ok(true)
    }

    /// Parse the current source and cache subscriber results.
    fn process_and_cache(&mut self) -> Result<(), String> {
        let events = match self.lang {
            Lang::Python => parse_python(&self.source, &self.tree, &self.file_path),
            Lang::JavaScript | Lang::TypeScript | Lang::Tsx => {
                let js_lang = self.lang.js_lang().unwrap_or(JsLang::JavaScript);
                parse_javascript(&self.source, &self.tree, &self.file_path, js_lang)
            }
            Lang::Rust => parse_rust(&self.source, &self.tree, &self.file_path),
            Lang::Terraform => parse_terraform(&self.source, &self.tree, &self.file_path),
        };

        self.cached_results = self.message_bus.publish_events(events)?;
        Ok(())
    }

    /// Get the cached subscriber results.
    fn get_results(&self) -> &[SubscriberResult] {
        &self.cached_results
    }
}

// ============================================================================
// Python Bindings
// ============================================================================

/// Manages multiple source files and their analysis state.
#[pyclass]
pub struct FileManager {
    files: HashMap<PathBuf, FileEntry>,
    /// Persistent graph builder. None until first call to build_dependency_graph.
    graph_builder: Option<GraphBuilder>,
}

/// Per-type subscriber data tagged with the originating file path.
struct PerFileData {
    scope_trees: Vec<(PathBuf, serde_json::Value)>,
    definitions: Vec<(PathBuf, serde_json::Value)>,
    uses: Vec<(PathBuf, serde_json::Value)>,
    raw_bindings: Vec<(PathBuf, serde_json::Value)>,
    imports: Vec<(PathBuf, serde_json::Value)>,
    decorators: Vec<(PathBuf, serde_json::Value)>,
    pdgs: Vec<(PathBuf, serde_json::Value)>,
    code_snippets: Vec<(PathBuf, serde_json::Value)>,
}

impl PerFileData {
    fn new() -> Self {
        PerFileData {
            scope_trees: Vec::new(),
            definitions: Vec::new(),
            uses: Vec::new(),
            raw_bindings: Vec::new(),
            imports: Vec::new(),
            decorators: Vec::new(),
            pdgs: Vec::new(),
            code_snippets: Vec::new(),
        }
    }

    fn collect_from(files: &HashMap<PathBuf, FileEntry>) -> Self {
        let mut data = Self::new();
        for (path, entry) in files {
            for result in entry.get_results() {
                let v = result.data.clone();
                match result.subscriber_name.as_str() {
                    "scope_tree" => data.scope_trees.push((path.clone(), v)),
                    "definitions" => data.definitions.push((path.clone(), v)),
                    "uses" => data.uses.push((path.clone(), v)),
                    "raw_bindings" => data.raw_bindings.push((path.clone(), v)),
                    "imports" => data.imports.push((path.clone(), v)),
                    "decorators" => data.decorators.push((path.clone(), v)),
                    "pdg" => data.pdgs.push((path.clone(), v)),
                    "code_snippet" => data.code_snippets.push((path.clone(), v)),
                    _ => {}
                }
            }
        }
        data
    }

    fn collect_for_paths<'a>(files: &HashMap<PathBuf, FileEntry>, paths: impl Iterator<Item = &'a PathBuf>) -> Self {
        let mut data = Self::new();
        for path in paths {
            if let Some(entry) = files.get(path) {
                for result in entry.get_results() {
                    let v = result.data.clone();
                    match result.subscriber_name.as_str() {
                        "scope_tree" => data.scope_trees.push((path.clone(), v)),
                        "definitions" => data.definitions.push((path.clone(), v)),
                        "uses" => data.uses.push((path.clone(), v)),
                        "raw_bindings" => data.raw_bindings.push((path.clone(), v)),
                        "imports" => data.imports.push((path.clone(), v)),
                        "decorators" => data.decorators.push((path.clone(), v)),
                        "pdg" => data.pdgs.push((path.clone(), v)),
                        "code_snippet" => data.code_snippets.push((path.clone(), v)),
                        _ => {}
                    }
                }
            }
        }
        data
    }
}

/// Run all load passes for the given per-file data against `builder`.
/// Sets `current_file` before each file's data and clears it after.
/// This is the "assert" path — adds to existing state rather than replacing.
fn assert_files(builder: &mut GraphBuilder, data: &PerFileData, files: &HashMap<PathBuf, FileEntry>) {
    // Pass 1: Build nodes
    for (path, d) in &data.scope_trees {
        builder.current_file = Some(path.clone());
        builder.load_scope_tree(d);
    }
    builder.current_file = None;

    for (path, d) in &data.definitions {
        builder.current_file = Some(path.clone());
        builder.load_definitions(d);
    }

    // Populate local_prefixes for these files
    for (path, _) in &data.scope_trees {
        let module_path = builder.file_to_module(&path.to_string_lossy());
        let top = module_path.split('.').next().unwrap_or(&module_path).to_string();
        if !top.is_empty() {
            builder.local_prefixes.insert(top);
        }
    }
    // Also ensure all files in the manager contribute their top-level prefix
    for path in files.keys() {
        let module_path = builder.file_to_module(&path.to_string_lossy());
        let top = module_path.split('.').next().unwrap_or(&module_path).to_string();
        if !top.is_empty() {
            builder.local_prefixes.insert(top);
        }
    }

    for (path, d) in &data.pdgs {
        builder.current_file = Some(path.clone());
        builder.load_pdgs(d);
    }
    builder.current_file = None;

    for (path, d) in &data.code_snippets {
        builder.current_file = Some(path.clone());
        builder.load_code_snippets(d);
    }
    builder.current_file = None;

    // Build re-export map (uses only reexport files, others are no-ops)
    let import_values: Vec<serde_json::Value> = data.imports.iter().map(|(_, v)| v.clone()).collect();
    for (path, d) in &data.imports {
        builder.current_file = Some(path.clone());
        let _ = d; // current_file is set; build_reexport_map iterates the slice
    }
    builder.current_file = None;
    builder.build_reexport_map(&import_values);

    for (path, d) in &data.imports {
        builder.current_file = Some(path.clone());
        builder.load_import_bindings(d);
    }
    builder.current_file = None;

    // Resolve is-a edges (needs import_bindings populated above)
    for (path, d) in &data.scope_trees {
        builder.current_file = Some(path.clone());
        builder.resolve_inheritance_edges(d);
    }
    builder.current_file = None;

    // Pass 2: Build edges
    for (path, d) in &data.uses {
        builder.current_file = Some(path.clone());
        builder.load_uses(d);
    }
    builder.current_file = None;

    for (path, d) in &data.decorators {
        builder.current_file = Some(path.clone());
        builder.load_decorators(d);
    }
    builder.current_file = None;

    // Store raw_bindings per file in contributions (for incremental re-merge)
    for (path, d) in &data.raw_bindings {
        if let serde_json::Value::Array(arr) = d {
            builder.file_contributions
                .entry(path.clone())
                .or_default()
                .raw_bindings = arr.clone();
        }
    }

    for (path, d) in &data.imports {
        builder.current_file = Some(path.clone());
        builder.load_imports(d);
    }
    builder.current_file = None;
}

/// Run the global load_raw_bindings pass using stored per-file raw_bindings.
fn run_global_raw_bindings(builder: &mut GraphBuilder) {
    let merged: Vec<serde_json::Value> = builder
        .file_contributions
        .values()
        .flat_map(|c| c.raw_bindings.iter().cloned())
        .collect();
    let merged_value = serde_json::Value::Array(merged);
    builder.load_raw_bindings(&merged_value);
}

#[pymethods]
impl FileManager {
    #[new]
    fn new() -> Self {
        FileManager {
            files: HashMap::new(),
            graph_builder: None,
        }
    }

    /// Open a file for analysis.
    fn open_file(&mut self, path: &str, source: &str) -> PyResult<()> {
        let pb = PathBuf::from(path);
        let lang = Lang::from_extension(&pb)
            .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("Unsupported language"))?;
        let mut entry = FileEntry::new(lang, source.to_string(), path.to_string());

        // Parse and cache results immediately
        entry
            .process_and_cache()
            .map_err(pyo3::exceptions::PyRuntimeError::new_err)?;

        self.files.insert(pb.clone(), entry);

        // Mark dirty so the next build_dependency_graph incrementally updates
        if let Some(ref mut builder) = self.graph_builder {
            builder.dirty_files.insert(pb);
        }

        Ok(())
    }

    /// Open multiple files in parallel using rayon.
    /// Accepts a list of (path, source) pairs and processes them concurrently.
    fn open_files_bulk(&mut self, files: Vec<(String, String)>) -> PyResult<()> {
        let results: Vec<(PathBuf, Result<FileEntry, String>)> = files
            .into_par_iter()
            .filter_map(|(path, source)| {
                let pb = PathBuf::from(&path);
                let lang = Lang::from_extension(&pb)?;
                let mut entry = FileEntry::new(lang, source, path);
                let result = entry.process_and_cache().map(|_| entry);
                Some((pb, result))
            })
            .collect();

        for (pb, result) in results {
            let entry = result.map_err(pyo3::exceptions::PyRuntimeError::new_err)?;
            self.files.insert(pb.clone(), entry);
            if let Some(ref mut builder) = self.graph_builder {
                builder.dirty_files.insert(pb);
            }
        }
        Ok(())
    }

    /// Update a file with new source content.
    fn update_file<'py>(
        &mut self,
        py: Python<'py>,
        path: &str,
        source: &str,
    ) -> PyResult<Bound<'py, PyList>> {
        let pb = PathBuf::from(path);
        let entry = self
            .files
            .get_mut(&pb)
            .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err("File not opened"))?;

        let changed = entry
            .update(source.to_string())
            .map_err(pyo3::exceptions::PyRuntimeError::new_err)?;

        let pylist = PyList::empty(py);
        if changed {
            // Mark dirty so the next build_dependency_graph incrementally updates
            if let Some(ref mut builder) = self.graph_builder {
                builder.dirty_files.insert(pb);
            }

            for result in entry.get_results() {
                let dict = PyDict::new(py);
                dict.set_item("subscriber", &result.subscriber_name)?;
                dict.set_item("data", result.data.to_string())?;
                pylist.append(dict)?;
            }
        }

        Ok(pylist)
    }

    /// Build a dependency graph from all tracked files' cached subscriber data.
    /// Returns the graph as a JSON string matching serpentine's GraphData format.
    ///
    /// On first call: full build. On subsequent calls with dirty files: incremental
    /// retract+assert for changed files, global raw_bindings re-run, then snapshot.
    fn build_dependency_graph(&mut self) -> PyResult<String> {
        if self.graph_builder.is_none() {
            // ----------------------------------------------------------------
            // Cold (first) build — initialize builder and run all passes
            // ----------------------------------------------------------------
            let mut builder = GraphBuilder::new();
            builder.lang_configs = vec![
                Box::new(PythonConfig::new()),
                Box::new(JsConfig::new()),
                Box::new(crate::rust_lang::config::RustConfig::new()),
                Box::new(TerraformConfig::new()),
            ];

            let data = PerFileData::collect_from(&self.files);
            assert_files(&mut builder, &data, &self.files);
            run_global_raw_bindings(&mut builder);
            builder.enrich_pdgs();

            let json = builder.snapshot().to_json();
            self.graph_builder = Some(builder);
            return Ok(json);
        }

        let builder = self.graph_builder.as_mut().unwrap();

        if builder.dirty_files.is_empty() {
            // Nothing changed — return snapshot of current state
            return Ok(builder.snapshot().to_json());
        }

        // ----------------------------------------------------------------
        // Incremental build — retract dirty files, re-assert, rebuild edges
        // ----------------------------------------------------------------
        let dirty_paths: Vec<PathBuf> = builder.dirty_files.drain().collect();

        // Retract all dirty files first so their stale contributions are gone
        for path in &dirty_paths {
            builder.retract_file(path);
        }

        // Re-assert dirty files (re-run load passes for changed files only)
        let dirty_data = PerFileData::collect_for_paths(&self.files, dirty_paths.iter());
        assert_files(builder, &dirty_data, &self.files);

        // Re-run the global raw_bindings pass now that per-file raw_bindings are updated
        builder.clear_raw_binding_edges();
        run_global_raw_bindings(builder);

        // Re-enrich PDGs globally (needed since LEGB resolution may have changed)
        builder.enrich_pdgs();

        Ok(builder.snapshot().to_json())
    }

    /// Get parsed results from all tracked files (deprecated).
    fn get_all_results<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let pylist = PyList::empty(py);
        // Return empty for now - this method is being phased out
        Ok(pylist)
    }

    fn close_file(&mut self, path: &str) -> PyResult<()> {
        let pb = PathBuf::from(path);
        self.files.remove(&pb);
        // Retract from builder if present
        if let Some(ref mut builder) = self.graph_builder {
            builder.retract_file(&pb);
            builder.dirty_files.remove(&pb);
        }
        Ok(())
    }

    /// Hydrate a FileEntry from cached subscriber results without re-running tree-sitter.
    /// Does not mark the file dirty — it is considered clean/unchanged.
    fn load_file_results(&mut self, path: &str, results_json: &str) -> PyResult<()> {
        let pb = PathBuf::from(path);
        let lang = Lang::from_extension(&pb)
            .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("Unsupported language"))?;
        let entry = FileEntry::from_results_json(lang, path.to_string(), results_json)
            .map_err(pyo3::exceptions::PyRuntimeError::new_err)?;
        self.files.insert(pb, entry);
        Ok(())
    }

    /// Serialize a file's cached subscriber results to JSON for per-file caching.
    fn get_file_results(&self, path: &str) -> PyResult<String> {
        let pb = PathBuf::from(path);
        let entry = self
            .files
            .get(&pb)
            .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err("File not opened"))?;
        let json: Vec<serde_json::Value> = entry
            .get_results()
            .iter()
            .map(|r| serde_json::json!({"subscriber": r.subscriber_name, "data": r.data}))
            .collect();
        serde_json::to_string(&json).map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }
}

#[pymodule]
fn _analyzer(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<FileManager>()?;
    Ok(())
}
