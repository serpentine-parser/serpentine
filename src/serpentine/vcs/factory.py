import os
from pathlib import Path

from serpentine.cache import NullCache
from serpentine.config import Config
from serpentine.storage.factory import ConfigError
from serpentine.vcs.manager import VcsManager


def build_vcs_manager(repo_id: str) -> VcsManager:
    """
    Build a VcsManager for repo_id using environment config.

    Phase 1: local repos only via SERPENTINE_REPOS_DIR.
    Phase 3 extends this to GitHubApiBackend for org/repo slugs.

    Config is NOT loaded here — it is fetched per-ref during ingest_ref().
    Always passes NullCache to VcsManager (GraphStore is the durable layer).
    """
    repos_dir = os.environ.get("SERPENTINE_REPOS_DIR")

    if repos_dir:
        repo_path = Path(repos_dir) / repo_id
        if repo_path.is_dir():
            from serpentine.vcs.backend import GitBackend
            backend = GitBackend(repo_path)
            return VcsManager(backend, NullCache(), Config.load(repo_path))

    raise ConfigError(
        f"Cannot build VcsManager for {repo_id!r}. "
        "Set SERPENTINE_REPOS_DIR to a directory of local git checkouts."
    )


def build_vcs_managers() -> dict[str, VcsManager]:
    """Build VcsManager dict for all known repos at startup."""
    repos_dir = os.environ.get("SERPENTINE_REPOS_DIR")
    if not repos_dir:
        return {}

    managers: dict[str, VcsManager] = {}
    base = Path(repos_dir)
    if not base.is_dir():
        raise ConfigError(f"SERPENTINE_REPOS_DIR does not exist: {repos_dir}")

    allowed_raw = os.environ.get("SERPENTINE_ALLOWED_REPOS")
    allowed: set[str] | None = (
        {r.strip() for r in allowed_raw.split(",") if r.strip()}
        if allowed_raw
        else None
    )

    for entry in base.iterdir():
        if not entry.is_dir():
            continue
        repo_id = entry.name
        if allowed is not None and repo_id not in allowed:
            continue
        try:
            managers[repo_id] = build_vcs_manager(repo_id)
        except Exception as e:
            import logging
            logging.getLogger(__name__).warning(f"Skipping {repo_id}: {e}")

    return managers
