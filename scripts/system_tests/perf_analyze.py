#!/usr/bin/env python3
# scripts/system_tests/perf_analyze.py
# ============================================================================
# Module: Performance Run Analyzer
# Description: Aggregates perf_summary artifacts and ranks bottlenecks.
# Purpose: Provide deterministic attribution by p95 and total time share.
# Dependencies: stdlib
# ============================================================================

from __future__ import annotations

import argparse
import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, List, TypedDict, cast


class ToolReportRow(TypedDict):
    """Per-tool row for the generated performance report."""

    tool: str
    calls: int
    failed_calls: int
    total_duration_ms: int
    avg_p95_latency_ms: float
    time_share_pct: float


class PerfReport(TypedDict):
    """Structured output for both JSON and Markdown rendering."""

    summary_count: int
    total_tool_duration_ms: int
    ranking_by_p95: List[ToolReportRow]
    ranking_by_total_duration: List[ToolReportRow]


@dataclass
class ToolAggregate:
    """Aggregated tool-level latency and volume metrics."""

    tool: str
    calls: int = 0
    failed_calls: int = 0
    total_duration_ms: int = 0
    p95_samples: List[float] = field(default_factory=lambda: cast(List[float], []))


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


def aggregate_tools(summaries: List[Dict[str, object]]) -> Dict[str, ToolAggregate]:
    """Aggregate tool metrics across all perf summaries."""
    output: Dict[str, ToolAggregate] = {}
    for summary in summaries:
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


def render_report(summaries: List[Dict[str, object]], tools: Dict[str, ToolAggregate]) -> PerfReport:
    """Build a deterministic JSON report."""
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
        "summary_count": len(summaries),
        "total_tool_duration_ms": total_duration_ms,
        "ranking_by_p95": by_p95,
        "ranking_by_total_duration": by_total_duration,
    }


def render_markdown(report: PerfReport) -> str:
    """Render a concise Markdown report."""
    by_p95 = report["ranking_by_p95"]
    by_total = report["ranking_by_total_duration"]
    lines = [
        "# Performance Analysis",
        "",
        f"- Summaries analyzed: {report['summary_count']}",
        f"- Total tool duration (ms): {report['total_tool_duration_ms']}",
        "",
        "## Top Tools by p95",
        "",
        "| Tool | Calls | Failed | Avg p95 (ms) | Time Share (%) |",
        "| --- | --- | --- | --- | --- |",
    ]
    for row in by_p95:
        lines.append(
            f"| {row['tool']} | {row['calls']} | {row['failed_calls']} "
            f"| {row['avg_p95_latency_ms']} | {row['time_share_pct']} |"
        )

    lines.extend(
        [
            "",
            "## Top Tools by Total Duration",
            "",
            "| Tool | Total Duration (ms) | Calls | Avg p95 (ms) |",
            "| --- | --- | --- | --- |",
        ]
    )
    for row in by_total:
        lines.append(
            f"| {row['tool']} | {row['total_duration_ms']} "
            f"| {row['calls']} | {row['avg_p95_latency_ms']} |"
        )
    lines.append("")
    return "\n".join(lines)


def main() -> None:
    """CLI entry point for perf artifact analysis."""
    parser = argparse.ArgumentParser(description="Analyze Decision Gate perf artifacts")
    parser.add_argument("--run-root", required=True, help="Run root containing perf artifacts")
    parser.add_argument("--out-json", help="Path to write JSON report")
    parser.add_argument("--out-md", help="Path to write Markdown report")
    args = parser.parse_args()

    run_root = Path(args.run_root).resolve()
    summaries = load_perf_summaries(run_root)
    tools = aggregate_tools(summaries)
    report = render_report(summaries, tools)

    out_json = Path(args.out_json) if args.out_json else run_root / "perf_analysis.json"
    out_md = Path(args.out_md) if args.out_md else run_root / "perf_analysis.md"
    out_json.write_text(json.dumps(report, indent=2), encoding="utf-8")
    out_md.write_text(render_markdown(report), encoding="utf-8")

    print(f"Wrote {out_json}")
    print(f"Wrote {out_md}")


if __name__ == "__main__":
    main()
