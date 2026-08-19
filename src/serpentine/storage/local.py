import re
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

_COMMIT_HASH_RE = re.compile(r"^[0-9a-f]{40}$")


class LocalGraphStore:
    """Filesystem-backed graph store. Layout: {root}/{repo_id}/{commit_hash}.json"""

    def __init__(self, root: Path) -> None:
        if not root.is_absolute():
            raise ValueError(f"SERPENTINE_LOCAL_STORE_PATH must be absolute, got: {root}")
        self._root = root

    def get(self, repo_id: str, commit_hash: str) -> str | None:
        self._validate_commit_hash(commit_hash)
        path = self._root / repo_id / f"{commit_hash}.json"
        if path.exists():
            return path.read_text(encoding="utf-8")
        return None

    def put(self, repo_id: str, commit_hash: str, graph_json: str) -> None:
        self._validate_commit_hash(commit_hash)
        path = self._root / repo_id / f"{commit_hash}.json"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(graph_json, encoding="utf-8")

    def list_ingested(self, repo_id: str) -> list[dict[str, Any]]:
        repo_dir = self._root / repo_id
        if not repo_dir.is_dir():
            return []
        result = []
        for p in repo_dir.glob("*.json"):
            if _COMMIT_HASH_RE.match(p.stem):
                ingested_at = datetime.fromtimestamp(p.stat().st_mtime, tz=timezone.utc).isoformat()
                result.append({"commit_hash": p.stem, "ingested_at": ingested_at})
        return result

    def _validate_commit_hash(self, commit_hash: str) -> None:
        if not _COMMIT_HASH_RE.match(commit_hash):
            raise ValueError(f"Invalid commit hash: {commit_hash!r}")
