#!/usr/bin/env python3
# scripts/system_tests/perf_analyze.py
# ============================================================================
# Module: Performance Run Analyzer
# Description: Aggregates perf artifacts and ranks throughput bottlenecks.
# Purpose: Provide deterministic attribution across memory and SQLite perf tracks.
# Dependencies: stdlib
# ============================================================================

from __future__ import annotations

import argparse
import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, List, Optional, TypedDict, cast


class ToolReportRow(TypedDict):
    """Per-tool row for generated performance reports."""

    tool: str
    calls: int
    failed_calls: int
    total_duration_ms: int
    avg_p95_latency_ms: float
    time_share_pct: float


class TrackReport(TypedDict):
    """Structured ranking output for a single perf track."""

    summary_count: int
    total_tool_duration_ms: int
    ranking_by_p95: List[ToolReportRow]
    ranking_by_total_duration: List[ToolReportRow]


class SqliteContentionReport(TypedDict):
    """Aggregated SQLite contention counters."""

    files: int
    op_counts: Dict[str, int]
    db_errors: Dict[str, int]
    total_duration_ms: Dict[str, int]


class PerfReport(TypedDict):
    """Top-level JSON output for mixed-track analysis."""

    summary_count: int
    ranking_by_p95: List[ToolReportRow]
    ranking_by_total_duration: List[ToolReportRow]
    tracks: Dict[str, TrackReport]
    sqlite_contention: SqliteContentionReport


@dataclass
class ToolAggregate:
    """Aggregated tool-level latency and volume metrics."""

    tool: str
    calls: int = 0
    failed_calls: int = 0
    total_duration_ms: int = 0
    p95_samples: List[float] = field(default_factory=lambda: cast(List[float], []))


@dataclass
class SqliteContentionAggregate:
    """Accumulated SQLite contention and operation counters."""

    files: int = 0
    op_counts: Dict[str, int] = field(
        default_factory=lambda: {
            "read": 0,
            "write": 0,
            "register": 0,
            "list": 0,
        }
    )
    db_errors: Dict[str, int] = field(
        default_factory=lambda: {
            "busy": 0,
            "locked": 0,
            "other": 0,
        }
    )
    total_duration_ms: Dict[str, int] = field(
        default_factory=lambda: {
            "read": 0,
            "write": 0,
            "register": 0,
            "list": 0,
        }
    )


def _to_int(value: object) -> int:
    """Best-effort integer conversion for untyped JSON values."""
    if isinstance(value, bool):
        return int(value)
    if isinstance(value, int):
        return value
    if isinstance(value, float):
        return int(value)
    if not isinstance(value, str):
        return 0
    try:
        return int(value)
    except ValueError:
        return 0


def _to_float(value: object) -> float:
    """Best-effort float conversion for untyped JSON values."""
    if isinstance(value, bool):
        return float(value)
    if isinstance(value, int):
        return float(value)
    if isinstance(value, float):
        return value
    if not isinstance(value, str):
        return 0.0
    try:
        return float(value)
    except ValueError:
        return 0.0


def _track_from_summary(summary: Dict[str, object]) -> str:
    """Classify summary into memory/sqlite tracks using deterministic naming."""
    test_name = str(summary.get("test_name", ""))
    if test_name.startswith("perf_sqlite_"):
        return "sqlite"
    return "memory"


def load_perf_summaries(run_root: Path) -> List[Dict[str, object]]:
    """Load all perf_summary.json files beneath a run root."""
    summaries: List[Dict[str, object]] = []
    for path in sorted(run_root.rglob("perf_summary.json")):
        decoded = json.loads(path.read_text(encoding="utf-8"))
        if isinstance(decoded, dict):
            summaries.append(cast(Dict[str, object], decoded))
    if not summaries:
        raise FileNotFoundError(f"no perf_summary.json files found under {run_root}")
    return summaries


def aggregate_tools(
    summaries: List[Dict[str, object]],
    track: Optional[str] = None,
) -> Dict[str, ToolAggregate]:
    """Aggregate tool metrics across summaries, optionally filtered by track."""
    output: Dict[str, ToolAggregate] = {}
    for summary in summaries:
        if track and _track_from_summary(summary) != track:
            continue
        tools_obj = summary.get("tools")
        if not isinstance(tools_obj, dict):
            continue
        tools = cast(Dict[str, object], tools_obj)
        for raw_tool_name, raw_payload in tools.items():
            if not isinstance(raw_payload, dict):
                continue
            payload = cast(Dict[str, object], raw_payload)
            aggregate = output.setdefault(raw_tool_name, ToolAggregate(tool=raw_tool_name))
            aggregate.calls += _to_int(payload.get("calls", 0))
            aggregate.failed_calls += _to_int(payload.get("failed_calls", 0))
            aggregate.total_duration_ms += _to_int(payload.get("total_duration_ms", 0))
            aggregate.p95_samples.append(_to_float(payload.get("p95_latency_ms", 0.0)))
    return output


