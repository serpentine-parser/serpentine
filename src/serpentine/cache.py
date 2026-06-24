"""
Disk cache for analysis results.

Caches the final graph JSON keyed by a fingerprint of all source file
mtimes and the config file mtime. Cache hits skip the full Rust analysis.

Also provides per-file subscriber result caching so subsequent cold starts
skip tree-sitter re-parsing for unchanged files.
"""

import hashlib
import logging
import shutil
from pathlib import Path

logger = logging.getLogger(__name__)

CACHE_DIR = ".serpentine"
FINGERPRINT_FILE = "fingerprint"
GRAPH_FILE = "graph.json"

# Bump this constant whenever the cache schema changes in a breaking way.
CACHE_VERSION = 3

FILES_CACHE_SUBDIR = "files"
# Bump when subscriber output format changes in a breaking way.
PER_FILE_CACHE_VERSION = 1


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
        self._cache_dir = project_path / CACHE_DIR

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
            fp_path = self._cache_dir / FINGERPRINT_FILE
            graph_path = self._cache_dir / GRAPH_FILE
            if not fp_path.exists() or not graph_path.exists():
                return None
            if fp_path.read_text(encoding="utf-8").strip() == fingerprint:
                logger.info("[cache] hit — skipping analysis")
                return graph_path.read_text(encoding="utf-8")
        except Exception as e:
            logger.debug(f"Cache load failed: {e}")
        return None

    def save(self, fingerprint: str, graph_json: str) -> None:
        """Write fingerprint and graph JSON to separate files."""
        try:
            self._cache_dir.mkdir(parents=True, exist_ok=True)
            (self._cache_dir / FINGERPRINT_FILE).write_text(fingerprint + "\n", encoding="utf-8")
            (self._cache_dir / GRAPH_FILE).write_text(graph_json, encoding="utf-8")
            logger.info("[cache] saved")
        except Exception as e:
            logger.warning(f"Cache save failed: {e}")


class PerFileCacheManager:
    """
    Per-file subscriber result cache stored under .serpentine/files/.

    Each entry caches the tree-sitter subscriber output for one source file,
    keyed by the file's absolute path hash and mtime_ns. On a cold start,
    cache hits let load_file_results() restore parsed state without re-running
    tree-sitter.

    The entire cache is invalidated when the analyzer binary changes (checked
    via a .version sentinel file), ensuring stale output never reaches the builder.
    """

    def __init__(self, project_path: Path) -> None:
        self._cache_dir = project_path / CACHE_DIR / FILES_CACHE_SUBDIR
        self._ensure_version()

    def _version_tag(self) -> str:
        package_dir = Path(__file__).parent
        binary = _find_analyzer_binary(package_dir)
        binary_mtime: int = 0
        if binary:
            try:
                binary_mtime = binary.stat().st_mtime_ns
            except OSError:
                pass
        return f"{PER_FILE_CACHE_VERSION}:{binary_mtime}"

    def _ensure_version(self) -> None:
        """Clear cache dir if the version tag has changed."""
        version_file = self._cache_dir / ".version"
        expected = self._version_tag()
        try:
            if version_file.exists() and version_file.read_text(encoding="utf-8").strip() == expected:
                return
        except Exception:
            pass
        # Version mismatch or unreadable — wipe and re-create.
        try:
            if self._cache_dir.exists():
                shutil.rmtree(self._cache_dir)
        except Exception as e:
            logger.debug(f"Per-file cache clear failed: {e}")
        try:
            self._cache_dir.mkdir(parents=True, exist_ok=True)
            version_file.write_text(expected + "\n", encoding="utf-8")
        except Exception as e:
            logger.debug(f"Per-file cache version write failed: {e}")

    def _entry_path(self, file_path: Path, mtime_ns: int) -> Path:
        path_hash = hashlib.sha256(str(file_path.resolve()).encode()).hexdigest()[:16]
        return self._cache_dir / f"{path_hash}_{mtime_ns}.json"

    def get(self, file_path: Path) -> str | None:
        """Return cached subscriber results JSON if mtime matches, else None."""
        try:
            mtime_ns = file_path.stat().st_mtime_ns
            entry = self._entry_path(file_path, mtime_ns)
            if entry.exists():
                return entry.read_text(encoding="utf-8")
        except Exception as e:
            logger.debug(f"Per-file cache load failed for {file_path}: {e}")
        return None

    def put(self, file_path: Path, results_json: str) -> None:
        """Write subscriber results JSON to cache keyed by current mtime."""
        try:
            mtime_ns = file_path.stat().st_mtime_ns
            self._cache_dir.mkdir(parents=True, exist_ok=True)
            entry = self._entry_path(file_path, mtime_ns)
            entry.write_text(results_json, encoding="utf-8")
        except Exception as e:
            logger.debug(f"Per-file cache save failed for {file_path}: {e}")


REFS_CACHE_SUBDIR = "refs"
# Bump when VCS ref cache format changes in a breaking way.
VCS_REF_CACHE_VERSION = 1


class VcsRefCacheManager:
    """
    Cache for graphs computed from historical VCS commits.

    Stored under .serpentine/refs/<key>.json, keyed by
    sha256(commit_hash + ":" + config_fingerprint)[:24].

    Historical commits are immutable so entries never expire — only
    invalidated when the analyzer binary changes (same sentinel pattern
    as PerFileCacheManager).
    """

    def __init__(self, project_path: Path) -> None:
        self._cache_dir = project_path / CACHE_DIR / REFS_CACHE_SUBDIR
        self._ensure_version()

    def _version_tag(self) -> str:
        package_dir = Path(__file__).parent
        binary = _find_analyzer_binary(package_dir)
        binary_mtime: int = 0
        if binary:
            try:
                binary_mtime = binary.stat().st_mtime_ns
            except OSError:
                pass
        return f"{VCS_REF_CACHE_VERSION}:{binary_mtime}"

    def _ensure_version(self) -> None:
        version_file = self._cache_dir / ".version"
        expected = self._version_tag()
        try:
            if version_file.exists() and version_file.read_text(encoding="utf-8").strip() == expected:
                return
        except Exception:
            pass
        try:
            if self._cache_dir.exists():
                shutil.rmtree(self._cache_dir)
        except Exception as e:
            logger.debug(f"VCS ref cache clear failed: {e}")
        try:
            self._cache_dir.mkdir(parents=True, exist_ok=True)
            version_file.write_text(expected + "\n", encoding="utf-8")
        except Exception as e:
            logger.debug(f"VCS ref cache version write failed: {e}")

    def _entry_path(self, commit_hash: str, config_fp: str) -> Path:
        key = hashlib.sha256(f"{commit_hash}:{config_fp}".encode()).hexdigest()[:24]
        return self._cache_dir / f"{key}.json"

    def get(self, commit_hash: str, config_fp: str) -> str | None:
        try:
            entry = self._entry_path(commit_hash, config_fp)
            if entry.exists():
                return entry.read_text(encoding="utf-8")
        except Exception as e:
            logger.debug(f"VCS ref cache load failed: {e}")
        return None

    def put(self, commit_hash: str, config_fp: str, graph_json: str) -> None:
        try:
            self._cache_dir.mkdir(parents=True, exist_ok=True)
            entry = self._entry_path(commit_hash, config_fp)
            entry.write_text(graph_json, encoding="utf-8")
        except Exception as e:
            logger.debug(f"VCS ref cache save failed: {e}")
