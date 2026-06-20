"""
Benchmark for graph build and incremental update performance.

Measures the two paths that the incremental caching implementation improves:
  - Full cold build (open_files_bulk + build_dependency_graph)
  - No-op rebuild (build_dependency_graph with no changes)
  - N-file incremental (update_file × N + build_dependency_graph)

Usage:
    uv run python benchmarks/bench_graph.py /path/to/large/repo
    uv run python benchmarks/bench_graph.py /path/to/large/repo --runs 5 --n-files 1,5,20,50
    uv run python benchmarks/bench_graph.py /path/to/large/repo --dry-run
"""

import argparse
import os
import random
import statistics
import sys
import time
import uuid
from pathlib import Path


def find_source_files(repo_path: Path) -> list[Path]:
    from serpentine.config import Config

    config = Config.load(repo_path)
    ext_set = set(config.extensions)
    exclude_dirs = config.exclude_dirs
    files: list[Path] = []

    for dirpath, dirnames, filenames in os.walk(repo_path):
        dirnames[:] = [
            d for d in dirnames
            if d not in exclude_dirs and not d.startswith(".")
        ]
        for filename in filenames:
            if Path(filename).suffix in ext_set:
                files.append(Path(dirpath) / filename)

    return sorted(files)


def read_files(paths: list[Path]) -> list[tuple[str, str]]:
    pairs = []
    for p in paths:
        try:
            pairs.append((str(p), p.read_text(encoding="utf-8", errors="replace")))
        except Exception:
            pass
    return pairs


def _mean_std(times: list[float]) -> tuple[float, float]:
    if len(times) == 1:
        return times[0], 0.0
    return statistics.mean(times), statistics.stdev(times)


def _fmt(mean: float, std: float, width: int = 14) -> str:
    if std == 0.0:
        s = f"{mean:.3f}s"
    else:
        s = f"{mean:.3f} ±{std:.3f}s"
    return s.rjust(width)


def _progress(msg: str) -> None:
    print(msg, flush=True)


def _timed(label: str, fn):
    _progress(f"  {label} …")
    t0 = time.perf_counter()
    result = fn()
    elapsed = time.perf_counter() - t0
    _progress(f"  {label}: {elapsed:.3f}s")
    return result, elapsed


def dry_run(repo_path: Path) -> None:
    from serpentine.config import Config

    config = Config.load(repo_path)
    print(f"\nDry run: {repo_path}")
    print(f"  extensions : {sorted(config.extensions)}")
    print(f"  exclude_dirs: {sorted(config.exclude_dirs)}")
    print()

    print("Discovering source files …")
    source_paths = find_source_files(repo_path)

    # Group by extension for a quick breakdown
    by_ext: dict[str, int] = {}
    for p in source_paths:
        by_ext[p.suffix] = by_ext.get(p.suffix, 0) + 1

    print(f"  total files: {len(source_paths)}")
    for ext, count in sorted(by_ext.items(), key=lambda x: -x[1]):
        print(f"    {ext:6s}  {count}")

    # Top directories by file count — reveals mismatched exclusions or stray caches
    by_dir: dict[Path, int] = {}
    for p in source_paths:
        top = repo_path / p.relative_to(repo_path).parts[0]
        by_dir[top] = by_dir.get(top, 0) + 1
    print()
    print("  top directories:")
    for d, count in sorted(by_dir.items(), key=lambda x: -x[1])[:10]:
        print(f"    {count:5d}  {d.relative_to(repo_path)}")
    print()
    print("Re-run without --dry-run to benchmark.")


