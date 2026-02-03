#!/usr/bin/env python3
# scripts/docs/docs_linkify_refs.py
# =============================================================================
# Module: Docs Linkify References
# Description: Convert [F:...] references into Markdown links.
# Purpose: Keep Docs/ and README.md cross-references clickable.
# =============================================================================
"""Linkify [F:...] cross-references into clickable GitHub-style links.

Authoring convention:
  [F:path/to/file.rs L10-L20]

Rendered form:
  [F:path/to/file.rs L10-L20](path/to/file.rs#L10-L20)

By default the script checks for un-linkified references and exits non-zero
when changes are needed. Use --write to apply changes in place.
"""

from __future__ import annotations

import argparse
import re
from pathlib import Path
from typing import Iterable, List, Tuple

REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_PATHS = [REPO_ROOT / "Docs", REPO_ROOT / "README.md"]

FENCE_RE = re.compile(r"^```")

# Match [F:... L10] or [F:... L10-L20], but not already linkified.
F_REF_RE = re.compile(r"\[F:(?P<path>[^\]\s]+)\s+L(?P<start>\d+)(?:-L?(?P<end>\d+))?\](?!\()")


def find_markdown_files(paths: Iterable[Path]) -> List[Path]:
    files: List[Path] = []
    for path in paths:
        if path.is_dir():
            files.extend(sorted(path.rglob("*.md")))
        elif path.is_file() and path.suffix.lower() == ".md":
            files.append(path)
    return files


def linkify_line(line: str) -> Tuple[str, int]:
    def replace(match: re.Match[str]) -> str:
        rel_path = match.group("path")
        start = match.group("start")
        end = match.group("end")
        anchor = f"#L{start}" if not end else f"#L{start}-L{end}"
        label = match.group(0)
        return f"{label}({rel_path}{anchor})"

    new_line, count = F_REF_RE.subn(replace, line)
    return new_line, count


def linkify_text(text: str) -> Tuple[str, int]:
    lines = text.splitlines(keepends=True)
    in_fence = False
    total = 0
    out_lines: List[str] = []
    for line in lines:
        if FENCE_RE.match(line):
            in_fence = not in_fence
            out_lines.append(line)
            continue
        if in_fence:
            out_lines.append(line)
            continue
        new_line, count = linkify_line(line)
        total += count
        out_lines.append(new_line)
    return "".join(out_lines), total


def process_file(path: Path, write: bool) -> Tuple[int, bool]:
    original = path.read_text(encoding="utf-8")
    updated, count = linkify_text(original)
    changed = updated != original
    if changed and write:
        path.write_text(updated, encoding="utf-8")
    return count, changed


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--write",
        action="store_true",
        help="Apply changes in place.",
    )
    parser.add_argument(
        "paths",
        nargs="*",
        type=Path,
        help="Files or directories to process (default: Docs/ and README.md).",
    )
    args = parser.parse_args()

    paths = args.paths if args.paths else DEFAULT_PATHS
    files = find_markdown_files(paths)
    if not files:
        print("No markdown files found.")
        return 0

    total_refs = 0
    changed_files: List[Path] = []
    for file in files:
        count, changed = process_file(file, write=args.write)
        total_refs += count
        if changed:
            changed_files.append(file)

    if changed_files and not args.write:
        print("Un-linkified [F:...] references found in:")
        for file in changed_files:
            print(f"- {file.relative_to(REPO_ROOT)}")
        return 1

    if args.write:
        print(f"Linkified {total_refs} reference(s) across {len(changed_files)} file(s).")
    else:
        print("All [F:...] references are already linkified.")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
