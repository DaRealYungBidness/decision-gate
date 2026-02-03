#!/usr/bin/env python3
# scripts/adapters/adapter_conformance.py
# =============================================================================
# Module: Decision Gate Adapter Conformance
# Description: Compare adapter tool surfaces with the MCP tool registry.
# Purpose: Ensure adapters expose the same tools as the MCP contract.
# =============================================================================
"""Adapter conformance checks against the MCP tool registry."""

from __future__ import annotations

import argparse
import json
import os
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, List, Optional, Sequence

REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_SPEC = REPO_ROOT / "Docs" / "generated" / "decision-gate" / "tooling.json"


@dataclass
class CheckResult:
    framework: str
    missing: List[str]
    extra: List[str]
    unnamed: List[str]
    duplicates: List[str]


def load_expected_tools(spec_path: Path) -> set[str]:
    if not spec_path.exists():
        raise FileNotFoundError(f"Missing tooling spec: {spec_path}")
    data = json.loads(spec_path.read_text(encoding="utf-8"))
    if not isinstance(data, list):
        raise ValueError("tooling.json must be a list of tool definitions")
    names: List[str] = []
    for entry in data:
        if not isinstance(entry, dict):
            raise ValueError("tooling.json entries must be objects")
        name = entry.get("name")
        if not isinstance(name, str) or not name:
            raise ValueError("tooling.json entry missing tool name")
        if name.startswith("decision_gate_"):
            names.append(name)
        else:
            names.append(f"decision_gate_{name}")
    return set(names)


def tool_name(tool: object) -> Optional[str]:
    for attr in ("name", "tool_name", "__name__"):
        value = getattr(tool, attr, None)
        if isinstance(value, str):
            return value
    return None


def build_tools(framework: str) -> Sequence[object]:
    from decision_gate import DecisionGateClient

    endpoint = os.environ.get("DG_ENDPOINT", "http://127.0.0.1:8080/rpc")
    client = DecisionGateClient(endpoint=endpoint)
    if framework == "langchain":
        from decision_gate_langchain import build_decision_gate_tools

        return build_decision_gate_tools(client)
    if framework == "crewai":
        from decision_gate_crewai import build_decision_gate_tools

        return build_decision_gate_tools(client)
    if framework == "autogen":
        from decision_gate_autogen import build_decision_gate_tools

        return build_decision_gate_tools(client)
    if framework == "openai_agents":
        from decision_gate_openai_agents import build_decision_gate_tools

        return build_decision_gate_tools(client)
    raise ValueError(f"Unknown framework: {framework}")


def check_framework(framework: str, expected: set[str]) -> CheckResult:
    tools = build_tools(framework)
    seen: List[str] = []
    unnamed: List[str] = []
    for tool in tools:
        name = tool_name(tool)
        if not name:
            unnamed.append(repr(tool))
            continue
        seen.append(name)
    actual = set(seen)
    duplicates = sorted({name for name in seen if seen.count(name) > 1})
    missing = sorted(expected - actual)
    extra = sorted(actual - expected)
    return CheckResult(
        framework=framework,
        missing=missing,
        extra=extra,
        unnamed=unnamed,
        duplicates=duplicates,
    )


def parse_frameworks(value: Optional[str]) -> List[str]:
    if not value:
        return ["langchain", "crewai", "autogen", "openai_agents"]
    return [item.strip() for item in value.split(",") if item.strip()]


def format_list(items: Iterable[str]) -> str:
    return "\n  - " + "\n  - ".join(items) if items else ""


def main(argv: Optional[Sequence[str]] = None) -> int:
    parser = argparse.ArgumentParser(description="Validate adapter tool coverage.")
    parser.add_argument(
        "--frameworks",
        help="Comma-separated list of frameworks (langchain,crewai,autogen,openai_agents).",
    )
    parser.add_argument(
        "--spec",
        type=Path,
        default=DEFAULT_SPEC,
        help="Path to tooling.json spec.",
    )
    args = parser.parse_args(argv)

    expected = load_expected_tools(args.spec)
    frameworks = parse_frameworks(args.frameworks)
    failures: List[CheckResult] = []

    for framework in frameworks:
        result = check_framework(framework, expected)
        if result.missing or result.extra or result.unnamed or result.duplicates:
            failures.append(result)

    if failures:
        for result in failures:
            print(f"[adapter-conformance] {result.framework}: mismatch detected")
            if result.missing:
                print(" missing:" + format_list(result.missing))
            if result.extra:
                print(" extra:" + format_list(result.extra))
            if result.unnamed:
                print(" unnamed:" + format_list(result.unnamed))
            if result.duplicates:
                print(" duplicates:" + format_list(result.duplicates))
        return 1

    print("[adapter-conformance] ok: tool surfaces match MCP registry")
    return 0


if __name__ == "__main__":
    sys.exit(main())
