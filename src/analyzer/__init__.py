"""
Analyzer - Rust-powered source code parsing.

This module wraps the Rust analyzer extension, providing the FileManager
class for parsing Python and JavaScript source files.

The Rust extension (analyzer.cpython-*.so) is built by maturin and
provides the actual implementation.
"""

# Re-export everything from the Rust extension
from serpentine._analyzer import FileManager  # type: ignore

__all__ = ["FileManager"]
