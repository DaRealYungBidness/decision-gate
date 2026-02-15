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
    writer: Dict[str, int]


class PerfReport(TypedDict):
    """Top-level JSON output for mixed-track analysis."""

    summary_count: int
    ranking_by_p95: List[ToolReportRow]
    ranking_by_total_duration: List[ToolReportRow]
    tracks: Dict[str, TrackReport]
    sqlite_contention: SqliteContentionReport
    mutation_diagnostics: Dict[str, int]
    registry_scaling: Dict[str, object]
    read_pool_sweep: Dict[str, object]


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
    writer: Dict[str, int] = field(
        default_factory=lambda: {
            "commands_enqueued": 0,
            "commands_rejected": 0,
            "commands_processed": 0,
            "commit_success_count": 0,
            "commit_failure_count": 0,
            "queue_depth_p50": 0,
            "queue_depth_p95": 0,
            "batch_size_p50": 0,
            "batch_size_p95": 0,
            "batch_wait_p50_us": 0,
            "batch_wait_p95_us": 0,
            "batch_commit_p50_us": 0,
            "batch_commit_p95_us": 0,
        }
    )


@dataclass
class MutationDiagnosticsAggregate:
    """Aggregated MCP per-run mutation coordinator diagnostics."""

    files: int = 0
    lock_acquisitions: int = 0
    lock_wait_p95_us_max: int = 0
    queue_depth_p95_max: int = 0
    pending_waiters_max: int = 0
    active_holders_max: int = 0


@dataclass
class RegistryScalingAggregate:
    """Tier-by-tier registry scaling diagnostics from detailed sweep artifacts."""

    files: int = 0
    tiers: List[Dict[str, object]] = field(default_factory=lambda: cast(List[Dict[str, object]], []))
    cliffs: List[Dict[str, object]] = field(
        default_factory=lambda: cast(List[Dict[str, object]], [])
    )
    health_reports: List[Dict[str, object]] = field(
        default_factory=lambda: cast(List[Dict[str, object]], [])
    )


@dataclass
class ReadPoolSweepAggregate:
    """Aggregated read_pool_size sweep diagnostics."""

    files: int = 0
    rows: List[Dict[str, object]] = field(default_factory=lambda: cast(List[Dict[str, object]], []))


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

        writer = payload.get("writer")
        if isinstance(writer, dict):
            casted = cast(Dict[str, object], writer)
            for key in aggregate.writer:
                aggregate.writer[key] += _to_int(casted.get(key, 0))

    return aggregate


def aggregate_mutation_diagnostics(run_root: Path) -> MutationDiagnosticsAggregate:
    """Aggregate mutation_diagnostics.json artifacts from SQLite MCP perf suites."""
    aggregate = MutationDiagnosticsAggregate()
    for path in sorted(run_root.rglob("mutation_diagnostics.json")):
        decoded = json.loads(path.read_text(encoding="utf-8"))
        if not isinstance(decoded, dict):
            continue
        payload = cast(Dict[str, object], decoded)
        aggregate.files += 1
        aggregate.lock_acquisitions += _to_int(payload.get("lock_acquisitions", 0))
        aggregate.lock_wait_p95_us_max = max(
            aggregate.lock_wait_p95_us_max,
            _to_int(payload.get("lock_wait_p95_us", 0)),
        )
        aggregate.queue_depth_p95_max = max(
            aggregate.queue_depth_p95_max,
            _to_int(payload.get("queue_depth_p95", 0)),
        )
        aggregate.pending_waiters_max = max(
            aggregate.pending_waiters_max,
            _to_int(payload.get("pending_waiters", 0)),
        )
        aggregate.active_holders_max = max(
            aggregate.active_holders_max,
            _to_int(payload.get("active_holders", 0)),
        )
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
        "writer": dict(sqlite_contention_agg.writer),
    }

    mutation_agg = aggregate_mutation_diagnostics(run_root)
    registry_scaling = aggregate_registry_scaling(run_root)
    read_pool_sweep = aggregate_read_pool_sweep(run_root)

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
        "mutation_diagnostics": {
            "files": mutation_agg.files,
            "lock_acquisitions": mutation_agg.lock_acquisitions,
            "lock_wait_p95_us_max": mutation_agg.lock_wait_p95_us_max,
            "queue_depth_p95_max": mutation_agg.queue_depth_p95_max,
            "pending_waiters_max": mutation_agg.pending_waiters_max,
            "active_holders_max": mutation_agg.active_holders_max,
        },
        "registry_scaling": {
            "files": registry_scaling.files,
            "tiers": registry_scaling.tiers,
            "cliffs": registry_scaling.cliffs,
            "health_reports": registry_scaling.health_reports,
        },
        "read_pool_sweep": {
            "files": read_pool_sweep.files,
            "rows": read_pool_sweep.rows,
        },
    }


def _suite_name_from_artifact(path: Path) -> str:
    """Resolve suite folder name from an artifact path."""
    parts = list(path.parts)
    if "artifacts" in parts:
        idx = parts.index("artifacts")
        if idx > 0:
            return parts[idx - 1]
    if len(parts) > 1:
        return parts[-2]
    return path.stem


