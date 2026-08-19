"""Tests for GitHubApiBackend using a fake httpx transport + hypothesis fixture data."""

import base64
import io
import tarfile
from typing import Any

import httpx
import pytest
from hypothesis import given
from hypothesis import strategies as st

from serpentine.vcs.github import GitHubApiBackend

# ---------------------------------------------------------------------------
# Fake transport
# ---------------------------------------------------------------------------


class FakeGitHubTransport(httpx.BaseTransport):
    """In-memory httpx transport. Routes requests by path, longest match wins.

    Route values can be:
      (status_code, json_data)      — returns JSON response
      (status_code, bytes, "bytes") — returns raw bytes response (e.g. tarball)
    """

    def __init__(self, routes: dict[str, tuple]) -> None:
        self._routes = routes

    def handle_request(self, request: httpx.Request) -> httpx.Response:
        path = request.url.path
        for fragment in sorted(self._routes, key=len, reverse=True):
            if (
                path == fragment
                or path.startswith(fragment + "?")
                or path.endswith(fragment)
            ):
                entry = self._routes[fragment]
                if len(entry) == 3 and entry[2] == "bytes":
                    status, body, _ = entry
                    return httpx.Response(status, content=body)
                status, body = entry
                return httpx.Response(status, json=body)
        return httpx.Response(404, json={"message": "Not Found"})


def _b64(content: bytes) -> str:
    return base64.b64encode(content).decode()


def _make_tarball(files: dict[str, bytes], prefix: str = "org-repo-abc1234") -> bytes:
    """Build an in-memory .tar.gz with a GitHub-style leading directory prefix."""
    buf = io.BytesIO()
    with tarfile.open(fileobj=buf, mode="w:gz") as tar:
        for path, content in files.items():
            member = tarfile.TarInfo(name=f"{prefix}/{path}")
            member.size = len(content)
            tar.addfile(member, io.BytesIO(content))
    return buf.getvalue()


def _make_backend(routes: dict[str, tuple[int, Any]]) -> GitHubApiBackend:
    transport = FakeGitHubTransport(routes)
    return GitHubApiBackend("org/repo", "ghp_fake", transport=transport)


# ---------------------------------------------------------------------------
# Hypothesis strategies
# ---------------------------------------------------------------------------

hex_sha = st.text(alphabet="0123456789abcdef", min_size=40, max_size=40)

py_file_content = st.text(
    alphabet=st.characters(blacklist_categories=("Cs",)),
    min_size=0,
    max_size=512,
)

file_path_component = st.text(
    alphabet="abcdefghijklmnopqrstuvwxyz_",
    min_size=1,
    max_size=20,
)

py_file_path = st.builds(
    lambda parts, name: "/".join(parts) + "/" + name + ".py" if parts else name + ".py",
    parts=st.lists(file_path_component, min_size=0, max_size=3),
    name=file_path_component,
)


# ---------------------------------------------------------------------------
# list_refs
# ---------------------------------------------------------------------------


class TestListRefs:
    @given(
        branch_names=st.lists(
            st.text(alphabet="abcdefghijklmnopqrstuvwxyz-", min_size=1, max_size=20),
            min_size=1,
            max_size=5,
            unique=True,
        )
    )
    def test_branches_appear_in_refs(self, branch_names):
        branches = [{"name": n} for n in branch_names]
        routes = {
            "/repos/org/repo/branches": (200, branches),
            "/repos/org/repo/tags": (200, []),
            "/repos/org/repo": (200, {"default_branch": "main"}),
            "/repos/org/repo/commits": (200, []),
        }
        backend = _make_backend(routes)
        refs = backend.list_refs()
        returned_ids = {r.id for r in refs}
        for name in branch_names:
            assert name in returned_ids

    @given(
        tag_names=st.lists(
            st.text(alphabet="v0123456789.", min_size=2, max_size=10),
            min_size=1,
            max_size=5,
            unique=True,
        )
    )
    def test_tags_appear_in_refs(self, tag_names):
        tags = [{"name": n} for n in tag_names]
        routes = {
            "/repos/org/repo/branches": (200, []),
            "/repos/org/repo/tags": (200, tags),
            "/repos/org/repo": (200, {"default_branch": "main"}),
            "/repos/org/repo/commits": (200, []),
        }
        backend = _make_backend(routes)
        refs = backend.list_refs()
        returned_ids = {r.id for r in refs}
        for name in tag_names:
            assert name in returned_ids

    @given(sha=hex_sha, message=st.text(min_size=0, max_size=200))
    def test_commits_appear_in_refs(self, sha, message):
        routes = {
            "/repos/org/repo/branches": (200, []),
            "/repos/org/repo/tags": (200, []),
            "/repos/org/repo": (200, {"default_branch": "main"}),
            "/repos/org/repo/commits": (
                200,
                [{"sha": sha, "commit": {"message": message}}],
            ),
        }
        backend = _make_backend(routes)
        refs = backend.list_refs()
        commit_refs = [r for r in refs if r.kind == "commit"]
        assert len(commit_refs) == 1
        assert commit_refs[0].id == sha
        # display truncated to 7 + space + 60 chars max
        assert len(commit_refs[0].display) <= 68

    def test_all_ref_kinds_present(self):
        routes = {
            "/repos/org/repo/branches": (200, [{"name": "main"}]),
            "/repos/org/repo/tags": (200, [{"name": "v1.0"}]),
            "/repos/org/repo": (200, {"default_branch": "main"}),
            "/repos/org/repo/commits": (
                200,
                [
                    {
                        "sha": "a" * 40,
                        "commit": {"message": "initial"},
                    }
                ],
            ),
        }
        backend = _make_backend(routes)
        refs = backend.list_refs()
        kinds = {r.kind for r in refs}
        assert kinds == {"branch", "tag", "commit"}