def render_track_report(
    summaries: List[Dict[str, object]],
    tools: Dict[str, ToolAggregate],
    track: Optional[str] = None,
) -> TrackReport:
    """Build deterministic JSON ranking output for a single track."""
    if track is None:
        track_summaries = summaries
    else:
        track_summaries = [summary for summary in summaries if _track_from_summary(summary) == track]

    total_duration_ms = sum(item.total_duration_ms for item in tools.values())
    report_rows: List[ToolReportRow] = []
    for tool_name in sorted(tools):
        aggregate = tools[tool_name]
        avg_p95 = (
            sum(aggregate.p95_samples) / len(aggregate.p95_samples)
            if aggregate.p95_samples
            else 0.0
        )
        time_share_pct = (
            (aggregate.total_duration_ms / total_duration_ms) * 100.0
            if total_duration_ms > 0
            else 0.0
        )
        report_rows.append(
            {
                "tool": tool_name,
                "calls": aggregate.calls,
                "failed_calls": aggregate.failed_calls,
                "total_duration_ms": aggregate.total_duration_ms,
                "avg_p95_latency_ms": round(avg_p95, 3),
                "time_share_pct": round(time_share_pct, 3),
            }
        )

    by_p95 = sorted(
        report_rows,
        key=lambda row: (
            float(row["avg_p95_latency_ms"]),
            int(row["total_duration_ms"]),
        ),
        reverse=True,
    )
    by_total_duration = sorted(
        report_rows,
        key=lambda row: int(row["total_duration_ms"]),
        reverse=True,
    )

    return {
        "summary_count": len(track_summaries),
        "total_tool_duration_ms": total_duration_ms,
        "ranking_by_p95": by_p95,
        "ranking_by_total_duration": by_total_duration,
    }


def aggregate_sqlite_contention(run_root: Path) -> SqliteContentionAggregate:
    """Aggregate sqlite_contention.json artifacts."""
    aggregate = SqliteContentionAggregate()
    for path in sorted(run_root.rglob("sqlite_contention.json")):
        decoded = json.loads(path.read_text(encoding="utf-8"))
        if not isinstance(decoded, dict):
            continue
        payload = cast(Dict[str, object], decoded)
        aggregate.files += 1

        op_counts = payload.get("op_counts")
        if isinstance(op_counts, dict):
            casted = cast(Dict[str, object], op_counts)
            for key in aggregate.op_counts:
                aggregate.op_counts[key] += _to_int(casted.get(key, 0))

        db_errors = payload.get("db_errors")
        if isinstance(db_errors, dict):
            casted = cast(Dict[str, object], db_errors)
            for key in aggregate.db_errors:
                aggregate.db_errors[key] += _to_int(casted.get(key, 0))

        for key, field_name in [
            ("read", "read_total_duration_ms"),
            ("write", "write_total_duration_ms"),
            ("register", "register_total_duration_ms"),
            ("list", "list_total_duration_ms"),
        ]:
            aggregate.total_duration_ms[key] += _to_int(payload.get(field_name, 0))

    return aggregate


def build_report(run_root: Path, summaries: List[Dict[str, object]]) -> PerfReport:
    """Create mixed-track report with memory/sqlite splits and contention aggregates."""
    all_tools = aggregate_tools(summaries)
    memory_tools = aggregate_tools(summaries, track="memory")
    sqlite_tools = aggregate_tools(summaries, track="sqlite")

    overall = render_track_report(summaries, all_tools)
    memory = render_track_report(summaries, memory_tools, track="memory")
    sqlite = render_track_report(summaries, sqlite_tools, track="sqlite")

    sqlite_contention_agg = aggregate_sqlite_contention(run_root)
    sqlite_contention: SqliteContentionReport = {
        "files": sqlite_contention_agg.files,
        "op_counts": dict(sqlite_contention_agg.op_counts),
        "db_errors": dict(sqlite_contention_agg.db_errors),
        "total_duration_ms": dict(sqlite_contention_agg.total_duration_ms),
    }

    return {
        "summary_count": overall["summary_count"],
        "ranking_by_p95": overall["ranking_by_p95"],
        "ranking_by_total_duration": overall["ranking_by_total_duration"],
        "tracks": {
            "all": overall,
            "memory": memory,
            "sqlite": sqlite,
        },
        "sqlite_contention": sqlite_contention,
    }


def _render_track_table(title: str, report: TrackReport) -> List[str]:
    """Render one track section as markdown."""
    lines = [
        f"## {title}",
        "",
        f"- Summaries analyzed: {report['summary_count']}",
        f"- Total tool duration (ms): {report['total_tool_duration_ms']}",
        "",
        "### Top Tools by p95",
        "",
        "| Tool | Calls | Failed | Avg p95 (ms) | Time Share (%) |",
        "| --- | --- | --- | --- | --- |",
    ]
    for row in report["ranking_by_p95"]:
        lines.append(
            f"| {row['tool']} | {row['calls']} | {row['failed_calls']} "
            f"| {row['avg_p95_latency_ms']} | {row['time_share_pct']} |"
        )

    lines.extend(
        [
            "",
            "### Top Tools by Total Duration",
            "",
            "| Tool | Total Duration (ms) | Calls | Avg p95 (ms) |",
            "| --- | --- | --- | --- |",
        ]
    )
    for row in report["ranking_by_total_duration"]:
        lines.append(
            f"| {row['tool']} | {row['total_duration_ms']} "
            f"| {row['calls']} | {row['avg_p95_latency_ms']} |"
        )
    lines.append("")
    return lines