def aggregate_registry_scaling(run_root: Path) -> RegistryScalingAggregate:
    """Aggregate registry tier diagnostics and identify scaling cliffs."""
    aggregate = RegistryScalingAggregate()
    for path in sorted(run_root.rglob("sqlite_sweep_detailed.json")):
        if "registry" not in str(path).lower():
            continue
        decoded = json.loads(path.read_text(encoding="utf-8"))
        if not isinstance(decoded, list):
            continue
        suite = _suite_name_from_artifact(path)
        aggregate.files += 1
        tiers: List[Dict[str, object]] = []
        for row in decoded:
            if not isinstance(row, dict):
                continue
            casted = cast(Dict[str, object], row)
            workers = _to_int(casted.get("workers", 0))
            metrics_obj = casted.get("metrics")
            metrics = cast(Dict[str, object], metrics_obj) if isinstance(metrics_obj, dict) else {}
            tools_obj = casted.get("tools")
            tools = cast(Dict[str, object], tools_obj) if isinstance(tools_obj, dict) else {}
            top_tool = ""
            top_duration = -1
            top_p95_ms = 0.0
            for tool_name, tool_raw in tools.items():
                if not isinstance(tool_raw, dict):
                    continue
                tool_payload = cast(Dict[str, object], tool_raw)
                duration = _to_int(tool_payload.get("total_duration_ms", 0))
                if duration > top_duration:
                    top_tool = tool_name
                    top_duration = duration
                    top_p95_ms = _to_float(tool_payload.get("p95_latency_ms", 0.0))
            tier_row = {
                "suite": suite,
                "workers": workers,
                "throughput_rps": _to_float(metrics.get("throughput_rps", 0.0)),
                "p95_latency_ms": _to_float(metrics.get("p95_latency_ms", 0.0)),
                "error_rate": _to_float(metrics.get("error_rate", 0.0)),
                "top_tool": top_tool,
                "top_tool_total_duration_ms": max(top_duration, 0),
                "top_tool_p95_ms": top_p95_ms,
            }
            tiers.append(tier_row)
            aggregate.tiers.append(tier_row)

        tiers.sort(key=lambda row: _to_int(row.get("workers", 0)))
        for idx in range(1, len(tiers)):
            previous = tiers[idx - 1]
            current = tiers[idx]
            prev_throughput = _to_float(previous.get("throughput_rps", 0.0))
            current_throughput = _to_float(current.get("throughput_rps", 0.0))
            prev_p95 = _to_float(previous.get("p95_latency_ms", 0.0))
            current_p95 = _to_float(current.get("p95_latency_ms", 0.0))
            if current_throughput + 1e-9 < prev_throughput:
                aggregate.cliffs.append(
                    {
                        "suite": suite,
                        "from_workers": _to_int(previous.get("workers", 0)),
                        "to_workers": _to_int(current.get("workers", 0)),
                        "throughput_from_rps": round(prev_throughput, 3),
                        "throughput_to_rps": round(current_throughput, 3),
                        "p95_from_ms": round(prev_p95, 3),
                        "p95_to_ms": round(current_p95, 3),
                        "top_tool": str(current.get("top_tool", "")),
                    }
                )

    for path in sorted(run_root.rglob("registry_health.json")):
        decoded = json.loads(path.read_text(encoding="utf-8"))
        if not isinstance(decoded, dict):
            continue
        payload = cast(Dict[str, object], decoded)
        payload["suite"] = _suite_name_from_artifact(path)
        aggregate.health_reports.append(payload)
    return aggregate