# ---------------------------------------------------------------------------
# resolve_to_commit_hash
# ---------------------------------------------------------------------------


class TestResolveToCommitHash:
    @given(
        ref=st.text(
            alphabet=st.characters(
                whitelist_categories=("Lu", "Ll", "Nd"), whitelist_characters="-_"
            ),
            min_size=1,
            max_size=40,
        ),
        sha=hex_sha,
    )
    def test_returns_sha_from_api(self, ref, sha):
        routes = {f"/repos/org/repo/commits/{ref}": (200, {"sha": sha})}
        backend = _make_backend(routes)
        assert backend.resolve_to_commit_hash(ref) == sha

    @given(sha=hex_sha)
    def test_resolves_full_sha(self, sha):
        routes = {f"/repos/org/repo/commits/{sha}": (200, {"sha": sha})}
        backend = _make_backend(routes)
        assert backend.resolve_to_commit_hash(sha) == sha


# ---------------------------------------------------------------------------
# get_archive_at
# ---------------------------------------------------------------------------


class TestGetArchiveAt:
    @given(path=py_file_path, content=py_file_content)
    def test_py_files_included(self, path, content):
        tarball = _make_tarball({path: content.encode()})
        routes = {"/repos/org/repo/tarball/main": (200, tarball, "bytes")}
        backend = _make_backend(routes)
        result = backend.get_archive_at("main", {".py"})
        assert path in result
        assert result[path] == content.encode()

    @given(content=py_file_content)
    def test_non_matching_extensions_excluded(self, content):
        tarball = _make_tarball({"README.md": content.encode()})
        routes = {"/repos/org/repo/tarball/main": (200, tarball, "bytes")}
        backend = _make_backend(routes)
        result = backend.get_archive_at("main", {".py"})
        assert result == {}

    def test_multiple_files_in_one_request(self):
        files = {
            "src/foo.py": b"def foo(): pass",
            "src/bar.py": b"def bar(): pass",
            "README.md": b"# readme",
        }
        tarball = _make_tarball(files)
        routes = {"/repos/org/repo/tarball/main": (200, tarball, "bytes")}
        backend = _make_backend(routes)
        result = backend.get_archive_at("main", {".py"})
        assert set(result.keys()) == {"src/foo.py", "src/bar.py"}
        assert result["src/foo.py"] == b"def foo(): pass"

    def test_leading_prefix_stripped(self):
        tarball = _make_tarball(
            {"src/hello.py": b"hello"}, prefix="myorg-myrepo-deadbeef"
        )
        routes = {"/repos/org/repo/tarball/main": (200, tarball, "bytes")}
        backend = _make_backend(routes)
        result = backend.get_archive_at("main", {".py"})
        assert "src/hello.py" in result
        assert "myorg-myrepo-deadbeef/src/hello.py" not in result


# ---------------------------------------------------------------------------
# get_config_file
# ---------------------------------------------------------------------------


