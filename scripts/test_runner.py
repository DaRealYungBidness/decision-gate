#!/usr/bin/env python3
# scripts/test_runner.py
# ============================================================================
# Module: System-Test Runner
# Description: Registry-driven runner for Decision Gate system-tests.
# Purpose: Execute tests by category/priority and collect artifacts.
# Dependencies: stdlib, tomllib (or toml)
# ============================================================================

from __future__ import annotations

import argparse
import concurrent.futures
import json
import os
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass
from datetime import datetime
from datetime import timezone
from pathlib import Path
from typing import Any, Dict, List, MutableMapping, Optional

try:
    import tomllib as toml  # Python 3.11+
except ModuleNotFoundError:  # pragma: no cover - fallback
    try:
        import toml  # type: ignore
    except ModuleNotFoundError:
        print("Error: tomllib not available. Install 'toml' with: pip install toml")
        sys.exit(1)

if sys.platform == "win32":
    for stream in (sys.stdout, sys.stderr):
        reconfigure = getattr(stream, "reconfigure", None)
        if callable(reconfigure):
            try:
                reconfigure(encoding="utf-8", errors="replace")
            except Exception:
                pass

RegistryData = MutableMapping[str, Any]


@dataclass
class TestDefinition:
    name: str
    category: str
    priority: str
    run_command: str
    description: Optional[str] = None
    artifacts: Optional[List[str]] = None
    env: Optional[Dict[str, str]] = None
    estimated_runtime_sec: Optional[int] = None


@dataclass
class TestResult:
    name: str
    command: str
    category: Optional[str]
    priority: Optional[str]
    status: str
    start_time: str
    end_time: str
    duration_sec: float
    artifacts: List[str]
    artifact_root: str
    runner_stdout: str
    runner_stderr: str
    return_code: Optional[int] = None
    error: Optional[str] = None


@dataclass
class Manifest:
    generated_at: str
    run_root: str
    summary: Dict[str, Any]
    wall_duration_sec: float
    total_duration_sec: float
    test_results: List[Dict[str, Any]]


def load_registry(path: Path) -> RegistryData:
    if not path.exists():
        raise FileNotFoundError(f"Registry file not found: {path}")
    return toml.loads(path.read_text(encoding="utf-8"))


def parse_tests(registry: RegistryData) -> List[TestDefinition]:
    tests: List[TestDefinition] = []
    for entry in registry.get("tests", []):
        tests.append(
            TestDefinition(
                name=entry["name"],
                category=entry.get("category", "unknown"),
                priority=entry.get("priority", "P2"),
                run_command=entry["run_command"],
                description=entry.get("description"),
                artifacts=list(entry.get("artifacts", [])),
                env=entry.get("env"),
                estimated_runtime_sec=entry.get("estimated_runtime_sec"),
            )
        )
    return tests


def select_tests(
    tests: List[TestDefinition],
    category: Optional[str],
    priority: Optional[str],
    name: Optional[str],
    quick_only: bool,
    registry: RegistryData,
) -> List[TestDefinition]:
    output = tests
    if category:
        output = [t for t in output if t.category == category]
    if priority:
        output = [t for t in output if t.priority == priority]
    if name:
        output = [t for t in output if t.name == name]
    if quick_only:
        quick_categories = {
            k for k, v in registry.get("categories", {}).items() if v.get("quick")
        }
        output = [t for t in output if t.category in quick_categories]
    return output


def sanitize_name(value: str) -> str:
    return "".join(ch if ch.isalnum() or ch in "-_" else "_" for ch in value)


def run_test(
    test: TestDefinition,
    workspace_root: Path,
    run_root: Path,
    timeout_override: Optional[int],
    isolate_target_dir: bool,
    dry_run: bool,
) -> TestResult:
    test_root = run_root / sanitize_name(test.name)
    stdout_path = test_root / "runner.stdout.log"
    stderr_path = test_root / "runner.stderr.log"

    result = TestResult(
        name=test.name,
        command=test.run_command,
        category=test.category,
        priority=test.priority,
        status="skipped" if dry_run else "unknown",
        start_time=datetime.now(timezone.utc).isoformat(),
        end_time=datetime.now(timezone.utc).isoformat(),
        duration_sec=0.0,
        artifacts=test.artifacts or [],
        artifact_root=str(test_root),
        runner_stdout=str(stdout_path),
        runner_stderr=str(stderr_path),
    )

    if dry_run:
        return result

    test_root.mkdir(parents=True, exist_ok=True)

    env = os.environ.copy()
    env["DECISION_GATE_SYSTEM_TEST_RUN_ROOT"] = str(test_root)
    if test.env:
        for key, value in test.env.items():
            env[str(key)] = str(value)

    if isolate_target_dir:
        target_root = Path(tempfile.gettempdir()) / "decision-gate-targets" / sanitize_name(test.name)
        target_root.mkdir(parents=True, exist_ok=True)
        env.setdefault("CARGO_TARGET_DIR", str(target_root))

    start = time.time()
    try:
        with open(stdout_path, "w", encoding="utf-8") as stdout_file, open(
            stderr_path, "w", encoding="utf-8"
        ) as stderr_file:
            process = subprocess.Popen(
                test.run_command,
                shell=True,
                cwd=str(workspace_root),
                stdout=stdout_file,
                stderr=stderr_file,
                env=env,
                text=True,
            )
            default_timeout = (test.estimated_runtime_sec or 300) + 30
            timeout = timeout_override or default_timeout
            process.wait(timeout=timeout)
            return_code = process.returncode
    except subprocess.TimeoutExpired:
        return_code = None
        result.status = "timeout"
        result.error = f"timeout after {timeout_override or test.estimated_runtime_sec or 300}s"
        try:
            process.kill()
        except Exception:
            pass
    except Exception as exc:
        return_code = None
        result.status = "error"
        result.error = str(exc)
    end = time.time()

    if result.status == "unknown":
        if return_code == 0:
            result.status = "passed"
        else:
            result.status = "failed"
    result.return_code = return_code
    result.start_time = datetime.fromtimestamp(start, timezone.utc).isoformat()
    result.end_time = datetime.fromtimestamp(end, timezone.utc).isoformat()
    result.duration_sec = round(end - start, 3)
    return result


