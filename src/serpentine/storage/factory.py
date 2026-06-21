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
        raise ConfigError("GCS store not yet implemented. Install serpentine[gcs] and use Phase 4.")

    if backend == "s3":
        raise ConfigError("S3 store not yet implemented. Install serpentine[s3] and use Phase 4.")

    raise ConfigError(f"Unknown SERPENTINE_STORE_BACKEND: {backend!r}. Expected: local, gcs, s3")