class TestGetConfigFile:
    @given(content=st.binary(min_size=1, max_size=512))
    def test_returns_content_when_present(self, content):
        routes = {
            "/repos/org/repo/contents/.serpentine.toml": (
                200,
                {
                    "encoding": "base64",
                    "content": _b64(content),
                },
            ),
        }
        backend = _make_backend(routes)
        assert backend.get_config_file("main") == content

    def test_returns_none_on_404(self):
        routes = {
            "/repos/org/repo/contents/.serpentine.toml": (404, {"message": "Not Found"})
        }
        backend = _make_backend(routes)
        assert backend.get_config_file("main") is None


# ---------------------------------------------------------------------------
# get_file_at
# ---------------------------------------------------------------------------


class TestGetFileAt:
    @given(path=py_file_path, content=st.binary(min_size=1, max_size=512))
    def test_returns_content_when_present(self, path, content):
        routes = {
            f"/repos/org/repo/contents/{path}": (
                200,
                {
                    "encoding": "base64",
                    "content": _b64(content),
                },
            ),
        }
        backend = _make_backend(routes)
        assert backend.get_file_at("main", path) == content

    def test_returns_none_on_404(self):
        routes = {
            "/repos/org/repo/contents/missing.py": (404, {"message": "Not Found"})
        }
        backend = _make_backend(routes)
        assert backend.get_file_at("main", "missing.py") is None


# ---------------------------------------------------------------------------
# Factory tests
# ---------------------------------------------------------------------------


class TestFactory:
    def test_github_slug_without_token_raises(self, monkeypatch):
        monkeypatch.delenv("SERPENTINE_GITHUB_TOKEN", raising=False)
        monkeypatch.delenv("SERPENTINE_REPOS_DIR", raising=False)
        from serpentine.storage.factory import ConfigError
        from serpentine.vcs.factory import build_vcs_manager

        with pytest.raises(ConfigError, match="SERPENTINE_GITHUB_TOKEN"):
            build_vcs_manager("org/repo")

    def test_github_slug_with_token_returns_manager(self, monkeypatch):
        monkeypatch.setenv("SERPENTINE_GITHUB_TOKEN", "ghp_test")
        monkeypatch.delenv("SERPENTINE_REPOS_DIR", raising=False)
        from serpentine.vcs.factory import build_vcs_manager
        from serpentine.vcs.github import GitHubApiBackend
        from serpentine.vcs.manager import VcsManager

        mgr = build_vcs_manager("org/repo")
        assert isinstance(mgr, VcsManager)
        assert isinstance(mgr._backend, GitHubApiBackend)

    def test_non_slug_without_repos_dir_raises(self, monkeypatch):
        monkeypatch.delenv("SERPENTINE_REPOS_DIR", raising=False)
        monkeypatch.delenv("SERPENTINE_GITHUB_TOKEN", raising=False)
        from serpentine.storage.factory import ConfigError
        from serpentine.vcs.factory import build_vcs_manager

        with pytest.raises(ConfigError):
            build_vcs_manager("notaslug")

    def test_local_repo_takes_precedence_over_github(self, monkeypatch, tmp_path):
        import pygit2

        repo_path = tmp_path / "myrepo"
        repo_path.mkdir()
        (repo_path / ".serpentine.toml").write_text(
            '[analysis]\nextensions = [".py"]\n'
        )
        sig = pygit2.Signature("T", "t@t.com")
        repo = pygit2.init_repository(str(repo_path))
        index = repo.index
        index.add(".serpentine.toml")
        index.write()
        tree = index.write_tree()
        repo.create_commit("refs/heads/main", sig, sig, "init", tree, [])

        monkeypatch.setenv("SERPENTINE_REPOS_DIR", str(tmp_path))
        monkeypatch.setenv("SERPENTINE_GITHUB_TOKEN", "ghp_test")

        from serpentine.vcs.backend import GitBackend
        from serpentine.vcs.factory import build_vcs_manager

        mgr = build_vcs_manager("myrepo")
        assert isinstance(mgr._backend, GitBackend)

    def test_build_vcs_managers_includes_github_slugs(self, monkeypatch):
        monkeypatch.delenv("SERPENTINE_REPOS_DIR", raising=False)
        monkeypatch.setenv("SERPENTINE_GITHUB_TOKEN", "ghp_test")
        monkeypatch.setenv("SERPENTINE_ALLOWED_REPOS", "org/repo1,org/repo2")
        from serpentine.vcs.factory import build_vcs_managers
        from serpentine.vcs.github import GitHubApiBackend

        managers = build_vcs_managers()
        assert "org/repo1" in managers
        assert "org/repo2" in managers
        assert isinstance(managers["org/repo1"]._backend, GitHubApiBackend)
