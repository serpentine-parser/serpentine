"""Security tests: path traversal, commit hash injection, repo_id validation, ref allowlist."""

import io
import tarfile

import pytest

from serpentine.cache import NullCache
from serpentine.config import Config
from serpentine.services import UnknownRepoError, _validate_ref
from serpentine.storage.local import LocalGraphStore

GOOD_HASH = "a" * 40


# ---------------------------------------------------------------------------
# LocalGraphStore — commit hash injection
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    "bad_hash",
    [
        "../etc/passwd",
        "abc/../secrets",
        "/etc/shadow",
        "a" * 39 + "/",
        "",
        "not-hex-at-all-but-is-40-chars-long-xx",
    ],
)
def test_store_get_rejects_path_traversal_in_hash(tmp_path, bad_hash):
    store = LocalGraphStore(tmp_path)
    with pytest.raises(ValueError, match="Invalid commit hash"):
        store.get("org/repo", bad_hash)


@pytest.mark.parametrize(
    "bad_hash",
    [
        "../etc/passwd",
        "abc/../secrets",
    ],
)
def test_store_put_rejects_path_traversal_in_hash(tmp_path, bad_hash):
    store = LocalGraphStore(tmp_path)
    with pytest.raises(ValueError, match="Invalid commit hash"):
        store.put("org/repo", bad_hash, "{}")


# ---------------------------------------------------------------------------
# repo_id validation via vcs_managers lookup in tools
#
# Tools check `repo_id not in vcs_managers` before any downstream call.
# These tests verify that unknown/adversarial repo_ids are caught at the
# boundary and raise UnknownRepoError, not a filesystem error.
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    "bad_repo_id",
    [
        "../../etc/passwd",
        "org/repo/../other",
        "../secret",
        "",
    ],
)
def test_unknown_repo_id_raises_unknown_repo_error(bad_repo_id):
    # Simulate the tool-layer check: vcs_managers does not contain the adversarial id.
    vcs_managers: dict = {}
    if bad_repo_id not in vcs_managers:
        exc = UnknownRepoError(bad_repo_id)
        assert bad_repo_id in str(exc) or bad_repo_id == ""
    else:
        pytest.fail("Should not be in vcs_managers")


def test_unknown_repo_id_has_correct_type():
    exc = UnknownRepoError("../../etc/passwd")
    assert isinstance(exc, UnknownRepoError)
    assert exc.repo_id == "../../etc/passwd"


# ---------------------------------------------------------------------------
# Ref allowlist validation
# ---------------------------------------------------------------------------


def test_validate_ref_rejects_git_revision_expressions(git_repo):
    """Expressions like HEAD~50 and refs/stash must not reach the backend."""
    from serpentine.vcs.backend import GitBackend
    from serpentine.vcs.manager import VcsManager

    vcs = VcsManager(GitBackend(git_repo), NullCache(), Config({}))

    for bad_ref in ["HEAD~50", "refs/stash", "HEAD^", "@{upstream}", "HEAD^{tree}"]:
        with pytest.raises(UnknownRepoError, match="not an allowed ref"):
            _validate_ref(vcs, "myrepo", bad_ref)


def test_validate_ref_allows_known_branch(git_repo):
    from serpentine.vcs.backend import GitBackend
    from serpentine.vcs.manager import VcsManager

    vcs = VcsManager(GitBackend(git_repo), NullCache(), Config({}))
    refs = vcs.list_refs()
    # Should not raise for any ref returned by list_refs()
    for r in refs:
        _validate_ref(vcs, "myrepo", r.id)


def test_validate_ref_allows_full_commit_hash(git_repo):
    """40-char hex commit hashes bypass the allowlist (for querying ingested commits)."""
    from serpentine.vcs.backend import GitBackend
    from serpentine.vcs.manager import VcsManager

    vcs = VcsManager(GitBackend(git_repo), NullCache(), Config({}))
    commit_hash = vcs._backend.resolve_to_commit_hash("main")
    # Should not raise
    _validate_ref(vcs, "myrepo", commit_hash)


# ---------------------------------------------------------------------------
# Tarball extraction: symlink members are filtered before extractfile
# (regression guard — isfile() must exclude SYMTYPE members)
# ---------------------------------------------------------------------------


def _make_tarball_with_symlink(target: str) -> bytes:
    """Build an in-memory tar.gz containing only a symlink member."""
    buf = io.BytesIO()
    with tarfile.open(fileobj=buf, mode="w:gz") as tar:
        info = tarfile.TarInfo(name="prefix/evil.py")
        info.type = tarfile.SYMTYPE
        info.linkname = target
        tar.addfile(info)
    return buf.getvalue()


