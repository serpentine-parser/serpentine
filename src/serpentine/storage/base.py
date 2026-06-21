from typing import Protocol, runtime_checkable


@runtime_checkable
class GraphStore(Protocol):
    def get(self, repo_id: str, commit_hash: str) -> str | None: ...
    def put(self, repo_id: str, commit_hash: str, graph_json: str) -> None: ...
