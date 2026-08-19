"""Shared fixtures for MCP tests."""

import os
import textwrap

import pygit2
import pytest

from serpentine.cache import NullCache
from serpentine.config import Config
from serpentine.mcp import config as mcp_config
from serpentine.storage.local import LocalGraphStore
from serpentine.vcs.backend import GitBackend
from serpentine.vcs.manager import VcsManager

SERPENTINE_TOML = textwrap.dedent("""\
    [analysis]
    extensions = [".py"]
    exclude_dirs = []
""")

SAMPLE_PY = textwrap.dedent("""\
    def hello():
        return "world"

    class Greeter:
        def greet(self, name: str) -> str:
            return hello() + name
""")


def _init_repo(path) -> pygit2.Repository:
    """Create a minimal git repo with one commit."""
    repo = pygit2.init_repository(str(path))
    sig = pygit2.Signature("Test", "test@example.com")

    # Write files to disk (GitBackend reads from working tree)
    (path / ".serpentine.toml").write_text(SERPENTINE_TOML)
    (path / "sample.py").write_text(SAMPLE_PY)

    index = repo.index
    index.add(".serpentine.toml")
    index.add("sample.py")
    index.write()
    tree = index.write_tree()
    repo.create_commit("refs/heads/main", sig, sig, "initial", tree, [])
    return repo


@pytest.fixture
def git_repo(tmp_path):
    """A minimal git repo with .serpentine.toml and sample.py on branch main."""
    repo_path = tmp_path / "myrepo"
    repo_path.mkdir()
    _init_repo(repo_path)
    return repo_path


@pytest.fixture
def store(tmp_path):
    return LocalGraphStore(tmp_path / "store")


@pytest.fixture
def vcs(git_repo):
    backend = GitBackend(git_repo)
    return VcsManager(backend, NullCache(), Config.load(git_repo))


@pytest.fixture(autouse=True)
def reset_mcp_config_hooks():
    """Isolate the serpentine.mcp.config._HOOKS registry between tests."""
    saved = dict(mcp_config._HOOKS)
    mcp_config._HOOKS.clear()
    yield
    mcp_config._HOOKS.clear()
    mcp_config._HOOKS.update(saved)
