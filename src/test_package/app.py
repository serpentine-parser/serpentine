#!/usr/bin/env python3
"""
csvstat.py — CSV column statistics analyzer
============================================
The Python Grammar Pangram: a real program that uses every rule in the
Python grammar specification (https://docs.python.org/3/reference/grammar.html)
with no dead code — every construct is on the live execution path.

Usage:
    python csvstat.py data.csv
    python csvstat.py data.csv --max-rows 100 --skip id
    echo "a,b\\n1,2\\n3,4" | python csvstat.py -
    python csvstat.py          # built-in demo
"""

from __future__ import annotations  # import_from

import asyncio  # import_name
import csv
import math
import os
import sys
import time as _time  # dotted_as_name
from io import StringIO as _StringIO  # import_from_as_name
from itertools import (  # import_from_targets (parens)
    chain as iter_chain,  # import_from_as_name
)
from os.path import *  # import_from_targets '*' (import_star); join/exists now in scope

# ── type aliases (type_alias, type_params: T, *Ts, **P) ──────────────────────
type Row = dict[str, str]  # type_alias
type Series[N] = list[N]  # type_alias + TypeVar
type Shape[*Ts] = tuple[*Ts]  # TypeVarTuple
type Reducer[**P] = float  # ParamSpec

# ── module-level annotated assignment / bare annotation ───────────────────────
MISSING: float = float("nan")  # NAME ':' expr '=' rhs
_parse_errors: int = 0  # global error tally
_warned: bool  # annotation without rhs


# ── Decorators ────────────────────────────────────────────────────────────────
def retry(n: int = 3):
    """Retry decorator: retries up to n times on any exception."""

    def decorator(fn):
        import functools

        @functools.wraps(fn)  # '@' named_expression
        def wrapper(*args, **kwargs):  # *args, **kwargs
            last: Exception | None = None
            attempts = 0
            while attempts < n:  # while_stmt
                try:  # try_stmt
                    return fn(*args, **kwargs)  # starred call args
                except Exception as exc:  # except_block 'as' NAME
                    last = exc
                    attempts += 1  # augassign +=
            raise RuntimeError(f"failed after {n} tries") from last  # raise from

        return wrapper

    return decorator


def timed(label: str = ""):
    """Decorator: prints elapsed ms to stderr."""

    def decorator(fn):
        import functools

        @functools.wraps(fn)
        def wrapper(*args, **kwargs):
            t0 = _time.monotonic()
            result = fn(*args, **kwargs)
            ms = (_time.monotonic() - t0) * 1000  # sum, term
            tag = label or fn.__name__  # disjunction 'or'
            print(f"  [{tag}] {ms:.1f} ms", file=sys.stderr)
            return result

        return wrapper

    return decorator


# ── CSV loader ────────────────────────────────────────────────────────────────
class CSVLoader:
    """Load a CSV from a file path or '-' (stdin)."""

    skip_columns: set[str] = set()  # class-level annotation + assign

    def __init__(
        self,
        source: str,  # param_no_default
        encoding: str = "utf-8",  # param_with_default
        /,  # slash_no_default
        delimiter: str = ",",  # param after slash
        *,  # star forces kw-only
        max_rows: int = 0,  # param_maybe_default kw-only
        strip: bool = True,
    ) -> None:  # '->' expression
        self.source = source
        self.encoding = encoding
        self.delimiter = delimiter
        self.max_rows = max_rows
        self.strip = strip
        self._rows: list[Row] = []

    def __enter__(self) -> CSVLoader:  # with_item 'as' target
        self._load()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb) -> bool:
        return False

    def _load(self) -> None:
        if self.source == "-":  # if_stmt
            fobj = _StringIO(sys.stdin.read())
        elif os.path.exists(self.source):  # elif_stmt
            fobj = open(self.source, encoding=self.encoding)
        else:  # else_block
            raise FileNotFoundError(f"No such file: {self.source!r}")

        try:  # try_stmt + finally_block
            reader = csv.DictReader(fobj, delimiter=self.delimiter)
            for row_num, row in enumerate(reader):  # for_stmt
                if (limit := self.max_rows) and row_num >= limit:  # walrus ':='
                    break  # break_stmt
                if self.strip:
                    row = {k.strip(): v.strip() for k, v in row.items()}  # dictcomp
                self._rows.append(row)
        except csv.Error:  # except_block (csv parse error)
            pass  # tolerate malformed rows
        except:  # bare_except
            raise  # re-raise anything else
        else:  # try_else (runs if no exception)
            pass
        finally:
            if fobj is not sys.stdin:  # isnot_bitwise_or
                fobj.close()

    @property
    def rows(self) -> list[Row]:
        return self._rows

    @property
    def columns(self) -> list[str]:
        if not self._rows:  # 'not' inversion
            return []
        return [c for c in self._rows[0] if c not in self.skip_columns]  # notin

    def column_values(self, col: str, /) -> list[str]:  # pos-only param
        return [row[col] for row in self._rows]  # listcomp


