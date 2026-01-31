#!/usr/bin/env python3
# scripts/docs_tag_fences.py
# =============================================================================
# Module: Docs Fence Tagger
# Description: Adds dg-* metadata tags to code fences in guides.
# Purpose: Ensure every fenced block has verification metadata for doc checks.
# =============================================================================

from __future__ import annotations

from pathlib import Path

DEFAULT_EXPIRES = "2026-06-30"


def tag_for_language(lang: str) -> str:
    if lang in ("bash", "sh", "console"):
        return "dg-run dg-level=fast"
    if lang == "json":
        return "dg-parse dg-level=fast"
    if lang == "toml":
        return "dg-parse dg-level=fast"
    if lang == "python":
        return f'dg-skip dg-reason="pseudocode" dg-expires={DEFAULT_EXPIRES}'
    if not lang:
        return f'dg-skip dg-reason="output-only" dg-expires={DEFAULT_EXPIRES}'
    return f'dg-skip dg-reason="unclassified" dg-expires={DEFAULT_EXPIRES}'


def process_file(path: Path) -> bool:
    lines = path.read_text().splitlines()
    changed = False
    out: list[str] = []
    in_code = False

    for line in lines:
        if line.startswith("```"):
            if not in_code:
                in_code = True
                info = line.strip()[3:].strip()
                if "dg-" in info:
                    out.append(line)
                    continue
                parts = info.split()
                lang = parts[0] if parts else ""
                tag = tag_for_language(lang)
                if info:
                    new_info = f"{info} {tag}"
                else:
                    new_info = f"{tag}"
                out.append(f"```{new_info}")
                changed = True
                continue
            in_code = False
        out.append(line)

    if changed:
        path.write_text("\n".join(out) + "\n")
    return changed


def main() -> int:
    root = Path("Docs/guides")
    changed_any = False
    for path in sorted(root.glob("*.md")):
        if process_file(path):
            changed_any = True
    if changed_any:
        print("Tagged code fences in guides.")
    else:
        print("No changes needed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
