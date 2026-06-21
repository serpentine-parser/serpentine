import os
from pathlib import Path

from serpentine.storage.base import GraphStore


class ConfigError(Exception):
    pass


def build_store() -> GraphStore:
    """Build a GraphStore from environment variables. Fails fast on missing config."""
    backend = os.environ.get("SERPENTINE_STORE_BACKEND")
    if not backend:
        raise ConfigError("SERPENTINE_STORE_BACKEND is required")

    if backend == "local":
        raw = os.environ.get("SERPENTINE_LOCAL_STORE_PATH")
        if not raw:
            raise ConfigError("SERPENTINE_LOCAL_STORE_PATH is required when SERPENTINE_STORE_BACKEND=local")
        from serpentine.storage.local import LocalGraphStore
        return LocalGraphStore(Path(raw))

    if backend == "gcs":
        bucket = os.environ.get("SERPENTINE_GCS_BUCKET")
        if not bucket:
            raise ConfigError("SERPENTINE_GCS_BUCKET is required when SERPENTINE_STORE_BACKEND=gcs")
        try:
            from serpentine.storage.gcs import GcsGraphStore
        except ImportError as exc:
            raise ConfigError("GCS dependencies missing. Install serpentine[gcs].") from exc
        return GcsGraphStore(bucket)

    if backend == "s3":
        bucket = os.environ.get("SERPENTINE_S3_BUCKET")
        if not bucket:
            raise ConfigError("SERPENTINE_S3_BUCKET is required when SERPENTINE_STORE_BACKEND=s3")
        region = os.environ.get("SERPENTINE_S3_REGION")
        try:
            from serpentine.storage.s3 import S3GraphStore
        except ImportError as exc:
            raise ConfigError("S3 dependencies missing. Install serpentine[s3].") from exc
        return S3GraphStore(bucket, region)

    raise ConfigError(f"Unknown SERPENTINE_STORE_BACKEND: {backend!r}. Expected: local, gcs, s3")