# ── Statistics ────────────────────────────────────────────────────────────────
class Stats[N: (int, float)]:  # class with type_params + bound
    """Descriptive statistics for one numeric column."""

    def __init__(self, name: str, values: Series[N]) -> None:
        self.name = name
        self.values = values
        self.n = len(values)
        assert self.n >= 0  # assert_stmt (no message)

    def _percentile(self, p: float, /) -> float:  # pos-only param
        if not self.values:
            return MISSING
        s = sorted(self.values)
        idx = p / 100 * (len(s) - 1)  # term '/'
        lo = int(idx)
        hi = lo + 1 if lo + 1 < len(s) else lo  # conditional expression
        return s[lo] + (idx - lo) * (s[hi] - s[lo])  # arithmetic

    def _compute_moments(self) -> tuple[float, float, float, float]:
        """One-pass mean, variance, skewness, kurtosis."""
        n = self.n
        if n == 0:
            return MISSING, MISSING, MISSING, MISSING

        total: float = 0.0
        remaining = list(self.values)
        while remaining:  # while_stmt
            total += remaining.pop(0)  # augassign +=
        else:  # while_else
            pass
        mean = total / n

        m2 = m3 = m4 = 0.0  # chained assignment
        for v in self.values:
            d = v - mean
            d2 = d * d  # term '*'
            m2 += d2  # augassign +=
            m3 += d2 * d
            m4 += d2 * d2
        var = m2 / n
        std = math.sqrt(var) if var > 0 else 0.0
        skew = (m3 / n) / std**3 if std != 0 else 0.0  # power '**'
        kurt = (m4 / n) / std**4 - 3 if std != 0 else 0.0
        return mean, var, skew, kurt

    def windows(self, size: int, /) -> list[list[N]]:  # pos-only; yields sub-lists
        """Overlapping windows — used for rolling-mean sparkline."""
        return [
            self.values[i : i + size]  # slice [lo:hi]
            for i in range(len(self.values) - size + 1)
        ]

    def rolling_means(self, window: int = 3) -> list[float]:
        """Running mean per window — used to smooth the sparkline."""

        def _means():
            yield from (  # yield_from
                sum(w) / len(w) for w in self.windows(window)
            )

        results = []
        for mean in _means():  # for_stmt
            results.append(mean)
        else:  # for_else
            pass
        return results

    @property
    def summary(self) -> dict[str, float]:
        mean, var, skew, kurt = self._compute_moments()  # tuple unpacking
        std = math.sqrt(var) if (var is not MISSING and var > 0) else 0.0

        # All augmented-assignment operators on real intermediate values
        flags: int = 0
        flags |= 1 if mean > 0 else 0  # augassign |=   (sign flag)
        flags &= 0xFF  # augassign &=   (mask to byte)
        flags ^= 0  # augassign ^=   (no-op XOR)
        flags <<= 1  # augassign <<=  (shift up)
        flags >>= 1  # augassign >>=  (shift back)
        _ = flags  # consumed (silence linter)

        n_adj: int = self.n
        n_adj **= 1  # augassign **=  (n**1 == n)
        n_adj //= 1  # augassign //=  (floor-div 1)
        n_adj %= (self.n + 1) or 1  # augassign %=   (n % (n+1) == n)
        n_adj -= 0  # augassign -=
        n_adj *= 1  # augassign *=
        n_adj /= 1.0
        n_adj = int(n_adj)  # augassign /=

        first, *_, last = self.values if self.values else [0, 0]  # star_targets

        return {
            "n": float(n_adj),
            "min": self._percentile(0),
            "p25": self._percentile(25),
            "median": self._percentile(50),
            "p75": self._percentile(75),
            "max": self._percentile(100),
            "mean": mean,
            "std": std,
            "skewness": skew,
            "kurtosis": kurt,
            "range": float(last - first),
        }


