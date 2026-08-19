import os
from pathlib import Path

from serpentine.cache import NullCache
from serpentine.config import Config, DEFAULT_CONFIG
from serpentine.storage.factory import ConfigError
from serpentine.vcs.manager import VcsManager


def _is_github_slug(repo_id: str) -> bool:
    """Return True if repo_id looks like an org/repo GitHub slug."""
    parts = repo_id.split("/")
    return len(parts) == 2 and all(parts)


def build_vcs_manager(repo_id: str) -> VcsManager:
    """
    Build a VcsManager for repo_id using environment config.

    - If SERPENTINE_REPOS_DIR is set and repo_id is a known subdir: GitBackend
    - If repo_id looks like org/repo and SERPENTINE_GITHUB_TOKEN is set: GitHubApiBackend
    - Otherwise: raises ConfigError

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

    if _is_github_slug(repo_id):
        token = os.environ.get("SERPENTINE_GITHUB_TOKEN")
        if not token:
            raise ConfigError(
                f"repo_id {repo_id!r} looks like a GitHub slug but SERPENTINE_GITHUB_TOKEN is not set."
            )
        from serpentine.vcs.github import GitHubApiBackend
        backend = GitHubApiBackend(repo_id, token)
        return VcsManager(backend, NullCache(), Config(DEFAULT_CONFIG.copy()))

    raise ConfigError(
        f"Cannot build VcsManager for {repo_id!r}. "
        "Set SERPENTINE_REPOS_DIR for local repos or SERPENTINE_GITHUB_TOKEN for GitHub repos."
    )


def build_vcs_managers() -> dict[str, VcsManager]:
    """Build VcsManager dict for all known repos at startup.

    Sources (in priority order):
    1. SERPENTINE_REPOS_DIR — local git checkouts (one subdir per repo)
    2. SERPENTINE_ALLOWED_REPOS — explicit org/repo slugs for GitHub backend
    """
    import logging as _logging
    _log = _logging.getLogger(__name__)

    managers: dict[str, VcsManager] = {}

    # Local repos
    repos_dir = os.environ.get("SERPENTINE_REPOS_DIR")
    if repos_dir:
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
                _log.warning(f"Skipping local repo {repo_id}: {e}")

    # GitHub repos via SERPENTINE_ALLOWED_REPOS (org/repo slugs not found locally)
    allowed_raw = os.environ.get("SERPENTINE_ALLOWED_REPOS")
    if allowed_raw:
        for slug in (s.strip() for s in allowed_raw.split(",") if s.strip()):
            if slug in managers:
                continue
            if _is_github_slug(slug):
                try:
                    managers[slug] = build_vcs_manager(slug)
                except Exception as e:
                    _log.warning(f"Skipping GitHub repo {slug}: {e}")

    return managers
