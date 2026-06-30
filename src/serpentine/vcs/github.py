"""GitHub REST API backend for VcsBackend protocol."""

import base64
import io
import logging
import tarfile
from typing import Any

import httpx

from serpentine.vcs.backend import VcsRef

logger = logging.getLogger(__name__)

_GITHUB_API = "https://api.github.com"
_MAX_COMMITS = 100


class GitHubApiBackend:
    """VcsBackend implementation using the GitHub REST API via httpx.

    All file contents stay in memory. Nothing is written to disk.
    """

    def __init__(
        self,
        repo_slug: str,
        token: str,
        *,
        transport: httpx.BaseTransport | None = None,
    ) -> None:
        self._slug = repo_slug  # "org/repo"
        self._client = httpx.Client(
            base_url=_GITHUB_API,
            headers={
                "Authorization": f"Bearer {token}",
                "Accept": "application/vnd.github+json",
                "X-GitHub-Api-Version": "2022-11-28",
            },
            timeout=30.0,
            transport=transport,
        )

    def _get(self, path: str, **params: Any) -> Any:
        resp = self._client.get(path, params=params or None)
        resp.raise_for_status()
        return resp.json()

    def list_refs(self) -> list[VcsRef]:
        refs: list[VcsRef] = []

        # Branches
        page = 1
        while True:
            items = self._get(f"/repos/{self._slug}/branches", per_page=100, page=page)
            if not items:
                break
            for b in items:
                sha = (b.get("commit") or {}).get("sha")
                refs.append(VcsRef(id=b["name"], display=b["name"], kind="branch", commit_hash=sha))
            if len(items) < 100:
                break
            page += 1

        # Tags
        page = 1
        while True:
            items = self._get(f"/repos/{self._slug}/tags", per_page=100, page=page)
            if not items:
                break
            for t in items:
                sha = (t.get("commit") or {}).get("sha")
                refs.append(VcsRef(id=t["name"], display=t["name"], kind="tag", commit_hash=sha))
            if len(items) < 100:
                break
            page += 1

        # Recent commits from default branch
        try:
            repo_info = self._get(f"/repos/{self._slug}")
            default_branch = repo_info.get("default_branch", "main")
            commits = self._get(
                f"/repos/{self._slug}/commits",
                sha=default_branch,
                per_page=min(_MAX_COMMITS, 100),
            )
            for c in commits[:_MAX_COMMITS]:
                sha = c["sha"]
                message = (c.get("commit", {}).get("message", "") or "").split("\n")[0][:60]
                refs.append(VcsRef(id=sha, display=f"{sha[:7]} {message}", kind="commit", commit_hash=sha))
        except httpx.HTTPError as e:
            logger.warning(f"[github] could not fetch commits: {e}")

        return refs

    def resolve_to_commit_hash(self, ref: str) -> str:
        """Resolve a branch, tag, or commit SHA to a full 40-char commit hash."""
        data = self._get(f"/repos/{self._slug}/commits/{ref}")
        return data["sha"]

    def get_archive_at(self, ref: str, extensions: set[str]) -> dict[str, bytes]:
        """Return {path: content_bytes} for all files at ref matching extensions.

        Downloads the repo as a single tarball (one request) and extracts
        matching files in memory. All content stays in memory.
        """
        resp = self._client.get(
            f"/repos/{self._slug}/tarball/{ref}",
            follow_redirects=True,
            timeout=120.0,
        )
        resp.raise_for_status()

        result: dict[str, bytes] = {}
        with tarfile.open(fileobj=io.BytesIO(resp.content), mode="r:gz") as tar:
            for member in tar.getmembers():
                if not member.isfile():
                    continue
                # GitHub tarballs have a leading directory component (org-repo-sha/...)
                # Strip it to get the repo-relative path.
                parts = member.name.split("/", 1)
                if len(parts) < 2:
                    continue
                path = parts[1]
                if not any(path.endswith(ext) for ext in extensions):
                    continue
                f = tar.extractfile(member)
                if f is not None:
                    result[path] = f.read()

        return result

    def get_config_file(self, ref: str) -> bytes | None:
        """Return .serpentine.toml bytes at ref, or None if not present."""
        return self._fetch_contents(".serpentine.toml", ref)

    def get_file_at(self, ref: str, path: str) -> bytes | None:
        """Return file contents at ref for a single path, or None if not found."""
        return self._fetch_contents(path, ref)

    def _fetch_contents(self, path: str, ref: str) -> bytes | None:
        """Fetch a single file via the Contents API. Returns raw bytes or None."""
        try:
            data = self._get(f"/repos/{self._slug}/contents/{path}", ref=ref)
            if isinstance(data, dict) and data.get("encoding") == "base64":
                return base64.b64decode(data["content"].replace("\n", ""))
            return None
        except httpx.HTTPStatusError as e:
            if e.response.status_code == 404:
                return None
            raise

    def close(self) -> None:
        self._client.close()

    def __del__(self) -> None:
        try:
            self._client.close()
        except Exception:
            pass