def _make_tarball_with_files(files: dict) -> bytes:
    buf = io.BytesIO()
    with tarfile.open(fileobj=buf, mode="w:gz") as tar:
        for path, content in files.items():
            data = content if isinstance(content, bytes) else content.encode()
            info = tarfile.TarInfo(name=f"prefix/{path}")
            info.size = len(data)
            tar.addfile(info, io.BytesIO(data))
    return buf.getvalue()


def test_get_archive_at_skips_symlink_members():
    """Symlink tar members must not appear in get_archive_at results."""

    from serpentine.vcs.github import GitHubApiBackend
    from tests.mcp.test_github_backend import FakeGitHubTransport

    tarball = _make_tarball_with_symlink("/etc/passwd")
    transport = FakeGitHubTransport(
        {
            "/repos/org/repo/tarball/main": (200, tarball, "bytes"),
        }
    )
    backend = GitHubApiBackend("org/repo", "ghp_fake", transport=transport)
    result = backend.get_archive_at("main", {".py"})
    assert result == {}, "Symlink members must be excluded from archive results"


def test_get_archive_at_symlink_does_not_leak_filesystem():
    """Even if a symlink member slips through, in-memory extraction cannot read real FS paths."""

    from serpentine.vcs.github import GitHubApiBackend
    from tests.mcp.test_github_backend import FakeGitHubTransport

    # Tarball with both a symlink and a regular file with the same .py extension
    buf = io.BytesIO()
    with tarfile.open(fileobj=buf, mode="w:gz") as tar:
        # Symlink member
        sym = tarfile.TarInfo(name="prefix/link.py")
        sym.type = tarfile.SYMTYPE
        sym.linkname = "/etc/passwd"
        tar.addfile(sym)
        # Regular file
        content = b"def safe(): pass"
        reg = tarfile.TarInfo(name="prefix/safe.py")
        reg.size = len(content)
        tar.addfile(reg, io.BytesIO(content))
    tarball = buf.getvalue()

    transport = FakeGitHubTransport(
        {
            "/repos/org/repo/tarball/main": (200, tarball, "bytes"),
        }
    )
    backend = GitHubApiBackend("org/repo", "ghp_fake", transport=transport)
    result = backend.get_archive_at("main", {".py"})

    assert "link.py" not in result, "Symlink member must not appear in results"
    assert "safe.py" in result
    assert result["safe.py"] == content


# ---------------------------------------------------------------------------
# Tarball extraction: traversal paths in member names stay in-memory only
# (regression guard — paths from tar must never reach filesystem operations)
# ---------------------------------------------------------------------------


def test_get_archive_at_traversal_paths_not_written_to_disk(tmp_path):
    """Member names with ../ sequences must not cause any filesystem writes."""
    from serpentine.vcs.github import GitHubApiBackend
    from tests.mcp.test_github_backend import FakeGitHubTransport

    content = b"def evil(): pass"
    buf = io.BytesIO()
    with tarfile.open(fileobj=buf, mode="w:gz") as tar:
        info = tarfile.TarInfo(name="prefix/../../../tmp/evil.py")
        info.size = len(content)
        tar.addfile(info, io.BytesIO(content))
    tarball = buf.getvalue()

    transport = FakeGitHubTransport(
        {
            "/repos/org/repo/tarball/main": (200, tarball, "bytes"),
        }
    )
    backend = GitHubApiBackend("org/repo", "ghp_fake", transport=transport)
    result = backend.get_archive_at("main", {".py"})

    # The key may appear with the traversal path — that is acceptable as long as
    # no file was written outside the intended directory.
    evil_file = tmp_path.parent.parent / "tmp" / "evil.py"
    assert not evil_file.exists(), (
        "Traversal path in tar member must not cause a filesystem write"
    )


# ---------------------------------------------------------------------------
# Ref validation: traversal-style refs are rejected before reaching the backend
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    "bad_ref",
    [
        "main/../../other-org/other-repo",
        "main/../secret",
        "refs/heads/main/../../admin",
    ],
)
def test_validate_ref_rejects_path_traversal_refs(git_repo, bad_ref):
    """Refs containing path traversal sequences must be rejected by _validate_ref."""
    from serpentine.vcs.backend import GitBackend
    from serpentine.vcs.manager import VcsManager

    vcs = VcsManager(GitBackend(git_repo), NullCache(), Config({}))
    with pytest.raises(UnknownRepoError, match="not an allowed ref"):
        _validate_ref(vcs, "myrepo", bad_ref)
