# This module is provided by the Rust extension built with maturin.
# It exports FileManager for parsing source code.

class FileManager:
    """Manages source files and builds dependency graphs."""

    def __init__(self) -> None: ...
    def open_file(self, path: str, source: str) -> None:
        """Open a source file for analysis."""
        ...

    def build_dependency_graph(self) -> str:
        """Build the dependency graph and return as JSON string."""
        ...