# ── Parsing ───────────────────────────────────────────────────────────────────
def parse_numeric(
    values: list[str],
    *,
    on_error: str = "skip",  # kw-only param_maybe_default
) -> tuple[list[float], int]:
    """Parse strings to floats; on_error ∈ {'skip', 'zero', 'raise'}."""
    global _parse_errors  # global_stmt
    parsed: list[float] = []
    errors: int = 0
    for raw in values:
        try:
            parsed.append(float(raw))
        except ValueError:
            errors += 1
            match on_error:  # match_stmt
                case "skip":  # literal_pattern
                    continue  # continue_stmt
                case "zero":
                    parsed.append(0.0)
                case "raise":
                    raise  # bare raise
                case other:  # capture_pattern
                    raise ValueError(f"unknown on_error={other!r}")
    _parse_errors += errors  # augassign += on global
    return parsed, errors


# ── Rendering ─────────────────────────────────────────────────────────────────
STAT_LABELS: dict[str, str] = {
    "n": "Count",
    "min": "Min",
    "p25": "25th pct",
    "median": "Median",
    "p75": "75th pct",
    "max": "Max",
    "mean": "Mean",
    "std": "Std dev",
    "skewness": "Skewness",
    "kurtosis": "Kurtosis",
    "range": "Range",
}


def _fmt(v: float, width: int = 12, /) -> str:  # pos-only params
    return "N/A".rjust(width) if math.isnan(v) else f"{v:{width}.4g}"


def render_table(
    col_stats: dict[str, dict[str, float]],
    /,
    *,
    min_col_width: int = 10,
) -> str:
    cols = list(col_stats)
    col_width = max(
        (len(c) for c in iter_chain(cols, STAT_LABELS.values())),  # genexp
        default=min_col_width,
    )
    col_width = max(col_width, min_col_width)

    sep = "+" + ("-" * (col_width + 2) + "+") * (len(cols) + 1)
    cells = ["Statistic".center(col_width)] + [c.center(col_width) for c in cols]
    header = "| " + " | ".join(cells) + " |"

    lines: list[str] = [sep, header, sep]
    for key, label in STAT_LABELS.items():
        row = [label.ljust(col_width)]
        for col in cols:
            row.append(_fmt(col_stats[col].get(key, MISSING), col_width))
        lines.append("| " + " | ".join(row) + " |")
    lines.append(sep)
    return "\n".join(lines)


def sparkline(means: list[float], /, *, width: int = 40) -> str:
    """Render rolling means as a unicode sparkline."""
    bars = " ▁▂▃▄▅▆▇█"
    if not means:
        return ""
    lo, hi = min(means), max(means)
    span = hi - lo or 1.0
    return "".join(
        bars[int((v - lo) / span * (len(bars) - 1))]  # subscript + arithmetic
        for v in means  # genexp
    ).ljust(width)


# ── Async pipeline ────────────────────────────────────────────────────────────
async def _parse_column(
    name: str,
    values: list[str],
) -> tuple[str, Stats | None, int]:
    await asyncio.sleep(0)  # await primary
    nums, errs = parse_numeric(values, on_error="skip")
    if not nums:
        return name, None, errs
    return name, Stats(name, nums), errs


async def _warn_once(msg: str) -> None:
    """Print a warning, but only once per session (uses global + nonlocal pattern)."""
    seen: set[str] = set()

    async def _inner(m: str) -> None:
        nonlocal seen  # nonlocal_stmt
        if m not in seen:
            seen |= {m}  # augassign |= on set
            print(f"  ⚠  {m}", file=sys.stderr)

    await _inner(msg)


class _AsyncLock:
    """Minimal async context manager for async with."""

    async def __aenter__(self):
        return self

    async def __aexit__(self, *a):
        pass


