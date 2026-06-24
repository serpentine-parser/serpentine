"""VcsManager: loads and analyzes historical snapshots from a VCS backend."""

import fnmatch
import logging
from pathlib import Path
from typing import Any

from serpentine.vcs.backend import VcsBackend, VcsRef

logger = logging.getLogger(__name__)


class VcsManager:
    """Coordinates VCS ref listing and snapshot analysis."""

    def __init__(self, backend: VcsBackend, cache: Any, config: Any) -> None:
        self._backend = backend
        self._cache = cache
        self._config = config

    def list_refs(self) -> list[VcsRef]:
        return self._backend.list_refs()

    def get_graph_at(self, ref: str) -> str:
        """Return graph JSON for the project at the given VCS ref. Cache-aware."""
        commit_hash = self._backend.resolve_to_commit_hash(ref)
        config_fp = self._config_fingerprint()

        cached = self._cache.get(commit_hash, config_fp)
        if cached is not None:
            logger.info(f"[vcs] cache hit for {ref} ({commit_hash[:7]})")
            return cached

        logger.info(f"[vcs] analyzing snapshot at {ref} ({commit_hash[:7]})")
        archive = self._backend.get_archive_at(ref, set(self._config.extensions))
        graph_json = self._analyze_archive(archive)
        del archive  # release dict[str, bytes] immediately

        self._cache.put(commit_hash, config_fp, graph_json)
        return graph_json

    def _config_fingerprint(self) -> str:
        import hashlib
        import json

        data = json.dumps(self._config.to_dict(), sort_keys=True)
        return hashlib.sha256(data.encode()).hexdigest()[:16]

    def _is_excluded(self, rel_path: str) -> bool:
        """Return True if rel_path should be excluded per the active config."""
        p = Path(rel_path)
        # Extension filter
        if p.suffix not in self._config.extensions:
            return True
        # Directory filter — any path component that is in exclude_dirs
        exclude_dirs = self._config.exclude_dirs
        if any(part in exclude_dirs for part in p.parts[:-1]):
            return True
        # Glob pattern filter
        for pattern in self._config.exclude_patterns:
            if fnmatch.fnmatch(rel_path, pattern) or fnmatch.fnmatch(p.name, pattern):
                return True
        return False

    def _analyze_archive(self, archive: dict[str, bytes]) -> str:
        from serpentine import _analyzer

        fm = _analyzer.FileManager()
        file_pairs: list[tuple[str, str]] = []
        for rel_path, content_bytes in archive.items():
            if self._is_excluded(rel_path):
                logger.debug(f"[vcs] excluded by config: {rel_path}")
                continue
            try:
                content = content_bytes.decode("utf-8", errors="replace")
                file_pairs.append((rel_path, content))
            except Exception as e:
                logger.debug(f"[vcs] skipping {rel_path}: {e}")

        if file_pairs:
            try:
                fm.open_files_bulk(file_pairs)
            except Exception as e:
                logger.warning(f"[vcs] bulk open failed, falling back to serial: {e}")
                for path, source in file_pairs:
                    try:
                        fm.open_file(path, source)
                    except Exception as e2:
                        logger.debug(f"[vcs] skipping {path}: {e2}")

        return fm.build_dependency_graph()