def aggregate_read_pool_sweep(run_root: Path) -> ReadPoolSweepAggregate:
    """Aggregate read_pool_size sweep rows."""
    aggregate = ReadPoolSweepAggregate()
    for path in sorted(run_root.rglob("sqlite_read_pool_sweep.json")):
        decoded = json.loads(path.read_text(encoding="utf-8"))
        if not isinstance(decoded, list):
            continue
        aggregate.files += 1
        suite = _suite_name_from_artifact(path)
        for row in decoded:
            if not isinstance(row, dict):
                continue
            casted = cast(Dict[str, object], row)
            aggregate.rows.append(
                {
                    "suite": suite,
                    "read_pool_size": _to_int(casted.get("read_pool_size", 0)),
                    "workers": _to_int(casted.get("workers", 0)),
                    "throughput_rps": round(_to_float(casted.get("throughput_rps", 0.0)), 3),
                    "p95_latency_ms": round(_to_float(casted.get("p95_latency_ms", 0.0)), 3),
                    "error_rate": round(_to_float(casted.get("error_rate", 0.0)), 6),
                }
            )
    aggregate.rows.sort(
        key=lambda row: (
            str(row.get("suite", "")),
            _to_int(row.get("read_pool_size", 0)),
            _to_int(row.get("workers", 0)),
        )
    )
    return aggregate


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
            f"| writer.commands_enqueued | {report['sqlite_contention']['writer']['commands_enqueued']} |",
            f"| writer.commands_rejected | {report['sqlite_contention']['writer']['commands_rejected']} |",
            f"| writer.commands_processed | {report['sqlite_contention']['writer']['commands_processed']} |",
            f"| writer.commit_success_count | {report['sqlite_contention']['writer']['commit_success_count']} |",
            f"| writer.commit_failure_count | {report['sqlite_contention']['writer']['commit_failure_count']} |",
            f"| writer.queue_depth_p95 | {report['sqlite_contention']['writer']['queue_depth_p95']} |",
            f"| writer.batch_size_p95 | {report['sqlite_contention']['writer']['batch_size_p95']} |",
            f"| writer.batch_wait_p95_us | {report['sqlite_contention']['writer']['batch_wait_p95_us']} |",
            f"| writer.batch_commit_p95_us | {report['sqlite_contention']['writer']['batch_commit_p95_us']} |",
            "",
        ]
    )
    lines.extend(
        [
            "## Registry Scaling Cliffs",
            "",
            f"- Detailed files analyzed: {report['registry_scaling']['files']}",
            f"- Cliffs detected: {len(cast(List[object], report['registry_scaling']['cliffs']))}",
            "",
            "| Suite | From Workers | To Workers | Throughput From (rps) | Throughput To (rps) | p95 From (ms) | p95 To (ms) | Top Tool at To Tier |",
            "| --- | --- | --- | --- | --- | --- | --- | --- |",
        ]
    )
    for cliff in cast(List[Dict[str, object]], report["registry_scaling"]["cliffs"]):
        lines.append(
            f"| {cliff.get('suite', '')} | {cliff.get('from_workers', 0)} | {cliff.get('to_workers', 0)} "
            f"| {cliff.get('throughput_from_rps', 0)} | {cliff.get('throughput_to_rps', 0)} "
            f"| {cliff.get('p95_from_ms', 0)} | {cliff.get('p95_to_ms', 0)} | {cliff.get('top_tool', '')} |"
        )
    lines.append("")
    lines.extend(
        [
            "## Read Pool Sweep",
            "",
            f"- Files analyzed: {report['read_pool_sweep']['files']}",
            "",
            "| Suite | Read Pool Size | Workers | Throughput (rps) | p95 (ms) | Error Rate |",
            "| --- | --- | --- | --- | --- | --- |",
        ]
    )
    for row in cast(List[Dict[str, object]], report["read_pool_sweep"]["rows"]):
        lines.append(
            f"| {row.get('suite', '')} | {row.get('read_pool_size', 0)} | {row.get('workers', 0)} "
            f"| {row.get('throughput_rps', 0)} | {row.get('p95_latency_ms', 0)} | {row.get('error_rate', 0)} |"
        )
    lines.append("")
    lines.extend(
        [
            "## MCP Mutation Diagnostics",
            "",
            f"- Files analyzed: {report['mutation_diagnostics']['files']}",
            "",
            "| Counter | Value |",
            "| --- | --- |",
            f"| lock_acquisitions | {report['mutation_diagnostics']['lock_acquisitions']} |",
            f"| lock_wait_p95_us_max | {report['mutation_diagnostics']['lock_wait_p95_us_max']} |",
            f"| queue_depth_p95_max | {report['mutation_diagnostics']['queue_depth_p95_max']} |",
            f"| pending_waiters_max | {report['mutation_diagnostics']['pending_waiters_max']} |",
            f"| active_holders_max | {report['mutation_diagnostics']['active_holders_max']} |",
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
            f"| writer.commands_enqueued | {report['sqlite_contention']['writer']['commands_enqueued']} |",
            f"| writer.commands_rejected | {report['sqlite_contention']['writer']['commands_rejected']} |",
            f"| writer.commit_failure_count | {report['sqlite_contention']['writer']['commit_failure_count']} |",
            "",
            "## Registry Scaling Cliffs",
            "",
            f"- Cliffs detected: {len(cast(List[object], report['registry_scaling']['cliffs']))}",
            "",
            "| Suite | From Workers | To Workers | Throughput From (rps) | Throughput To (rps) | Top Tool |",
            "| --- | --- | --- | --- | --- | --- |",
        ]
    )
    for cliff in cast(List[Dict[str, object]], report["registry_scaling"]["cliffs"]):
        lines.append(
            f"| {cliff.get('suite', '')} | {cliff.get('from_workers', 0)} | {cliff.get('to_workers', 0)} "
            f"| {cliff.get('throughput_from_rps', 0)} | {cliff.get('throughput_to_rps', 0)} | {cliff.get('top_tool', '')} |"
        )
    lines.extend(
        [
            "",
            "## MCP Mutation Diagnostics",
            "",
            f"- Files analyzed: {report['mutation_diagnostics']['files']}",
            "",
            "| Counter | Value |",
            "| --- | --- |",
            f"| lock_acquisitions | {report['mutation_diagnostics']['lock_acquisitions']} |",
            f"| lock_wait_p95_us_max | {report['mutation_diagnostics']['lock_wait_p95_us_max']} |",
            f"| queue_depth_p95_max | {report['mutation_diagnostics']['queue_depth_p95_max']} |",
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