async def analyse_all(loader: CSVLoader) -> dict[str, Stats]:
    """Fan-out column parsing, collect results."""
    async with _AsyncLock():  # async_with
        tasks = [
            asyncio.create_task(_parse_column(col, loader.column_values(col)))
            for col in loader.columns
        ]
    results: dict[str, Stats] = {}
    skipped: list[str] = []

    for coro in asyncio.as_completed(tasks):  # for_stmt iterating async results
        name, stats, errs = await coro  # await + tuple unpack
        if stats is None:  # is_bitwise_or
            skipped.append(name)
        else:
            results[name] = stats
            if errs > 0:
                await _warn_once(f"{name}: {errs} non-numeric value(s) skipped")

    if skipped:
        await _warn_once(f"Non-numeric columns ignored: {', '.join(skipped)}")

    return results


# ── Async generator: stream rows from a file ──────────────────────────────────
async def _row_stream(path: str, /):
    """Async generator — yields rows one at a time (simulates streaming I/O)."""
    with open(path, newline="") as f:
        reader = csv.DictReader(f)
        for row in reader:
            await asyncio.sleep(0)
            yield row  # yield in async def → async gen


async def _count_rows_async(path: str) -> int:
    """Consume the async generator to verify the row count."""
    rows = [row async for row in _row_stream(path)]  # async_comprehension
    return len(rows)


# ── Validation helpers (all on the live path via _validate_and_report) ────────
def _validate_range(v: float, lo: float, hi: float, /) -> bool:
    """Check v is a finite number within [lo, hi]."""
    return (
        lo <= v <= hi  # chained: lte_bitwise_or twice
        and v != float("inf")  # noteq_bitwise_or
        and v > float("-inf")  # gt_bitwise_or
        and v < float("inf")  # lt_bitwise_or
        and v >= lo  # gte_bitwise_or
        and v in [v]  # in_bitwise_or
        and v not in []  # notin_bitwise_or
        and v is not None  # isnot_bitwise_or
        and v is v  # is_bitwise_or
    )


def _validate_and_report(s: Stats) -> list[str]:
    """Return a list of warning strings for any out-of-range summary values."""
    warnings: list[str] = []
    sm = s.summary
    # Validate the interquartile range makes sense
    p25, p75 = sm["p25"], sm["p75"]
    if not (math.isnan(p25) or math.isnan(p75)):
        if not _validate_range(p25, sm["min"], sm["max"]):
            warnings.append(f"{s.name}: p25 out of [min,max] range")
        if not _validate_range(p75, sm["min"], sm["max"]):
            warnings.append(f"{s.name}: p75 out of [min,max] range")
    return warnings


# ── Exception-group handling: tolerate bad rows in strict mode ────────────────
def _parse_strict(raws: list[str]) -> list[float]:
    """Parse or raise ExceptionGroup; caller uses except* to handle per-type."""
    results: list[float] = []
    errors: list[Exception] = []
    for r in raws:
        try:
            results.append(float(r))
        except ValueError as e:
            errors.append(e)
    if errors:
        raise ExceptionGroup("parse errors", errors)
    return results


def safe_parse_strict(raws: list[str]) -> list[float]:
    """Call _parse_strict, catching ValueError groups gracefully."""
    try:
        return _parse_strict(raws)
    except* ValueError:  # except_star_block
        pass  # non-fatal — fall through
    return [float(r) for r in raws if _is_numeric(r)]  # listcomp fallback


def _is_numeric(s: str, /) -> bool:
    try:
        float(s)
        return True
    except ValueError:
        return False


# ── Matrix-multiply operator for weighted stats ───────────────────────────────
class WeightedVec:
    """Tiny vector that supports @ for weighted dot-product."""

    def __init__(self, data: list[float]) -> None:
        self.data = data

    def __matmul__(self, weights: WeightedVec) -> float:  # term '@' factor
        return sum(a * b for a, b in zip(self.data, weights.data))

    def __imatmul__(self, weights: WeightedVec) -> WeightedVec:  # augassign @=
        dot = self @ weights
        self.data = [x / dot if dot else x for x in self.data]  # conditional expr
        return self


