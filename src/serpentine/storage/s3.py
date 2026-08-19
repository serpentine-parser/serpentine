import re
from typing import Any

_COMMIT_HASH_RE = re.compile(r"^[0-9a-f]{40}$")


class S3GraphStore:
    """S3-backed graph store. Key layout: {repo_id}/{commit_hash}.json"""

    def __init__(self, bucket_name: str, region: str | None = None) -> None:
        import boto3  # type: ignore[import-untyped]

        kwargs: dict = {}
        if region:
            kwargs["region_name"] = region
        self._client = boto3.client("s3", **kwargs)
        self._bucket = bucket_name

    def get(self, repo_id: str, commit_hash: str) -> str | None:
        self._validate_commit_hash(commit_hash)
        import botocore.exceptions  # type: ignore[import-untyped]

        key = f"{repo_id}/{commit_hash}.json"
        try:
            response = self._client.get_object(Bucket=self._bucket, Key=key)
            return response["Body"].read().decode("utf-8")
        except botocore.exceptions.ClientError as exc:
            if exc.response["Error"]["Code"] == "NoSuchKey":
                return None
            raise

    def put(self, repo_id: str, commit_hash: str, graph_json: str) -> None:
        self._validate_commit_hash(commit_hash)
        key = f"{repo_id}/{commit_hash}.json"
        self._client.put_object(
            Bucket=self._bucket,
            Key=key,
            Body=graph_json.encode("utf-8"),
            ContentType="application/json",
        )

    def list_ingested(self, repo_id: str) -> list[dict[str, Any]]:
        prefix = f"{repo_id}/"
        result = []
        paginator = self._client.get_paginator("list_objects_v2")
        for page in paginator.paginate(Bucket=self._bucket, Prefix=prefix):
            for obj in page.get("Contents", []):
                key = obj["Key"]
                stem = key[len(prefix):].removesuffix(".json")
                if _COMMIT_HASH_RE.match(stem):
                    result.append({
                        "commit_hash": stem,
                        "ingested_at": obj["LastModified"].isoformat(),
                    })
        return result

    def _validate_commit_hash(self, commit_hash: str) -> None:
        if not _COMMIT_HASH_RE.match(commit_hash):
            raise ValueError(f"Invalid commit hash: {commit_hash!r}")