def write_manifest(run_root: Path, results: List[TestResult], wall_duration: float) -> None:
    total = len(results)
    passed = len([r for r in results if r.status == "passed"])
    failed = len([r for r in results if r.status == "failed"])
    skipped = len([r for r in results if r.status == "skipped"])
    errors = len([r for r in results if r.status in ("error", "timeout")])
    success_rate = f"{(passed / total * 100):.1f}%" if total else "0%"

    manifest = Manifest(
        generated_at=datetime.now(timezone.utc).isoformat(),
        run_root=str(run_root),
        summary={
            "total_tests": total,
            "passed": passed,
            "failed": failed,
            "skipped": skipped,
            "errors": errors,
            "success_rate": success_rate,
        },
        wall_duration_sec=round(wall_duration, 3),
        total_duration_sec=round(sum(r.duration_sec for r in results), 3),
        test_results=[r.__dict__ for r in results],
    )

    path = run_root / "manifest.json"
    path.write_text(json.dumps(manifest.__dict__, indent=2), encoding="utf-8")


def main() -> None:
    parser = argparse.ArgumentParser(description="Decision Gate system-test runner")
    parser.add_argument("--category", help="Filter by category")
    parser.add_argument("--priority", help="Filter by priority (P0/P1/P2)")
    parser.add_argument("--name", help="Run a single test by name")
    parser.add_argument("--quick", action="store_true", help="Run quick categories only")
    parser.add_argument("--parallel", type=int, default=1, help="Parallel workers")
    parser.add_argument("--timeout", type=int, help="Override timeout seconds")
    parser.add_argument("--dry-run", action="store_true", help="Print commands only")
    parser.add_argument("--isolate-target-dir", action="store_true", help="Isolate target dir per test")
    parser.add_argument("--run-root", help="Override run root directory")
    args = parser.parse_args()

    workspace_root = Path(__file__).resolve().parents[1]
    registry_path = workspace_root / "system-tests" / "test_registry.toml"
    registry = load_registry(registry_path)
    tests = parse_tests(registry)
    selected = select_tests(
        tests,
        args.category,
        args.priority,
        args.name,
        args.quick,
        registry,
    )

    if not selected:
        print("No tests matched filters.")
        sys.exit(1)

    timestamp = datetime.now(timezone.utc).strftime("%Y%m%d_%H%M%S")
    base_root = Path(args.run_root) if args.run_root else workspace_root / ".tmp" / "system-tests" / f"run_{timestamp}"
    base_root.mkdir(parents=True, exist_ok=True)

    start_wall = time.time()

    results: List[TestResult] = []
    if args.parallel <= 1:
        for test in selected:
            results.append(
                run_test(
                    test,
                    workspace_root,
                    base_root,
                    args.timeout,
                    args.isolate_target_dir,
                    args.dry_run,
                )
            )
    else:
        with concurrent.futures.ThreadPoolExecutor(max_workers=args.parallel) as executor:
            futures = [
                executor.submit(
                    run_test,
                    test,
                    workspace_root,
                    base_root,
                    args.timeout,
                    args.isolate_target_dir,
                    args.dry_run,
                )
                for test in selected
            ]
            for future in concurrent.futures.as_completed(futures):
                results.append(future.result())

    end_wall = time.time()
    write_manifest(base_root, results, end_wall - start_wall)

    print(f"Run root: {base_root}")
    for result in results:
        print(f"[{result.status.upper()}] {result.name}")

    failed = [r for r in results if r.status not in ("passed", "skipped")]
    if failed:
        sys.exit(1)


if __name__ == "__main__":
    main()