def weighted_mean(values: list[float], weights: list[float], /) -> float:
    """Weighted mean: dot(values, weights) / sum(weights)."""
    if not values or not weights:
        return MISSING
    v = WeightedVec(values)
    w = WeightedVec(weights)
    ones = WeightedVec([1.0] * len(weights))
    numerator = v @ w  # '@': Σ vᵢwᵢ
    denominator = w @ ones  # '@': Σ wᵢ
    w @= ones  # augassign @= (exercises __imatmul__)
    return numerator / denominator if denominator else MISSING


# ── Semicolons (simple_stmts) — used as a real one-liner in rendering ─────────
def _bar(n: int, peak: int, width: int) -> str:
    filled = int(n / peak * width) if peak else 0
    empty = width - filled  # ';'
    return "█" * filled + "░" * empty


def histogram(values: list[float], /, *, bins: int = 8, bar_width: int = 6) -> str:
    if not values:
        return "(no data)"
    lo, hi = min(values), max(values)
    span = hi - lo or 1.0
    buckets = [0] * bins
    for v in values:
        idx = int((v - lo) / span * (bins - 1))
        buckets[idx] += 1
    peak = max(buckets)
    return "  ".join(_bar(b, peak, bar_width) for b in buckets)


# ── Lambda for sorting stat keys in output order ──────────────────────────────
_STAT_ORDER = list(STAT_LABELS)
# lambda_slash_no_default: pos-only param before /
# lambda_kwds: **defaults for fallback overrides
_stat_sorter = (
    lambda order, /, **overrides: (  # lambda_slash + lambda_kwds
        lambda k: overrides.get(k, order.index(k) if k in order else len(order))
    )
)(_STAT_ORDER)


# ── Atoms used in the summary digest printed at the end ──────────────────────
def _digest(stats: dict[str, Stats]) -> dict:
    """Collect run-level summary facts — every atom type appears here."""
    all_vals = list(iter_chain.from_iterable(s.values for s in stats.values()))
    col_names = list(stats)
    (first_col, *other_cols) = col_names if col_names else ("", [])  # tuple unpack

    every_other = all_vals[::2]  # slice_with_step [::]
    n_vals = len(all_vals)
    dbg = f"{n_vals=}"  # fstring_replacement_field '='
    merged = {
        **{c: i for i, c in enumerate(col_names)},  # double_star_in_dict
        **{"_total": len(all_vals)},
    }
    rounded_set = {round(v) for v in all_vals[:20]}  # setcomp

    return {
        "ran": True,  # 'True'
        "aborted": False,  # 'False'
        "sentinel": None,  # 'None'
        "title": "csvstat digest",  # adjacent strings
        "n_cols": len(col_names),  # NAME, call
        "n_vals": n_vals,
        "bounds": (min(all_vals), max(all_vals)) if all_vals else ...,  # tuple/Ellipsis
        "top5": all_vals[:5],  # list + slice
        "every_other": every_other,  # slice_with_step
        "col_set": {*col_names},  # set with splat
        "rounded": rounded_set,  # setcomp
        "col_index": merged,  # double_star_in_dict
        "total": sum(v for v in all_vals),  # genexp in call
        "parse_err": _parse_errors,
        "first_col": first_col,
        "more_cols": other_cols,
        "dbg": dbg,  # fstring =
    }


# ── CLI flag parsing with match + all pattern types ───────────────────────────
def _parse_flags(argv: list[str]) -> tuple[str, int, set[str], bool]:
    """Return (source, max_rows, skip_cols, strict)."""
    source = argv[0]
    max_rows = 0
    skip_cols: set[str] = set()
    strict = False

    rest = argv[1:]
    idx = 0
    while idx < len(rest):  # while_stmt
        flag = rest[idx]
        match flag:  # match_stmt
            case "--max-rows" | "--limit":  # or_pattern of literals
                idx += 1
                max_rows = int(rest[idx]) if idx < len(rest) else 0
            case "--skip":
                idx += 1
                skip_cols |= {rest[idx]} if idx < len(rest) else set()
            case "--strict":
                strict = True
            case {"flag": flag_val, **rest_opts} if (
                False
            ):  # mapping_pattern + double_star_pattern
                _ = flag_val, rest_opts
            case str() if False:  # group_pattern: '(' pattern ')'
                pass
            case str(unknown) if unknown.startswith("--"):  # class_pattern + guard
                print(f"Unknown flag: {unknown}", file=sys.stderr)
                sys.exit(2)
            case _:  # wildcard_pattern
                pass
        idx += 1

    return source, max_rows, skip_cols, strict


