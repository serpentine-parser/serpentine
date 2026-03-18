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

use crate::javascript::{parse as parse_javascript, JsLang};
use crate::javascript::config::JsConfig;
use crate::message_bus::{MessageBus, SubscriberResult};
use crate::graph::GraphBuilder;
use crate::python::parse as parse_python;
use crate::python::config::PythonConfig;
use crate::rust_lang::parse as parse_rust;
use crate::subscribers::{
    PdgSubscriberFactory, CodeSnippetSubscriberFactory, DefinitionsSubscriberFactory,
    EventCounterSubscriberFactory, ImportsSubscriberFactory, RawBindingsSubscriberFactory,
    ScopeTreeSubscriberFactory, UsesSubscriberFactory,
};

use rayon::prelude::*;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

use tree_sitter::{Language, Parser, Tree};

use tree_sitter_javascript::language as javascript_language;
use tree_sitter_python::language as python_language;
use tree_sitter_rust::language as rust_language;
use tree_sitter_typescript::{language_tsx, language_typescript};

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
            _ => None,
        }
    }

    /// Get the tree-sitter language for parsing.
    fn language(&self) -> Language {
        match self {
            Lang::Python => python_language(),
            Lang::JavaScript => javascript_language(),
            Lang::TypeScript => language_typescript(),
            Lang::Tsx => language_tsx(),
            Lang::Rust => rust_language(),
        }
    }

    /// Map to the JsLang variant used by the JS/TS walker.
    fn js_lang(&self) -> Option<JsLang> {
        match self {
            Lang::JavaScript => Some(JsLang::JavaScript),
            Lang::TypeScript => Some(JsLang::TypeScript),
            Lang::Tsx => Some(JsLang::Tsx),
            Lang::Python | Lang::Rust => None,
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
        parser.set_language(lang.language()).unwrap();
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
}

#[pymethods]
impl FileManager {
    #[new]
    fn new() -> Self {
        FileManager {
            files: HashMap::new(),
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

        self.files.insert(pb, entry);
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
            self.files.insert(pb, result.map_err(pyo3::exceptions::PyRuntimeError::new_err)?);
        }
        Ok(())
    }

    /// Update a file with new source content.
    fn update_file<'py>(
        &mut self,
        py: Python<'py>,
        path: &str,
        source: &str,
    ) -> PyResult<&'py PyList> {
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
    /// The graph is built in three passes:
    /// 1. Build nodes: scope_tree + definitions + attach raw PDGs
    /// 2. Build edges: uses + raw_bindings + imports (all reference definitions)
    /// 3. Enrich: cross-scope data-flow, parameter bindings, flow-graph expansion
    fn build_dependency_graph(&mut self) -> PyResult<String> {
        let mut builder = GraphBuilder::new();
        builder.lang_configs = vec![
            Box::new(PythonConfig::new()),
            Box::new(JsConfig::new()),
            Box::new(crate::rust_lang::config::RustConfig::new()),
        ];

        // Collect all subscriber data from all files (already cached from parsing)
        let mut all_scope_trees: Vec<serde_json::Value> = Vec::new();
        let mut all_definitions: Vec<serde_json::Value> = Vec::new();
        let mut all_uses: Vec<serde_json::Value> = Vec::new();
        let mut all_raw_bindings: Vec<serde_json::Value> = Vec::new();
        let mut all_imports: Vec<serde_json::Value> = Vec::new();
        let mut all_pdgs: Vec<serde_json::Value> = Vec::new();
        let mut all_code_snippets: Vec<serde_json::Value> = Vec::new();

        for entry in self.files.values() {
            for result in entry.get_results() {
                let data = result.data.clone();
                match result.subscriber_name.as_str() {
                    "scope_tree" => all_scope_trees.push(data),
                    "definitions" => all_definitions.push(data),
                    "uses" => all_uses.push(data),
                    "raw_bindings" => all_raw_bindings.push(data),
                    "imports" => all_imports.push(data),
                    "pdg" => all_pdgs.push(data),
                    "code_snippet" => all_code_snippets.push(data),
                    _ => {}
                }
            }
        }

        // Pass 1: Build nodes — scope tree, definitions, and attach raw PDGs
        for data in all_scope_trees {
            builder.load_scope_tree(&data);
        }
        for data in all_definitions {
            builder.load_definitions(&data);
        }
        for data in all_pdgs {
            builder.load_pdgs(&data);
        }
        for data in all_code_snippets {
            builder.load_code_snippets(&data);
        }

        // Build re-export map from __init__.py imports before creating edges.
        builder.build_reexport_map(&all_imports);
        for data in &all_imports {
            builder.load_import_bindings(data);
        }

        // Pass 2: Build edges — uses, bindings, and imports (all reference definitions)
        for data in all_uses {
            builder.load_uses(&data);
        }

        // Merge all raw bindings into one array — the two-pass ASSIGNED→CALLS
        // logic in load_raw_bindings requires all bindings to be present at once.
        let merged_bindings: Vec<serde_json::Value> = all_raw_bindings
            .iter()
            .filter_map(|data| data.as_array())
            .flatten()
            .cloned()
            .collect();
        let merged_bindings_value = serde_json::Value::Array(merged_bindings);

        builder.load_raw_bindings(&merged_bindings_value);
        for data in all_imports {
            builder.load_imports(&data);
        }

        // Pass 3: Enrich PDGs — resolve callee_text → references on call nodes
        builder.enrich_pdgs();

        // Build and serialize the graph (includes deduplicate_edges)
        let graph = builder.build();
        Ok(graph.to_json())
    }

    /// Get parsed results from all tracked files (deprecated).
    fn get_all_results<'py>(&self, py: Python<'py>) -> PyResult<&'py PyList> {
        let pylist = PyList::empty(py);
        // Return empty for now - this method is being phased out
        Ok(pylist)
    }

    fn close_file(&mut self, path: &str) -> PyResult<()> {
        self.files.remove(&PathBuf::from(path));
        Ok(())
    }
}

#[pymodule]
fn _analyzer(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<FileManager>()?;
    Ok(())
}
