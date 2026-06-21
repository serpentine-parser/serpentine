"""Integration tests: real local git repo + LocalGraphStore."""

import json
import textwrap

import pytest

from serpentine.cache import NullCache
from serpentine.config import Config
from serpentine.services import MissingConfigError, NotIngestedError, get_graph, ingest_ref
from serpentine.vcs.backend import GitBackend
from serpentine.vcs.manager import VcsManager


def test_full_ingest_and_retrieve(git_repo, store):
    """Ingest local repo then get_graph round-trips correctly with nodes."""
    vcs = VcsManager(GitBackend(git_repo), NullCache(), Config.load(git_repo))
    commit_hash = ingest_ref(vcs, store, "myrepo", "main")
    assert len(commit_hash) == 40

    graph = get_graph(store, vcs, "myrepo", "main")
    assert "nodes" in graph
    assert len(graph["nodes"]) > 0


def test_ingest_missing_config_raises(tmp_path, store):
    """ingest_ref without .serpentine.toml raises MissingConfigError."""
    import pygit2
    repo_path = tmp_path / "notoml"
    repo_path.mkdir()

    repo = pygit2.init_repository(str(repo_path))
    sig = pygit2.Signature("Test", "test@example.com")
    sample = repo_path / "sample.py"
    sample.write_text("x = 1\n")
    index = repo.index
    index.add("sample.py")
    index.write()
    tree = index.write_tree()
    repo.create_commit("refs/heads/main", sig, sig, "init", tree, [])

    vcs = VcsManager(GitBackend(repo_path), NullCache(), Config({}))
    with pytest.raises(MissingConfigError) as exc_info:
        ingest_ref(vcs, store, "notoml", "main")
    assert "notoml" in str(exc_info.value)


def test_ingest_ignore_config_succeeds(tmp_path, store):
    """ingest_ref with ignore_config=True succeeds even without .serpentine.toml."""
    import pygit2
    repo_path = tmp_path / "notoml2"
    repo_path.mkdir()

    repo = pygit2.init_repository(str(repo_path))
    sig = pygit2.Signature("Test", "test@example.com")
    sample = repo_path / "sample.py"
    sample.write_text("def foo(): pass\n")
    index = repo.index
    index.add("sample.py")
    index.write()
    tree = index.write_tree()
    repo.create_commit("refs/heads/main", sig, sig, "init", tree, [])

    vcs = VcsManager(GitBackend(repo_path), NullCache(), Config({}))
    commit_hash = ingest_ref(vcs, store, "notoml2", "main", ignore_config=True)
    assert len(commit_hash) == 40


def test_get_graph_not_ingested_raises(git_repo, store):
    """get_graph on a ref that hasn't been ingested raises NotIngestedError."""
    vcs = VcsManager(GitBackend(git_repo), NullCache(), Config.load(git_repo))
    with pytest.raises(NotIngestedError) as exc_info:
        get_graph(store, vcs, "myrepo", "main")
    assert "myrepo" in str(exc_info.value)
    assert "main" in str(exc_info.value)


def test_list_refs_returns_branch(git_repo):
    """list_refs returns at least the main branch."""
    vcs = VcsManager(GitBackend(git_repo), NullCache(), Config.load(git_repo))
    refs = vcs.list_refs()
    names = [r.display for r in refs]
    assert any("main" in n for n in names)