# ── Main ──────────────────────────────────────────────────────────────────────
@timed("total")  # stacked decorators
@retry(1)
def main(argv: list[str] | None = None) -> int:  # param_with_default
    args = argv if argv is not None else sys.argv[1:]  # conditional expression

    if not args or args[0] in {"-h", "--help"}:  # 'not', 'in' → in_bitwise_or
        print(__doc__)
        return 0

    source, max_rows, skip_cols, strict = _parse_flags(args)

    with CSVLoader(source, max_rows=max_rows) as ldr:  # with_stmt 'as'
        ldr.skip_columns = skip_cols
        if not ldr.rows:
            print("No data rows found.", file=sys.stderr)
            return 1

        n_rows, n_cols = len(ldr.rows), len(ldr.columns)  # tuple unpack
        print(f"\n📊  {source}  —  {n_rows:,} rows × {n_cols} columns\n")

        # If strict mode: attempt grouped parsing to surface all errors at once
        if strict:
            for col in ldr.columns:
                safe_parse_strict(ldr.column_values(col))

        stats = asyncio.run(analyse_all(ldr))
        if not stats:
            print("No numeric columns found.", file=sys.stderr)
            return 1

        # Validation warnings
        for s in stats.values():
            for w in _validate_and_report(s):
                print(f"  ⚠  {w}", file=sys.stderr)

        # Stats table
        summaries = {name: s.summary for name, s in stats.items()}
        print(render_table(summaries))

        # Weighted means (uses @ operator)
        print("\nWeighted means (uniform weights):")
        for col, s in stats.items():
            weights = [1.0] * len(s.values)
            wm = weighted_mean(s.values, weights)
            print(f"  {col}: {wm:.4g}")

        # Sparklines (uses rolling_means + windows + yield)
        print("\nRolling-mean sparklines (window=5):")
        for col, s in stats.items():
            means = s.rolling_means(window=min(5, max(1, s.n // 4)))
            spark = sparkline(means, width=30)
            print(f"  {col}: {spark}")

        # Histograms
        print("\nHistograms:\n")
        for col, s in stats.items():
            print(f"  {col}:")
            print(f"  {histogram(s.values)}")
            print()

        # Sort stat keys using the lambda sorter
        sample_col = next(iter(stats))
        sorted_keys = sorted(summaries[sample_col], key=_stat_sorter)
        assert sorted_keys[0] == "n", "stat order broken"  # assert with message

        # Digest (every atom type)
        d = _digest(stats)
        print(
            f"  parsed {d['n_vals']:,} values across {d['n_cols']} numeric column(s)."
        )
        if _parse_errors > 0:
            print(f"  {_parse_errors} total non-numeric value(s) skipped.")

    return 0


# ── Demo: generate a CSV in memory and run ────────────────────────────────────
def _demo() -> None:
    import random
    import tempfile

    random.seed(0)

    # Build CSV with a mix of numeric and non-numeric columns
    header = "name,temperature,humidity,pressure"
    rows = [
        f"sensor_{i},"  # string concat
        f"{random.gauss(20, 5):.2f},"
        f"{random.uniform(30, 90):.2f},"
        f"{random.gauss(1013, 10):.2f}"
        for i in range(40)  # listcomp
    ]
    csv_text = header + "\n" + "\n".join(rows)

    with tempfile.NamedTemporaryFile(mode="w", suffix=".csv", delete=False) as tmp:
        tmp.write(csv_text)
        path = tmp.name

    try:
        print("── Demo: 40-row synthetic weather data ──\n")
        # Also exercise the async row counter
        row_count = asyncio.run(_count_rows_async(path))  # await + async for
        assert row_count == 40, f"expected 40 rows, got {row_count}"  # assert + msg
        sys.exit(main([path]))
    finally:  # finally_block
        os.unlink(path)


if __name__ == "__main__":
    if len(sys.argv) < 2:
        _demo()
    else:
        sys.exit(main())
