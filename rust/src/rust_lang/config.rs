//! Rust language configuration — implements `LanguageConfig` for Rust crates.

use crate::graph::LanguageConfig;

/// Rust standard library crate names.
const RUST_STDLIB: &[&str] = &["std", "core", "alloc", "proc_macro"];

/// Language configuration for Rust projects.
pub struct RustConfig;

impl RustConfig {
    pub fn new() -> Self {
        RustConfig
    }
}

impl Default for RustConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageConfig for RustConfig {
    fn derive_module_path(&self, file_path: &str, _project_root: &str) -> String {
        crate::rust_lang::derive_module_path(file_path).join(".")
    }

    fn is_reexport_file(&self, file_path: &str) -> bool {
        let stem = std::path::Path::new(file_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        matches!(stem, "lib" | "mod")
    }

    fn is_stdlib(&self, module: &str) -> bool {
        let top = module.split('.').next().unwrap_or(module);
        RUST_STDLIB.contains(&top)
    }

    fn is_third_party(&self, module: &str) -> bool {
        !self.is_stdlib(module)
    }

    fn extensions(&self) -> &[&str] {
        &[".rs"]
    }
}
