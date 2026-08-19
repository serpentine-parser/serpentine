"""Tests for GcsGraphStore and build_store() GCS path."""

from unittest.mock import MagicMock, patch

import pytest

from serpentine.storage.factory import ConfigError, build_store
from serpentine.storage.gcs import GcsGraphStore

GOOD_HASH = "a" * 40


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _make_store(bucket_mock: MagicMock) -> GcsGraphStore:
    """Build a GcsGraphStore wired to a pre-created mock bucket."""
    store = object.__new__(GcsGraphStore)
    store._bucket = bucket_mock
    return store


# ---------------------------------------------------------------------------
# GcsGraphStore — round-trip
# ---------------------------------------------------------------------------


def test_put_calls_upload(tmp_path):
    bucket = MagicMock()
    store = _make_store(bucket)
    store.put("org/repo", GOOD_HASH, '{"nodes": []}')
    blob = bucket.blob.return_value
    blob.upload_from_string.assert_called_once_with(
        '{"nodes": []}', content_type="application/json"
    )


def test_get_returns_text_when_blob_exists():
    bucket = MagicMock()
    blob = MagicMock()
    blob.exists.return_value = True
    blob.download_as_text.return_value = '{"nodes": []}'
    bucket.blob.return_value = blob

    store = _make_store(bucket)
    result = store.get("org/repo", GOOD_HASH)
    assert result == '{"nodes": []}'


def test_get_returns_none_when_blob_missing():
    bucket = MagicMock()
    blob = MagicMock()
    blob.exists.return_value = False
    bucket.blob.return_value = blob

    store = _make_store(bucket)
    assert store.get("org/repo", GOOD_HASH) is None


def test_key_format():
    bucket = MagicMock()
    store = _make_store(bucket)
    store.put("org/repo", GOOD_HASH, "{}")
    bucket.blob.assert_called_once_with(f"org/repo/{GOOD_HASH}.json")


# ---------------------------------------------------------------------------
# GcsGraphStore — commit hash validation
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    "bad_hash",
    [
        "abc",
        "../etc/passwd",
        "a" * 39,
        "g" * 40,
        "",
    ],
)
def test_get_rejects_bad_hash(bad_hash):
    store = _make_store(MagicMock())
    with pytest.raises(ValueError, match="Invalid commit hash"):
        store.get("repo", bad_hash)


@pytest.mark.parametrize(
    "bad_hash",
    [
        "../etc/passwd",
        "a" * 39,
        "g" * 40,
    ],
)
def test_put_rejects_bad_hash(bad_hash):
    store = _make_store(MagicMock())
    with pytest.raises(ValueError, match="Invalid commit hash"):
        store.put("repo", bad_hash, "{}")


# ---------------------------------------------------------------------------
# build_store() — gcs path
# ---------------------------------------------------------------------------


def test_build_store_gcs_missing_bucket(monkeypatch):
    monkeypatch.setenv("SERPENTINE_STORE_BACKEND", "gcs")
    monkeypatch.delenv("SERPENTINE_GCS_BUCKET", raising=False)
    with pytest.raises(ConfigError, match="SERPENTINE_GCS_BUCKET"):
        build_store()


def test_build_store_gcs_missing_deps(monkeypatch):
    monkeypatch.setenv("SERPENTINE_STORE_BACKEND", "gcs")
    monkeypatch.setenv("SERPENTINE_GCS_BUCKET", "my-bucket")
    with patch.dict("sys.modules", {"google.cloud.storage": None}):
        with pytest.raises((ConfigError, ImportError)):
            build_store()


def test_build_store_gcs_ok(monkeypatch):
    monkeypatch.setenv("SERPENTINE_STORE_BACKEND", "gcs")
    monkeypatch.setenv("SERPENTINE_GCS_BUCKET", "my-bucket")

    mock_bucket = MagicMock()
    mock_client = MagicMock()
    mock_client.return_value.bucket.return_value = mock_bucket
    mock_gcs_module = MagicMock()
    mock_gcs_module.Client = mock_client

    with patch.dict(
        "sys.modules",
        {
            "google.cloud.storage": mock_gcs_module,
            "google.cloud": MagicMock(),
            "google": MagicMock(),
        },
    ):
        store = build_store()

    assert isinstance(store, GcsGraphStore)
