use crate::graph::LanguageConfig;
use std::path::Path;

pub struct TerraformConfig;

impl TerraformConfig {
    pub fn new() -> Self {
        TerraformConfig
    }
}

impl Default for TerraformConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageConfig for TerraformConfig {
    fn derive_module_path(&self, file_path: &str, _project_root: &str) -> String {
        // Use file stem only — Terraform files don't have a package hierarchy
        // akin to Python's __init__.py or Rust's mod.rs.
        Path::new(file_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("main")
            .to_string()
    }

    fn is_reexport_file(&self, _file_path: &str) -> bool {
        false
    }

    fn is_stdlib(&self, _module: &str) -> bool {
        // Terraform has no stdlib concept
        false
    }

    fn is_third_party(&self, _module: &str) -> bool {
        // All external sources are third-party; local ones are identified via
        // local_prefixes in GraphBuilder::classify_module.
        true
    }

    fn extensions(&self) -> &[&str] {
        &[".tf"]
    }
}