def run_benchmark(repo_path: Path, runs: int, n_files_list: list[int]) -> None:
    from serpentine import _analyzer

    print(f"\nDiscovering source files in {repo_path} …", flush=True)
    source_paths = find_source_files(repo_path)
    if not source_paths:
        print("No supported source files found. Check the repo path and extensions.")
        return

    print(f"Reading {len(source_paths)} files into memory …", flush=True)
    file_pairs = read_files(source_paths)
    print(f"Loaded {len(file_pairs)} files.\n", flush=True)

    max_n = max(n_files_list)
    if max_n > len(file_pairs):
        n_files_list = [n for n in n_files_list if n <= len(file_pairs)]
        if not n_files_list:
            n_files_list = [1]
        print(f"  (capped --n-files to {n_files_list} — repo has {len(file_pairs)} files)\n")

    sample_pairs = random.sample(file_pairs, max(n_files_list))
    perturbed = [
        (path, content + f"\n# bench {uuid.uuid4()}\n")
        for path, content in sample_pairs
    ]

    # ── Scenario A: full cold build ───────────────────────────────────────────
    print(f"Scenario A: Full cold build ({runs} run(s))", flush=True)
    t_bulk_list, t_graph_a_list = [], []
    for i in range(runs):
        print(f"  run {i + 1}/{runs}", flush=True)
        fm = _analyzer.FileManager()
        _, t_bulk = _timed("open_files_bulk", lambda: fm.open_files_bulk(file_pairs))
        _, t_graph = _timed("build_dependency_graph", lambda: fm.build_dependency_graph())
        t_bulk_list.append(t_bulk)
        t_graph_a_list.append(t_graph)

    # ── Scenario B: no-op rebuild ─────────────────────────────────────────────
    print(f"\nScenario B: No-op rebuild ({runs} run(s))", flush=True)
    t_noop_list = []
    for i in range(runs):
        print(f"  run {i + 1}/{runs}", flush=True)
        fm = _analyzer.FileManager()
        fm.open_files_bulk(file_pairs)
        fm.build_dependency_graph()
        _, t_noop = _timed("build_dependency_graph (2nd call)", lambda: fm.build_dependency_graph())
        t_noop_list.append(t_noop)

    # ── Scenarios C+: N-file incremental ─────────────────────────────────────
    incremental_results: dict[int, tuple[list[float], list[float]]] = {}
    for n in n_files_list:
        print(f"\nScenario C: {n}-file incremental ({runs} run(s))", flush=True)
        t_upd_list, t_inc_list = [], []
        for i in range(runs):
            print(f"  run {i + 1}/{runs}", flush=True)
            fm = _analyzer.FileManager()
            fm.open_files_bulk(file_pairs)
            fm.build_dependency_graph()

            _, t_upd = _timed(
                f"update_file ×{n}",
                lambda: [
                    fm.update_file(path, content) if _try_update(fm, path, content) else None
                    for path, content in perturbed[:n]
                ],
            )
            _, t_inc = _timed("build_dependency_graph", lambda: fm.build_dependency_graph())
            t_upd_list.append(t_upd)
            t_inc_list.append(t_inc)

        incremental_results[n] = (t_upd_list, t_inc_list)

    # ── Results table ─────────────────────────────────────────────────────────
    run_label = f"{runs} run" + ("s" if runs > 1 else "")
    print(f"\n{'─' * 78}")
    print(f"Benchmark: {repo_path} ({len(file_pairs)} files, {run_label} each)\n")

    col_w = 16
    header = (
        f"{'Scenario':<30}"
        f"{'open_files_bulk':>{col_w}}"
        f"{'build_graph':>{col_w}}"
        f"{'total':>{col_w}}"
    )
    print(header)
    print("─" * len(header))

    bulk_mean, bulk_std = _mean_std(t_bulk_list)
    graph_a_mean, graph_a_std = _mean_std(t_graph_a_list)
    print(
        f"{'Full cold build':<30}"
        f"{_fmt(bulk_mean, bulk_std, col_w)}"
        f"{_fmt(graph_a_mean, graph_a_std, col_w)}"
        f"{'  ' + f'{bulk_mean + graph_a_mean:.3f}s':>{col_w}}"
    )

    noop_mean, noop_std = _mean_std(t_noop_list)
    print(
        f"{'No-op rebuild':<30}"
        f"{'n/a':>{col_w}}"
        f"{_fmt(noop_mean, noop_std, col_w)}"
        f"{'  ' + f'{noop_mean:.3f}s':>{col_w}}"
    )

    for n in n_files_list:
        t_upd_list, t_inc_list = incremental_results[n]
        upd_mean, upd_std = _mean_std(t_upd_list)
        inc_mean, inc_std = _mean_std(t_inc_list)
        print(
            f"{f'{n}-file incremental':<30}"
            f"{_fmt(upd_mean, upd_std, col_w)}"
            f"{_fmt(inc_mean, inc_std, col_w)}"
            f"{'  ' + f'{upd_mean + inc_mean:.3f}s':>{col_w}}"
        )

    print()
    print("Key signals after incremental caching implementation:")
    print("  No-op rebuild → build_graph should drop to ~0ms")
    print("  N-file incremental → build_graph should be proportional to N, not total files")


def _try_update(fm, path: str, content: str) -> bool:
    try:
        fm.update_file(path, content)
    except KeyError:
        fm.open_file(path, content)
    return True


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Benchmark serpentine graph build and incremental update performance"
    )
    parser.add_argument("repo_path", type=Path, nargs="?", default=Path("."),
                        help="Path to the repo to analyze (default: current directory)")
    parser.add_argument(
        "--runs", type=int, default=3, metavar="N",
        help="Timed repetitions per scenario (default: 3)",
    )
    parser.add_argument(
        "--n-files", default="1,5,20", metavar="LIST",
        help="Comma-separated incremental file counts (default: 1,5,20)",
    )
    parser.add_argument(
        "--dry-run", action="store_true",
        help="Print file count and extension breakdown without running the benchmark",
    )
    args = parser.parse_args()

    repo_path = args.repo_path.resolve()

    if args.dry_run:
        dry_run(repo_path)
        return

    n_files_list = sorted({int(x.strip()) for x in args.n_files.split(",") if x.strip()})
    run_benchmark(repo_path, args.runs, n_files_list)


if __name__ == "__main__":
    main()
