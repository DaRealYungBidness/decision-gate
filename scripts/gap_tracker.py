#!/usr/bin/env python3
# scripts/gap_tracker.py
# ============================================================================
# Module: Gap Tracker
# Description: Manage system-test gaps for Decision Gate.
# Purpose: Track missing coverage and generate LLM-ready tasks.
# Dependencies: stdlib, tomllib (or toml), tomli_w (optional for writing)
# ============================================================================

from __future__ import annotations

import argparse
from pathlib import Path
from typing import Any, Dict, List, MutableMapping, Optional, Protocol, cast

try:
    import tomllib  # Python 3.11+
except ModuleNotFoundError:  # pragma: no cover
    tomllib = None

try:
    import tomli_w  # type: ignore[import-not-found]
except ModuleNotFoundError:  # pragma: no cover
    tomli_w = None

try:
    import toml  # type: ignore[import-not-found]
except ModuleNotFoundError:  # pragma: no cover
    toml = None

GapsData = MutableMapping[str, Any]


class TomlLoader(Protocol):
    """Protocol for TOML parsers that expose a loads(text) API."""
    def loads(self, text: str) -> GapsData:
        ...


class TomlDumper(Protocol):
    """Protocol for TOML writers that expose a dumps(data) API."""
    def dumps(self, data: GapsData) -> str:
        ...


def _load_toml(text: str) -> GapsData:
    """Load TOML text using the best available parser."""
    if tomllib is not None:
        return cast(TomlLoader, tomllib).loads(text)
    if toml is not None:
        return cast(TomlLoader, toml).loads(text)
    raise RuntimeError("TOML parser unavailable. Install 'toml' with: pip install toml")


def _dump_toml(data: GapsData) -> str:
    """Serialize gap data to TOML using the best available writer."""
    if tomli_w is not None:
        return cast(TomlDumper, tomli_w).dumps(data)
    if toml is not None:
        return cast(TomlDumper, toml).dumps(data)
    raise RuntimeError("TOML writer unavailable. Install 'toml' with: pip install toml")


def load_gaps(path: Path) -> GapsData:
    """Read the gap TOML file from disk."""
    if not path.exists():
        raise FileNotFoundError(f"Gap file not found: {path}")
    return _load_toml(path.read_text(encoding="utf-8"))


def save_gaps(path: Path, data: GapsData) -> None:
    """Persist gap data to disk in TOML format."""
    text = _dump_toml(data)
    path.write_text(text, encoding="utf-8")


def list_gaps(gaps: List[Dict[str, Any]], priority: Optional[str], status: Optional[str]) -> None:
    """Print a filtered list of gaps to stdout."""
    filtered = gaps
    if priority:
        filtered = [g for g in filtered if g.get("priority") == priority]
    if status:
        filtered = [g for g in filtered if g.get("status") == status]

    if not filtered:
        print("No gaps found.")
        return

    for gap in filtered:
        print(f"[{gap.get('id')}] {gap.get('title')}")
        print(f"  Priority: {gap.get('priority')} | Status: {gap.get('status')} | Category: {gap.get('category')}")
        print(f"  Effort: {gap.get('estimated_effort')}")
        print()


def show_gap(gap: Dict[str, Any]) -> None:
    """Print details for a single gap."""
    print(f"Gap: {gap.get('id')} - {gap.get('title')}")
    print(f"Priority: {gap.get('priority')} | Status: {gap.get('status')} | Category: {gap.get('category')}")
    print(f"Estimated Effort: {gap.get('estimated_effort')}")
    print()

    print("Acceptance Criteria:")
    for item in gap.get("acceptance_criteria", []):
        print(f"- {item}")

    if gap.get("files_to_modify"):
        print("\nFiles to Modify:")
        for file_path in gap.get("files_to_modify", []):
            print(f"- {file_path}")

    if gap.get("dependencies"):
        print("\nDependencies:")
        for dep in gap.get("dependencies", []):
            print(f"- {dep}")


def generate_task_prompt(gap: Dict[str, Any]) -> None:
    """Print an LLM-ready task prompt for a gap entry."""
    lines = [
        f"# Task: {gap.get('title')}",
        "",
        f"**Gap ID:** {gap.get('id')}",
        f"**Priority:** {gap.get('priority')}",
        f"**Category:** {gap.get('category')}",
        f"**Estimated Effort:** {gap.get('estimated_effort')}",
        "",
        "## Acceptance Criteria",
    ]
    for item in gap.get("acceptance_criteria", []):
        lines.append(f"- {item}")
    lines.append("")
    lines.append("## Required Reading")
    lines.append("- system-tests/AGENTS.md")
    lines.append("- system-tests/README.md")
    lines.append("- Docs/standards/codebase_engineering_standards.md")
    lines.append("- Docs/standards/codebase_formatting_standards.md")
    print("\n".join(lines))


def close_gap(gaps: List[Dict[str, Any]], gap_id: str, gaps_path: Path) -> None:
    """Mark a gap as closed and persist changes to disk."""
    for gap in gaps:
        if gap.get("id") == gap_id:
            gap["status"] = "closed"
            save_gaps(gaps_path, {"gaps": gaps})
            print(f"Closed gap {gap_id}")
            return
    print(f"Gap {gap_id} not found")


def main() -> None:
    """CLI entry point for gap tracking workflows."""
    parser = argparse.ArgumentParser(description="Decision Gate gap tracker")
    sub = parser.add_subparsers(dest="command", required=True)

    list_cmd = sub.add_parser("list", help="List gaps")
    list_cmd.add_argument("--priority")
    list_cmd.add_argument("--status")

    show_cmd = sub.add_parser("show", help="Show a gap")
    show_cmd.add_argument("gap_id")

    gen_cmd = sub.add_parser("generate-task", help="Generate task prompt")
    gen_cmd.add_argument("gap_id")

    close_cmd = sub.add_parser("close", help="Close a gap")
    close_cmd.add_argument("gap_id")

    args = parser.parse_args()

    workspace_root = Path(__file__).resolve().parents[1]
    gaps_path = workspace_root / "system-tests" / "test_gaps.toml"
    gaps_data = load_gaps(gaps_path)
    gaps = list(gaps_data.get("gaps", []))

    if args.command == "list":
        list_gaps(gaps, args.priority, args.status)
        return

    target = next((gap for gap in gaps if gap.get("id") == getattr(args, "gap_id", "")), None)
    if args.command == "show":
        if not target:
            print("Gap not found")
            return
        show_gap(target)
        return

    if args.command == "generate-task":
        if not target:
            print("Gap not found")
            return
        generate_task_prompt(target)
        return

    if args.command == "close":
        close_gap(gaps, args.gap_id, gaps_path)
        return


if __name__ == "__main__":
    main()
