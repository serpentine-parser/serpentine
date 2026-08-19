"""
Infrastructure adapters implementing domain protocols.
"""

from pathlib import Path
from typing import Any

from serpentine.services import SourceProvider  # noqa: F401 — re-export for convenience


class DiskSourceProvider:
    """Reads files from the local filesystem."""
    def get_file(self, path: str) -> str | None:
        try:
            return Path(path).read_text(encoding="utf-8")
        except OSError:
            return None


class VcsSourceProvider:
    """Reads files from a VCS backend at a specific ref."""
    def __init__(self, backend: Any, ref: str) -> None:
        self._backend = backend
        self._ref = ref

    def get_file(self, path: str) -> str | None:
        content = self._backend.get_file_at(self._ref, path)
        return content.decode("utf-8", errors="replace") if content else None
