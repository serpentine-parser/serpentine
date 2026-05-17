import json
from serpentine import _analyzer


def analyze_sources(sources: list[tuple[str, str]]) -> list[dict]:
    """
    Run the full analysis pipeline on the given (path, source) pairs.
    Returns edge list with 'imports' edges filtered out — tests assert
    only on the four reference types: calls, has-a, references, is-a.
    """
    fm = _analyzer.FileManager()
    for path, source in sources:
        fm.open_file(path, source)
    graph = json.loads(fm.build_dependency_graph())
    return [e for e in graph.get("edges", []) if e["type"] != "imports"]


def assert_has_edge(edges: list[dict], caller: str, callee: str, edge_type: str) -> None:
    matching = [
        e for e in edges
        if e["caller"] == caller and e["callee"] == callee and e["type"] == edge_type
    ]
    if not matching:
        lines = "\n".join(
            f"  {e['caller']} --{e['type']}--> {e['callee']}" for e in sorted(edges, key=lambda e: e["caller"])
        )
        raise AssertionError(
            f"Missing edge: {caller} --{edge_type}--> {callee}\nActual edges:\n{lines}"
        )


def assert_no_edge(edges: list[dict], caller: str, callee: str, edge_type: str) -> None:
    matching = [
        e for e in edges
        if e["caller"] == caller and e["callee"] == callee and e["type"] == edge_type
    ]
    if matching:
        raise AssertionError(
            f"Unexpected edge: {caller} --{edge_type}--> {callee}"
        )
