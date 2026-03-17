"""
Disk cache for analysis results.

Caches the final graph JSON keyed by a fingerprint of all source file
mtimes and the config file mtime. Cache hits skip the full Rust analysis.
"""

import hashlib
import json
import logging
from pathlib import Path

logger = logging.getLogger(__name__)

CACHE_DIR = ".serpentine"
CACHE_FILE = "graph_cache.json"

# Bump this constant whenever the cache schema changes in a breaking way.
CACHE_VERSION = 2


def _find_analyzer_binary(package_dir: Path) -> Path | None:
    """Return the path to the compiled _analyzer extension, or None if not found."""
    for pattern in ("_analyzer*.so", "_analyzer*.pyd"):
        matches = list(package_dir.glob(pattern))
        if matches:
            return matches[0]
    return None


class CacheManager:
    """
    Manages a single on-disk graph cache for a project.

    The cache stores the graph JSON alongside a fingerprint computed from
    the mtime_ns of every source file and the config file. Any file change
    produces a new fingerprint and forces a full re-analysis.
    """

    def __init__(self, project_path: Path, config_path: Path | None = None) -> None:
        self._project_path = project_path
        self._config_path = config_path
        self._cache_path = project_path / CACHE_DIR / CACHE_FILE

    def compute_fingerprint(self, source_files: list[Path]) -> str:
        """Return a SHA-256 hex digest over sorted (relative_path, mtime_ns) pairs."""
        h = hashlib.sha256()
        for file_path in sorted(source_files):
            try:
                rel = str(file_path.relative_to(self._project_path))
                mtime = file_path.stat().st_mtime_ns
                h.update(f"{rel}:{mtime}\n".encode())
            except (OSError, ValueError):
                pass

        if self._config_path and self._config_path.exists():
            try:
                mtime = self._config_path.stat().st_mtime_ns
                h.update(f"config:{mtime}\n".encode())
            except OSError:
                pass

        # Include schema version so breaking changes always invalidate the cache.
        h.update(f"schema:{CACHE_VERSION}\n".encode())

        # Include analyzer binary mtime so every `maturin develop` invalidates the cache.
        package_dir = Path(__file__).parent
        binary = _find_analyzer_binary(package_dir)
        if binary:
            try:
                h.update(f"binary:{binary.stat().st_mtime_ns}\n".encode())
            except OSError:
                pass

        return h.hexdigest()

    def load(self, fingerprint: str) -> str | None:
        """Return cached graph_json if fingerprint matches, else None."""
        try:
            if not self._cache_path.exists():
                return None
            data = json.loads(self._cache_path.read_text(encoding="utf-8"))
            if data.get("fingerprint") == fingerprint:
                logger.info("[cache] hit — skipping analysis")
                return data.get("graph_json")
        except Exception as e:
            logger.debug(f"Cache load failed: {e}")
        return None

    def save(self, fingerprint: str, graph_json: str) -> None:
        """Write fingerprint + graph_json to disk."""
        try:
            self._cache_path.parent.mkdir(parents=True, exist_ok=True)
            self._cache_path.write_text(
                json.dumps({"fingerprint": fingerprint, "graph_json": graph_json}),
                encoding="utf-8",
            )
            logger.info("[cache] saved")
        except Exception as e:
            logger.warning(f"Cache save failed: {e}")
