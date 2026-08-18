"""VCS backend abstraction and GitBackend implementation."""

import logging
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Protocol, runtime_checkable

logger = logging.getLogger(__name__)

MAX_COMMITS = 100


def _commit_time_iso(commit_time: int) -> str:
    return datetime.fromtimestamp(commit_time, tz=timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


@dataclass
class VcsRef:
    id: str
    display: str
    kind: str  # "branch" | "tag" | "commit"
    commit_hash: str | None = None  # pre-resolved SHA when available, avoids extra API call
    timestamp: str | None = None  # ISO 8601 UTC committer date, None when unavailable (e.g. GitHub branch/tag)


@runtime_checkable
class VcsBackend(Protocol):
    def list_refs(self) -> list[VcsRef]: ...
    def get_archive_at(self, ref: str, extensions: set[str]) -> dict[str, bytes]: ...
    def resolve_to_commit_hash(self, ref: str) -> str: ...
    def get_config_file(self, ref: str) -> bytes | None: ...
    def get_file_at(self, ref: str, path: str) -> bytes | None: ...


class GitBackend:
    """pygit2-based Git backend. No subprocess, no git binary required."""

    def __init__(self, repo_path: Path) -> None:
        import pygit2

        self._repo = pygit2.Repository(str(repo_path))

    def list_refs(self) -> list[VcsRef]:
        import pygit2

        refs: list[VcsRef] = []

        # Local branches
        for branch_name in self._repo.branches.local:
            branch = self._repo.branches.local[branch_name]
            sha = str(branch.target) if branch.target else None
            timestamp = None
            try:
                commit = branch.peel(pygit2.Commit)
                timestamp = _commit_time_iso(commit.commit_time)
            except Exception:
                pass
            refs.append(VcsRef(
                id=branch_name, display=branch_name, kind="branch", commit_hash=sha, timestamp=timestamp
            ))

        # Tags
        for ref_name in self._repo.references:
            if ref_name.startswith("refs/tags/"):
                tag_name = ref_name[len("refs/tags/"):]
                try:
                    obj = self._repo.references[ref_name].peel(pygit2.Commit)
                    sha = str(obj.id)
                    timestamp = _commit_time_iso(obj.commit_time)
                except Exception:
                    sha = None
                    timestamp = None
                refs.append(VcsRef(id=tag_name, display=tag_name, kind="tag", commit_hash=sha, timestamp=timestamp))

        # Recent commits (hard cap at MAX_COMMITS)
        try:
            head = self._repo.head
            walker = self._repo.walk(head.target, pygit2.GIT_SORT_TIME)
            for i, commit in enumerate(walker):
                if i >= MAX_COMMITS:
                    break
                short_id = str(commit.id)[:7]
                sha = str(commit.id)
                message = commit.message.split("\n")[0][:60]
                refs.append(VcsRef(
                    id=sha,
                    display=f"{short_id} {message}",
                    kind="commit",
                    commit_hash=sha,
                    timestamp=_commit_time_iso(commit.commit_time),
                ))
        except pygit2.GitError:
            pass

        return refs

    def resolve_to_commit_hash(self, ref: str) -> str:
        """Resolve a ref string (branch, tag, or commit hash) to a full commit hash."""
        import pygit2

        obj = self._repo.revparse_single(ref)
        if hasattr(obj, "peel"):
            try:
                commit = obj.peel(pygit2.Commit)
                return str(commit.id)
            except Exception:
                pass
        return str(obj.id)

    def get_archive_at(self, ref: str, extensions: set[str]) -> dict[str, bytes]:
        """Return {relative_path_str: file_content_bytes} for files at ref matching extensions.

        Callers (routes.py) validate ref against list_refs() before reaching here,
        so ref is always a known branch name, tag, or full commit hash — never
        arbitrary user input that could trigger unexpected pygit2 behaviour.
        """
        import pygit2

        obj = self._repo.revparse_single(ref)
        if hasattr(obj, "peel"):
            try:
                commit = obj.peel(pygit2.Commit)
            except Exception:
                commit = obj
        else:
            commit = obj

        tree = commit.peel(pygit2.Tree)
        result: dict[str, bytes] = {}
        self._walk_tree(tree, "", result, extensions)
        return result

    def get_file_at(self, ref: str, path: str) -> bytes | None:
        """Return file contents at ref for a single path, or None if not found."""
        import pygit2

        try:
            obj = self._repo.revparse_single(ref)
            if hasattr(obj, "peel"):
                try:
                    commit = obj.peel(pygit2.Commit)
                except Exception:
                    commit = obj
            else:
                commit = obj
            tree = commit.peel(pygit2.Tree)
            parts = path.split("/")
            node = tree
            for part in parts[:-1]:
                node = self._repo.get(node[part].id)
            entry = node[parts[-1]]
            blob = self._repo.get(entry.id)
            return bytes(blob.data)
        except (KeyError, Exception):
            return None

    def get_config_file(self, ref: str) -> bytes | None:
        """Return .serpentine.toml bytes at ref, or None if not present."""
        import pygit2

        try:
            obj = self._repo.revparse_single(ref)
            if hasattr(obj, "peel"):
                try:
                    commit = obj.peel(pygit2.Commit)
                except Exception:
                    commit = obj
            else:
                commit = obj
            tree = commit.peel(pygit2.Tree)
            entry = tree[".serpentine.toml"]
            blob = self._repo.get(entry.id)
            return bytes(blob.data)
        except (KeyError, Exception):
            return None

    def _walk_tree(self, tree: object, prefix: str, result: dict[str, bytes], extensions: set[str]) -> None:
        import pygit2

        for entry in tree:
            path = f"{prefix}{entry.name}" if prefix else entry.name
            if entry.type_str == "tree":
                subtree = self._repo.get(entry.id)
                self._walk_tree(subtree, path + "/", result, extensions)
            elif entry.type_str == "blob":
                if any(path.endswith(ext) for ext in extensions):
                    blob = self._repo.get(entry.id)
                    result[path] = blob.data


def detect_backend(path: Path) -> "GitBackend | None":
    """Return a GitBackend if path is inside a git repo and pygit2 is available, else None."""
    try:
        import pygit2
    except ImportError:
        return None

    try:
        repo_path = pygit2.discover_repository(str(path))
        return GitBackend(Path(repo_path).parent)
    except Exception:
        return None
