//! Python language configuration — implements `LanguageConfig` for Python projects.

use std::collections::HashSet;
use std::sync::OnceLock;

use crate::graph::LanguageConfig;

/// Python standard library module names.
///
/// Used by `PythonConfig::is_stdlib()` to classify module origins.
/// Moved here from `graph/mod.rs` so that Python-specific knowledge lives
/// in the `python/` directory where it belongs.
pub const STDLIB_MODULES: &[&str] = &[
    "abc",
    "aifc",
    "argparse",
    "array",
    "ast",
    "asynchat",
    "asyncio",
    "asyncore",
    "atexit",
    "audioop",
    "base64",
    "bdb",
    "binascii",
    "binhex",
    "bisect",
    "builtins",
    "bz2",
    "calendar",
    "cgi",
    "cgitb",
    "chunk",
    "cmath",
    "cmd",
    "code",
    "codecs",
    "codeop",
    "collections",
    "colorsys",
    "compileall",
    "concurrent",
    "configparser",
    "contextlib",
    "contextvars",
    "copy",
    "copyreg",
    "cProfile",
    "crypt",
    "csv",
    "ctypes",
    "curses",
    "dataclasses",
    "datetime",
    "dbm",
    "decimal",
    "difflib",
    "dis",
    "distutils",
    "doctest",
    "email",
    "encodings",
    "enum",
    "errno",
    "faulthandler",
    "fcntl",
    "filecmp",
    "fileinput",
    "fnmatch",
    "fractions",
    "ftplib",
    "functools",
    "gc",
    "getopt",
    "getpass",
    "gettext",
    "glob",
    "grp",
    "gzip",
    "hashlib",
    "heapq",
    "hmac",
    "html",
    "http",
    "imaplib",
    "imghdr",
    "imp",
    "importlib",
    "inspect",
    "io",
    "ipaddress",
    "itertools",
    "json",
    "keyword",
    "lib2to3",
    "linecache",
    "locale",
    "logging",
    "lzma",
    "mailbox",
    "mailcap",
    "marshal",
    "math",
    "mimetypes",
    "mmap",
    "modulefinder",
    "multiprocessing",
    "netrc",
    "nis",
    "nntplib",
    "numbers",
    "operator",
    "optparse",
    "os",
    "ossaudiodev",
    "pathlib",
    "pdb",
    "pickle",
    "pickletools",
    "pipes",
    "pkgutil",
    "platform",
    "plistlib",
    "poplib",
    "posix",
    "posixpath",
    "pprint",
    "profile",
    "pstats",
    "pty",
    "pwd",
    "py_compile",
    "pyclbr",
    "pydoc",
    "queue",
    "quopri",
    "random",
    "re",
    "readline",
    "reprlib",
    "resource",
    "rlcompleter",
    "runpy",
    "sched",
    "secrets",
    "select",
    "selectors",
    "shelve",
    "shlex",
    "shutil",
    "signal",
    "site",
    "smtpd",
    "smtplib",
    "sndhdr",
    "socket",
    "socketserver",
    "spwd",
    "sqlite3",
    "ssl",
    "stat",
    "statistics",
    "string",
    "stringprep",
    "struct",
    "subprocess",
    "sunau",
    "symtable",
    "sys",
    "sysconfig",
    "syslog",
    "tabnanny",
    "tarfile",
    "telnetlib",
    "tempfile",
    "termios",
    "test",
    "textwrap",
    "threading",
    "time",
    "timeit",
    "tkinter",
    "token",
    "tokenize",
    "trace",
    "traceback",
    "tracemalloc",
    "tty",
    "turtle",
    "turtledemo",
    "types",
    "typing",
    "unicodedata",
    "unittest",
    "urllib",
    "uu",
    "uuid",
    "venv",
    "warnings",
    "wave",
    "weakref",
    "webbrowser",
    "winreg",
    "winsound",
    "wsgiref",
    "xdrlib",
    "xml",
    "xmlrpc",
    "zipapp",
    "zipfile",
    "zipimport",
    "zlib",
    "print", // Built-in function
];

/// Language configuration for Python projects.
pub struct PythonConfig;

impl PythonConfig {
    pub fn new() -> Self {
        PythonConfig
    }
}

impl Default for PythonConfig {
    fn default() -> Self {
        Self::new()
    }
}

static STDLIB_SET: OnceLock<HashSet<&'static str>> = OnceLock::new();

fn stdlib_set() -> &'static HashSet<&'static str> {
    STDLIB_SET.get_or_init(|| STDLIB_MODULES.iter().copied().collect())
}

impl LanguageConfig for PythonConfig {
    /// Derive the logical module qualname from a Python file path.
    ///
    /// Delegates to `crate::python::derive_module_path` which handles
    /// `__init__.py` → package directory and walks up to the project root.
    fn derive_module_path(&self, file_path: &str, _project_root: &str) -> String {
        crate::python::derive_module_path(file_path).join(".")
    }

    /// Returns `true` for `__init__.py` files, which re-export the package
    /// namespace in Python projects.
    fn is_reexport_file(&self, file_path: &str) -> bool {
        file_path.ends_with("__init__.py")
    }

    /// Returns `true` if `module` is a Python standard library module.
    fn is_stdlib(&self, module: &str) -> bool {
        let top_level = module.split('.').next().unwrap_or(module);
        stdlib_set().contains(top_level)
    }

    /// Returns `true` if `module` is not a Python stdlib module.
    ///
    /// Note: distinguishing third-party from local requires access to the
    /// graph's known definitions, which is handled by `GraphBuilder::classify_module`.
    /// This method is a coarse filter only.
    fn is_third_party(&self, module: &str) -> bool {
        !self.is_stdlib(module)
    }

    fn extensions(&self) -> &[&str] {
        &[".py"]
    }
}
