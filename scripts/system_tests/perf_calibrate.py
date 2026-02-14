#!/usr/bin/env python3
# scripts/system_tests/perf_calibrate.py
# ============================================================================
# Module: Performance Target Calibrator
# Description: Calibrates absolute SLO thresholds from repeated perf runs.
# Purpose: Produce deterministic local threshold recommendations for perf_targets.toml.
# Dependencies: stdlib, tomllib
# ============================================================================

from __future__ import annotations

import argparse
import json
import math
import os
import subprocess
import sys
from datetime import datetime
from datetime import timezone
from pathlib import Path
from typing import Any, Dict, List, MutableMapping

try:
    import tomllib as toml  # Python 3.11+
except ModuleNotFoundError:  # pragma: no cover
    try:
        import toml  # type: ignore
    except ModuleNotFoundError:
        print("Error: tomllib not available. Install 'toml' with: pip install toml")
        sys.exit(1)

TomlData = MutableMapping[str, Any]


def load_toml(path: Path) -> TomlData:
    """Read and parse TOML from disk."""
    if not path.exists():
        raise FileNotFoundError(f"TOML file not found: {path}")
    return toml.loads(path.read_text(encoding="utf-8"))


def percentile(values: List[float], percentile_value: float) -> float:
    """Return nearest-rank percentile."""
    if not values:
        raise ValueError("cannot compute percentile for empty values")
    if percentile_value <= 0 or percentile_value > 100:
        raise ValueError("percentile must be in range (0, 100]")
    ordered = sorted(values)
    rank = max(1, math.ceil((percentile_value / 100.0) * len(ordered)))
    return ordered[rank - 1]


def run_perf_sample(
    workspace_root: Path,
    test_name: str,
    run_root: Path,
) -> Dict[str, Any]:
    """Run a single performance test sample and return perf_summary.json payload."""
    env = os.environ.copy()
    env["DECISION_GATE_PERF_SKIP_SLO_ASSERTS"] = "1"
    command = [
        sys.executable,
        str(workspace_root / "scripts/system_tests/test_runner.py"),
        "--name",
        test_name,
        "--category",
        "performance",
        "--run-root",
        str(run_root),
    ]
    completed = subprocess.run(
        command,
        cwd=str(workspace_root),
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )
    if completed.returncode != 0:
        raise RuntimeError(
            f"sample run failed for {test_name}: rc={completed.returncode}\n"
            f"stdout:\n{completed.stdout}\n\nstderr:\n{completed.stderr}"
        )
    perf_summary = run_root / test_name / "perf_summary.json"
    if not perf_summary.exists():
        raise FileNotFoundError(f"missing perf summary: {perf_summary}")
    return json.loads(perf_summary.read_text(encoding="utf-8"))


def calibrate_thresholds(samples: List[Dict[str, Any]]) -> Dict[str, Any]:
    """Derive recommended thresholds from repeated samples."""
    throughput = [float(sample["metrics"]["throughput_rps"]) for sample in samples]
    p95 = [float(sample["metrics"]["p95_latency_ms"]) for sample in samples]
    p10_throughput = percentile(throughput, 10.0)
    p90_latency = percentile(p95, 90.0)
    return {
        "min_throughput_rps": math.floor(p10_throughput * 0.85),
        "max_p95_ms": math.ceil(p90_latency * 1.15),
        "max_error_rate": 0.0,
    }


def render_perf_targets(meta: Dict[str, Any], tests: Dict[str, Dict[str, Any]]) -> str:
    """Render perf targets TOML in deterministic key order."""
    lines = [
        "# system-tests/perf_targets.toml",
        "# ============================================================================",
        "# Module: Performance Targets",
        "# Description: Absolute throughput/latency SLOs for system-test perf workflows.",
        "# Purpose: Provide deterministic, auditable pass/fail thresholds for local perf runs.",
        "# ============================================================================",
        "",
        "[meta]",
        f'version = {int(meta.get("version", 1))}',
        f'runner_class = "{meta.get("runner_class", "")}"',
        f'profile = "{meta.get("profile", "release")}"',
        f'notes = "{meta.get("notes", "")}"',
        "",
    ]
    field_order = [
        "description",
        "workers",
        "warmup_iterations",
        "measure_iterations",
        "payload_profile",
        "min_throughput_rps",
        "max_p95_ms",
        "max_error_rate",
    ]
    for test_name in sorted(tests):
        target = tests[test_name]
        lines.append(f"[tests.{test_name}]")
        for field in field_order:
            value = target[field]
            if isinstance(value, str):
                lines.append(f'{field} = "{value}"')
            elif isinstance(value, float):
                lines.append(f"{field} = {value:.1f}")
            else:
                lines.append(f"{field} = {value}")
        lines.append("")
    return "\n".join(lines).rstrip() + "\n"


def main() -> None:
    """CLI entry point for performance threshold calibration."""
    parser = argparse.ArgumentParser(description="Calibrate performance thresholds")
    parser.add_argument("--runs", type=int, default=5, help="Samples per perf test")
    parser.add_argument(
        "--targets-path",
        default="system-tests/perf_targets.toml",
        help="Path to perf targets TOML",
    )
    parser.add_argument(
        "--run-root",
        help="Calibration run root (default: .tmp/system-tests/perf-calibration/<timestamp>)",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print calibrated TOML without writing",
    )
    args = parser.parse_args()

    if args.runs <= 0:
        raise ValueError("--runs must be > 0")

    workspace_root = Path(__file__).resolve().parents[2]
    targets_path = workspace_root / args.targets_path
    targets = load_toml(targets_path)
    test_targets = dict(targets.get("tests", {}))
    if not test_targets:
        raise RuntimeError("no tests found in perf targets")

    timestamp = datetime.now(timezone.utc).strftime("%Y%m%d_%H%M%S")
    base_run_root = (
        Path(args.run_root)
        if args.run_root
        else workspace_root / ".tmp" / "system-tests" / "perf-calibration" / timestamp
    )
    base_run_root.mkdir(parents=True, exist_ok=True)

    recommendations: Dict[str, Dict[str, Any]] = {}
    for test_name in sorted(test_targets):
        samples: List[Dict[str, Any]] = []
        for run_idx in range(args.runs):
            sample_root = base_run_root / test_name / f"sample_{run_idx + 1:02d}"
            sample_root.mkdir(parents=True, exist_ok=True)
            sample = run_perf_sample(workspace_root, test_name, sample_root)
            samples.append(sample)
        recommendations[test_name] = calibrate_thresholds(samples)

    for test_name, recommendation in recommendations.items():
        target = test_targets[test_name]
        target["min_throughput_rps"] = float(recommendation["min_throughput_rps"])
        target["max_p95_ms"] = int(recommendation["max_p95_ms"])
        target["max_error_rate"] = float(recommendation["max_error_rate"])

    rendered = render_perf_targets(dict(targets.get("meta", {})), test_targets)
    if args.dry_run:
        print(rendered)
        return

    targets_path.write_text(rendered, encoding="utf-8")
    print(f"Updated thresholds: {targets_path}")
    print(f"Calibration artifacts: {base_run_root}")


if __name__ == "__main__":
    main()
