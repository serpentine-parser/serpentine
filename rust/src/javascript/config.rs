//! JavaScript/TypeScript language configuration — implements `LanguageConfig`
//! for the JS/TS/JSX/TSX family of grammars.

use std::collections::HashSet;
use std::sync::OnceLock;

use crate::graph::LanguageConfig;

/// Node.js built-in module names (as of Node 20).
const NODE_BUILTINS: &[&str] = &[
    "assert",
    "async_hooks",
    "buffer",
    "child_process",
    "cluster",
    "console",
    "constants",
    "crypto",
    "dgram",
    "diagnostics_channel",
    "dns",
    "domain",
    "events",
    "fs",
    "http",
    "http2",
    "https",
    "inspector",
    "module",
    "net",
    "os",
    "path",
    "perf_hooks",
    "process",
    "punycode",
    "querystring",
    "readline",
    "repl",
    "stream",
    "string_decoder",
    "sys",
    "timers",
    "tls",
    "trace_events",
    "tty",
    "url",
    "util",
    "v8",
    "vm",
    "wasi",
    "worker_threads",
    "zlib",
];

/// Language configuration for JavaScript/TypeScript/JSX/TSX projects.
pub struct JsConfig;

impl JsConfig {
    pub fn new() -> Self {
        JsConfig
    }
}

impl Default for JsConfig {
    fn default() -> Self {
        Self::new()
    }
}

static NODE_BUILTINS_SET: OnceLock<HashSet<&'static str>> = OnceLock::new();

fn node_builtins_set() -> &'static HashSet<&'static str> {
    NODE_BUILTINS_SET.get_or_init(|| NODE_BUILTINS.iter().copied().collect())
}

impl LanguageConfig for JsConfig {
    /// Derive the logical module qualname from a JS/TS file path.
    ///
    /// Delegates to `crate::javascript::derive_module_path`.
    fn derive_module_path(&self, file_path: &str, _project_root: &str) -> String {
        crate::javascript::derive_module_path(file_path).join(".")
    }

    /// Returns `true` for index files, which act as re-export hubs in Node projects.
    fn is_reexport_file(&self, file_path: &str) -> bool {
        let basename = std::path::Path::new(file_path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        matches!(
            basename,
            "index.js" | "index.jsx" | "index.ts" | "index.tsx" | "index.mjs" | "index.cjs"
        )
    }

    /// Returns `true` if `module` is a Node.js built-in.
    ///
    /// Handles bare names (`fs`) and the `node:` prefix (`node:fs`).
    fn is_stdlib(&self, module: &str) -> bool {
        let name = module.strip_prefix("node:").unwrap_or(module);
        let top = name.split('/').next().unwrap_or(name);
        node_builtins_set().contains(top)
    }

    /// Returns `true` if `module` looks like an npm package (third-party).
    ///
    /// npm packages are non-relative (don't start with `.` or `/`).
    /// Note: local file detection requires the graph's definition map and is
    /// handled by `GraphBuilder::classify_module`.
    fn is_third_party(&self, module: &str) -> bool {
        !module.starts_with('.') && !module.starts_with('/')
    }

    fn extensions(&self) -> &[&str] {
        &[".js", ".jsx", ".ts", ".tsx", ".mjs", ".cjs"]
    }
}
