"""Tests for LocalGraphStore and build_store()."""

import os

import pytest

from serpentine.storage.factory import ConfigError, build_store
from serpentine.storage.local import LocalGraphStore

GOOD_HASH = "a" * 40
OTHER_HASH = "b" * 40


# ---------------------------------------------------------------------------
# LocalGraphStore — basic round-trip
# ---------------------------------------------------------------------------

def test_put_and_get(tmp_path):
    store = LocalGraphStore(tmp_path)
    store.put("myrepo", GOOD_HASH, '{"nodes": []}')
    assert store.get("myrepo", GOOD_HASH) == '{"nodes": []}'


def test_get_missing_returns_none(tmp_path):
    store = LocalGraphStore(tmp_path)
    assert store.get("myrepo", GOOD_HASH) is None


def test_put_creates_nested_dirs(tmp_path):
    store = LocalGraphStore(tmp_path)
    store.put("org/repo", GOOD_HASH, "{}")
    assert (tmp_path / "org" / "repo" / f"{GOOD_HASH}.json").exists()


def test_relative_root_rejected():
    with pytest.raises(ValueError, match="absolute"):
        LocalGraphStore(pytest.importorskip("pathlib").Path("relative/path"))


# ---------------------------------------------------------------------------
# LocalGraphStore — commit hash validation
# ---------------------------------------------------------------------------

@pytest.mark.parametrize("bad_hash", [
    "abc",
    "../etc/passwd",
    "a" * 39,
    "a" * 41,
    "g" * 40,
    "A" * 40,
    "abc/def",
    "",
])
def test_get_rejects_bad_commit_hash(tmp_path, bad_hash):
    store = LocalGraphStore(tmp_path)
    with pytest.raises(ValueError, match="Invalid commit hash"):
        store.get("repo", bad_hash)


@pytest.mark.parametrize("bad_hash", [
    "../etc/passwd",
    "a" * 39,
    "g" * 40,
])
def test_put_rejects_bad_commit_hash(tmp_path, bad_hash):
    store = LocalGraphStore(tmp_path)
    with pytest.raises(ValueError, match="Invalid commit hash"):
        store.put("repo", bad_hash, "{}")


# ---------------------------------------------------------------------------
# build_store()
# ---------------------------------------------------------------------------

def test_build_store_missing_backend(monkeypatch):
    monkeypatch.delenv("SERPENTINE_STORE_BACKEND", raising=False)
    with pytest.raises(ConfigError, match="SERPENTINE_STORE_BACKEND"):
        build_store()


def test_build_store_local_missing_path(monkeypatch):
    monkeypatch.setenv("SERPENTINE_STORE_BACKEND", "local")
    monkeypatch.delenv("SERPENTINE_LOCAL_STORE_PATH", raising=False)
    with pytest.raises(ConfigError, match="SERPENTINE_LOCAL_STORE_PATH"):
        build_store()


def test_build_store_local_ok(monkeypatch, tmp_path):
    monkeypatch.setenv("SERPENTINE_STORE_BACKEND", "local")
    monkeypatch.setenv("SERPENTINE_LOCAL_STORE_PATH", str(tmp_path))
    store = build_store()
    assert isinstance(store, LocalGraphStore)


def test_build_store_unknown_backend(monkeypatch):
    monkeypatch.setenv("SERPENTINE_STORE_BACKEND", "badvalue")
    with pytest.raises(ConfigError, match="Unknown"):
        build_store()
