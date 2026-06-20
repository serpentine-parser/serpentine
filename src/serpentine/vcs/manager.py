"""VcsManager: loads and analyzes historical snapshots from a VCS backend."""

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
        archive = self._backend.get_archive_at(ref)
        graph_json = self._analyze_archive(archive)
        del archive  # release dict[str, bytes] immediately

        self._cache.put(commit_hash, config_fp, graph_json)
        return graph_json

    def _config_fingerprint(self) -> str:
        import hashlib
        import json

        data = json.dumps(self._config.to_dict(), sort_keys=True)
        return hashlib.sha256(data.encode()).hexdigest()[:16]

    def _analyze_archive(self, archive: dict[str, bytes]) -> str:
        from serpentine import _analyzer

        fm = _analyzer.FileManager()
        file_pairs: list[tuple[str, str]] = []
        for rel_path, content_bytes in archive.items():
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
