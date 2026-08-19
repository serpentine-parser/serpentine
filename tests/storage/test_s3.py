"""Tests for S3GraphStore and build_store() S3 path."""

from unittest.mock import MagicMock, patch

import pytest

from serpentine.storage.factory import ConfigError, build_store
from serpentine.storage.s3 import S3GraphStore

botocore = pytest.importorskip("botocore", reason="botocore not installed")

GOOD_HASH = "a" * 40


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _make_store(client_mock: MagicMock, bucket: str = "test-bucket") -> S3GraphStore:
    store = object.__new__(S3GraphStore)
    store._client = client_mock
    store._bucket = bucket
    return store


def _make_client_error(code: str) -> Exception:
    import botocore.exceptions

    error_response = {"Error": {"Code": code, "Message": "msg"}}
    return botocore.exceptions.ClientError(error_response, "GetObject")


# ---------------------------------------------------------------------------
# S3GraphStore — round-trip
# ---------------------------------------------------------------------------


def test_put_calls_put_object():
    client = MagicMock()
    store = _make_store(client)
    store.put("org/repo", GOOD_HASH, '{"nodes": []}')
    client.put_object.assert_called_once_with(
        Bucket="test-bucket",
        Key=f"org/repo/{GOOD_HASH}.json",
        Body=b'{"nodes": []}',
        ContentType="application/json",
    )


def test_get_returns_body_when_key_exists():
    client = MagicMock()
    body_mock = MagicMock()
    body_mock.read.return_value = b'{"nodes": []}'
    client.get_object.return_value = {"Body": body_mock}

    store = _make_store(client)
    result = store.get("org/repo", GOOD_HASH)
    assert result == '{"nodes": []}'


def test_get_returns_none_on_no_such_key():
    client = MagicMock()
    client.get_object.side_effect = _make_client_error("NoSuchKey")

    store = _make_store(client)
    assert store.get("org/repo", GOOD_HASH) is None


def test_get_reraises_other_client_errors():
    import botocore.exceptions

    client = MagicMock()
    client.get_object.side_effect = _make_client_error("AccessDenied")

    store = _make_store(client)
    with pytest.raises(botocore.exceptions.ClientError):
        store.get("org/repo", GOOD_HASH)


def test_key_format():
    client = MagicMock()
    body_mock = MagicMock()
    body_mock.read.return_value = b"{}"
    client.get_object.return_value = {"Body": body_mock}

    store = _make_store(client)
    store.get("org/repo", GOOD_HASH)
    client.get_object.assert_called_once_with(
        Bucket="test-bucket", Key=f"org/repo/{GOOD_HASH}.json"
    )


# ---------------------------------------------------------------------------
# S3GraphStore — commit hash validation
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
# build_store() — s3 path
# ---------------------------------------------------------------------------


def test_build_store_s3_missing_bucket(monkeypatch):
    monkeypatch.setenv("SERPENTINE_STORE_BACKEND", "s3")
    monkeypatch.delenv("SERPENTINE_S3_BUCKET", raising=False)
    with pytest.raises(ConfigError, match="SERPENTINE_S3_BUCKET"):
        build_store()


def test_build_store_s3_missing_deps(monkeypatch):
    monkeypatch.setenv("SERPENTINE_STORE_BACKEND", "s3")
    monkeypatch.setenv("SERPENTINE_S3_BUCKET", "my-bucket")
    with patch.dict("sys.modules", {"boto3": None}):
        with pytest.raises((ConfigError, ImportError)):
            build_store()


def test_build_store_s3_ok(monkeypatch):
    monkeypatch.setenv("SERPENTINE_STORE_BACKEND", "s3")
    monkeypatch.setenv("SERPENTINE_S3_BUCKET", "my-bucket")
    monkeypatch.delenv("SERPENTINE_S3_REGION", raising=False)

    mock_client = MagicMock()
    mock_boto3 = MagicMock()
    mock_boto3.client.return_value = mock_client

    with patch.dict(
        "sys.modules",
        {
            "boto3": mock_boto3,
            "botocore": MagicMock(),
            "botocore.exceptions": MagicMock(),
        },
    ):
        store = build_store()

    assert isinstance(store, S3GraphStore)
    assert store._bucket == "my-bucket"


def test_build_store_s3_with_region(monkeypatch):
    monkeypatch.setenv("SERPENTINE_STORE_BACKEND", "s3")
    monkeypatch.setenv("SERPENTINE_S3_BUCKET", "my-bucket")
    monkeypatch.setenv("SERPENTINE_S3_REGION", "us-west-2")

    mock_boto3 = MagicMock()

    with patch.dict(
        "sys.modules",
        {
            "boto3": mock_boto3,
            "botocore": MagicMock(),
            "botocore.exceptions": MagicMock(),
        },
    ):
        store = build_store()

    mock_boto3.client.assert_called_once_with("s3", region_name="us-west-2")
