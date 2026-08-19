import json

from serpentine import _analyzer


def analyze_sources(sources: list[tuple[str, str]]) -> list[dict]:
    """
    Run the full analysis pipeline on the given (path, source) pairs.
    Returns the full edge list from the dependency graph.
    """
    fm = _analyzer.FileManager()
    for path, source in sources:
        fm.open_file(path, source)
    graph = json.loads(fm.build_dependency_graph())
    return graph.get("edges", [])


def assert_has_edge(
    edges: list[dict], caller: str, callee: str, edge_type: str
) -> None:
    matching = [
        e
        for e in edges
        if e["caller"] == caller and e["callee"] == callee and e["type"] == edge_type
    ]
    if not matching:
        lines = "\n".join(
            f"  {e['caller']} --{e['type']}--> {e['callee']}"
            for e in sorted(edges, key=lambda e: e["caller"])
        )
        raise AssertionError(
            f"Missing edge: {caller} --{edge_type}--> {callee}\nActual edges:\n{lines}"
        )


def assert_no_edge(edges: list[dict], caller: str, callee: str, edge_type: str) -> None:
    matching = [
        e
        for e in edges
        if e["caller"] == caller and e["callee"] == callee and e["type"] == edge_type
    ]
    if matching:
        raise AssertionError(f"Unexpected edge: {caller} --{edge_type}--> {callee}")