def render_markdown(report: PerfReport) -> str:
    """Render a markdown report for mixed-track runs."""
    lines = [
        "# Performance Analysis",
        "",
        f"- Summaries analyzed: {report['summary_count']}",
        "",
    ]

    lines.extend(_render_track_table("Overall", report["tracks"]["all"]))
    if report["tracks"]["memory"]["summary_count"] > 0:
        lines.extend(_render_track_table("Memory Track", report["tracks"]["memory"]))
    if report["tracks"]["sqlite"]["summary_count"] > 0:
        lines.extend(_render_track_table("SQLite Track", report["tracks"]["sqlite"]))

    lines.extend(
        [
            "## SQLite Contention",
            "",
            f"- Files analyzed: {report['sqlite_contention']['files']}",
            "",
            "| Counter | Value |",
            "| --- | --- |",
            f"| op.read | {report['sqlite_contention']['op_counts']['read']} |",
            f"| op.write | {report['sqlite_contention']['op_counts']['write']} |",
            f"| op.register | {report['sqlite_contention']['op_counts']['register']} |",
            f"| op.list | {report['sqlite_contention']['op_counts']['list']} |",
            f"| db_error.busy | {report['sqlite_contention']['db_errors']['busy']} |",
            f"| db_error.locked | {report['sqlite_contention']['db_errors']['locked']} |",
            f"| db_error.other | {report['sqlite_contention']['db_errors']['other']} |",
            f"| duration.read_ms | {report['sqlite_contention']['total_duration_ms']['read']} |",
            f"| duration.write_ms | {report['sqlite_contention']['total_duration_ms']['write']} |",
            f"| duration.register_ms | {report['sqlite_contention']['total_duration_ms']['register']} |",
            f"| duration.list_ms | {report['sqlite_contention']['total_duration_ms']['list']} |",
            "",
        ]
    )
    return "\n".join(lines)


def render_sqlite_markdown(report: PerfReport) -> str:
    """Render SQLite-only markdown for mixed run roots."""
    lines = ["# SQLite Performance Analysis", ""]
    lines.extend(_render_track_table("SQLite Track", report["tracks"]["sqlite"]))
    lines.extend(
        [
            "## SQLite Contention",
            "",
            f"- Files analyzed: {report['sqlite_contention']['files']}",
            "",
            "| Counter | Value |",
            "| --- | --- |",
            f"| op.read | {report['sqlite_contention']['op_counts']['read']} |",
            f"| op.write | {report['sqlite_contention']['op_counts']['write']} |",
            f"| op.register | {report['sqlite_contention']['op_counts']['register']} |",
            f"| op.list | {report['sqlite_contention']['op_counts']['list']} |",
            f"| db_error.busy | {report['sqlite_contention']['db_errors']['busy']} |",
            f"| db_error.locked | {report['sqlite_contention']['db_errors']['locked']} |",
            f"| db_error.other | {report['sqlite_contention']['db_errors']['other']} |",
            "",
        ]
    )
    return "\n".join(lines)


def main() -> None:
    """CLI entry point for perf artifact analysis."""
    parser = argparse.ArgumentParser(description="Analyze Decision Gate perf artifacts")
    parser.add_argument("--run-root", required=True, help="Run root containing perf artifacts")
    parser.add_argument("--out-json", help="Path to write JSON report")
    parser.add_argument("--out-md", help="Path to write Markdown report")
    parser.add_argument(
        "--out-sqlite-md",
        help="Path to write SQLite-only Markdown report (default when mixed tracks are present)",
    )
    args = parser.parse_args()

    run_root = Path(args.run_root).resolve()
    summaries = load_perf_summaries(run_root)
    report = build_report(run_root, summaries)

    out_json = Path(args.out_json) if args.out_json else run_root / "perf_analysis.json"
    out_md = Path(args.out_md) if args.out_md else run_root / "perf_analysis.md"
    out_json.write_text(json.dumps(report, indent=2), encoding="utf-8")
    out_md.write_text(render_markdown(report), encoding="utf-8")

    sqlite_summary_count = report["tracks"]["sqlite"]["summary_count"]
    memory_summary_count = report["tracks"]["memory"]["summary_count"]
    if sqlite_summary_count > 0 and memory_summary_count > 0:
        sqlite_md = (
            Path(args.out_sqlite_md)
            if args.out_sqlite_md
            else run_root / "perf_analysis_sqlite.md"
        )
        sqlite_md.write_text(render_sqlite_markdown(report), encoding="utf-8")
        print(f"Wrote {sqlite_md}")

    print(f"Wrote {out_json}")
    print(f"Wrote {out_md}")


if __name__ == "__main__":
    main()
