"""VCS backend abstraction and GitBackend implementation."""

import logging
from dataclasses import dataclass
from pathlib import Path
from typing import Protocol, runtime_checkable

logger = logging.getLogger(__name__)

SUPPORTED_EXTENSIONS = {".py", ".js", ".jsx", ".ts", ".tsx", ".rs", ".tf"}
MAX_COMMITS = 100


@dataclass
class VcsRef:
    id: str
    display: str
    kind: str  # "branch" | "tag" | "commit"


@runtime_checkable
class VcsBackend(Protocol):
    def list_refs(self) -> list[VcsRef]: ...
    def get_archive_at(self, ref: str) -> dict[str, bytes]: ...
    def resolve_to_commit_hash(self, ref: str) -> str: ...


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
            refs.append(VcsRef(id=branch_name, display=branch_name, kind="branch"))

        # Tags
        for ref_name in self._repo.references:
            if ref_name.startswith("refs/tags/"):
                tag_name = ref_name[len("refs/tags/"):]
                refs.append(VcsRef(id=tag_name, display=tag_name, kind="tag"))

        # Recent commits (hard cap at MAX_COMMITS)
        try:
            head = self._repo.head
            walker = self._repo.walk(head.target, pygit2.GIT_SORT_TIME)
            for i, commit in enumerate(walker):
                if i >= MAX_COMMITS:
                    break
                short_id = str(commit.id)[:7]
                message = commit.message.split("\n")[0][:60]
                refs.append(VcsRef(
                    id=str(commit.id),
                    display=f"{short_id} {message}",
                    kind="commit",
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

    def get_archive_at(self, ref: str) -> dict[str, bytes]:
        """Return {relative_path_str: file_content_bytes} for all supported files at ref."""
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
        self._walk_tree(tree, "", result)
        return result

    def _walk_tree(self, tree: object, prefix: str, result: dict[str, bytes]) -> None:
        import pygit2

        for entry in tree:
            path = f"{prefix}{entry.name}" if prefix else entry.name
            if entry.type_str == "tree":
                subtree = self._repo.get(entry.id)
                self._walk_tree(subtree, path + "/", result)
            elif entry.type_str == "blob":
                if any(path.endswith(ext) for ext in SUPPORTED_EXTENSIONS):
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
