import re

_COMMIT_HASH_RE = re.compile(r"^[0-9a-f]{40}$")


class GcsGraphStore:
    """GCS-backed graph store. Key layout: {repo_id}/{commit_hash}.json"""

    def __init__(self, bucket_name: str) -> None:
        from google.cloud import storage as gcs  # type: ignore[import-untyped]

        self._bucket = gcs.Client().bucket(bucket_name)

    def get(self, repo_id: str, commit_hash: str) -> str | None:
        self._validate_commit_hash(commit_hash)
        blob = self._bucket.blob(f"{repo_id}/{commit_hash}.json")
        if not blob.exists():
            return None
        return blob.download_as_text(encoding="utf-8")

    def put(self, repo_id: str, commit_hash: str, graph_json: str) -> None:
        self._validate_commit_hash(commit_hash)
        blob = self._bucket.blob(f"{repo_id}/{commit_hash}.json")
        blob.upload_from_string(graph_json, content_type="application/json")

    def _validate_commit_hash(self, commit_hash: str) -> None:
        if not _COMMIT_HASH_RE.match(commit_hash):
            raise ValueError(f"Invalid commit hash: {commit_hash!r}")
