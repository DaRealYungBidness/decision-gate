#!/usr/bin/env python3
"""Decision Gate documentation verifier.

This script scans guides in Docs/guides, validates fenced-code metadata,
checks the verification registry, and optionally executes runnable blocks.
It is safe to run in CI without --run; execution only happens when requested.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import shlex
import subprocess
import tempfile
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, Iterable, List, Mapping, Optional, Sequence, cast

try:
    import tomllib  # Python 3.11+
except ModuleNotFoundError:  # pragma: no cover
    tomllib = None


REPO_ROOT = Path(__file__).resolve().parents[1]
GUIDE_ROOT = REPO_ROOT / "Docs" / "guides"
REGISTRY_PATH = REPO_ROOT / "Docs" / "verification" / "registry.toml"


def new_flags() -> set[str]:
    """Return a new flag set for a Block."""
    return set()


def new_attrs() -> Dict[str, str]:
    """Return a new attribute mapping for a Block."""
    return {}


@dataclass
class Block:
    """Represents a fenced code block parsed from a guide."""

    guide: Path
    start_line: int
    lang: str
    info: str
    content: str
    flags: set[str] = field(default_factory=new_flags)
    attrs: Dict[str, str] = field(default_factory=new_attrs)


def read_text(path: Path) -> str:
    """Read text with a consistent encoding for reproducible parsing."""
    return path.read_text(encoding="utf-8")


def to_str_keyed_dict(value: object) -> Optional[Dict[str, object]]:
    """Return a dict[str, object] when value has only string keys."""
    if not isinstance(value, dict):
        return None
    normalized: Dict[str, object] = {}
    items = cast(Dict[object, object], value)
    for key, entry_value in items.items():
        if not isinstance(key, str):
            return None
        normalized[key] = entry_value
    return normalized


def parse_info(info: str) -> tuple[str, set[str], Dict[str, str]]:
    """Parse a code-fence info string into language, flags, and attributes."""
    tokens = shlex.split(info)
    lang = tokens[0] if tokens else ""
    flags: set[str] = set()
    attrs: Dict[str, str] = {}
    start_index = 1
    if tokens and tokens[0].startswith("dg-"):
        lang = ""
        start_index = 0
    for token in tokens[start_index:]:
        if token.startswith("dg-") and "=" not in token:
            flags.add(token)
            continue
        if token.startswith("dg-") and "=" in token:
            key, value = token.split("=", 1)
            attrs[key] = value
    return lang, flags, attrs


def parse_blocks(path: Path) -> List[Block]:
    """Extract fenced code blocks from a guide file."""
    blocks: List[Block] = []
    lines = read_text(path).splitlines()
    in_code = False
    info = ""
    lang = ""
    flags: set[str] = set()
    attrs: Dict[str, str] = {}
    start_line = 0
    content_lines: List[str] = []

    for idx, line in enumerate(lines, 1):
        if line.startswith("```"):
            if not in_code:
                in_code = True
                start_line = idx
                info = line.strip()[3:].strip()
                lang, flags, attrs = parse_info(info)
                content_lines = []
            else:
                in_code = False
                content = "\n".join(content_lines).rstrip() + "\n" if content_lines else ""
                blocks.append(
                    Block(
                        guide=path,
                        start_line=start_line,
                        lang=lang,
                        info=info,
                        content=content,
                        flags=flags,
                        attrs=attrs,
                    )
                )
        elif in_code:
            content_lines.append(line)

    return blocks


def load_registry() -> Dict[str, Dict[str, object]]:
    """Load the guide verification registry from TOML."""
    if not REGISTRY_PATH.exists():
        return {}
    raw = read_text(REGISTRY_PATH)
    if tomllib is None:
        raise RuntimeError("Python 3.11+ required for toml parsing.")
    data = tomllib.loads(raw)
    guides = data.get("guides", {})
    if not isinstance(guides, dict):
        return {}
    return cast(Dict[str, Dict[str, object]], guides)


def ensure_registry(guide_paths: Iterable[Path], registry: Dict[str, Dict[str, object]]) -> List[str]:
    """Confirm every guide has a registry entry."""
    errors: List[str] = []
    for path in guide_paths:
        key = path.name
        if key not in registry:
            errors.append(f"Missing registry entry for guide {path}.")
    return errors


def ensure_block_metadata(blocks: List[Block]) -> List[str]:
    """Ensure every fenced block declares its execution/validation intent."""
    errors: List[str] = []
    for block in blocks:
        has_action = (
            "dg-run" in block.flags
            or "dg-parse" in block.flags
            or block.attrs.get("dg-validate")
            or "dg-skip" in block.flags
        )
        if not has_action:
            errors.append(
                f"{block.guide}:{block.start_line}: missing dg-run/dg-parse/dg-validate/dg-skip"
            )
            continue
        if "dg-skip" in block.flags:
            reason = block.attrs.get("dg-reason")
            expires = block.attrs.get("dg-expires")
            if not reason or not expires:
                errors.append(
                    f"{block.guide}:{block.start_line}: dg-skip requires dg-reason and dg-expires"
                )
                continue
            try:
                expiry = dt.date.fromisoformat(expires)
            except ValueError:
                errors.append(
                    f"{block.guide}:{block.start_line}: dg-expires must be YYYY-MM-DD"
                )
                continue
            if expiry < dt.date.today():
                errors.append(
                    f"{block.guide}:{block.start_line}: dg-skip expired on {expiry}"
                )
    return errors


def guide_has_verified_blocks(blocks: Sequence[Block]) -> bool:
    """Return True when a guide has at least one runnable/parsable block."""
    for block in blocks:
        if "dg-skip" in block.flags:
            continue
        if "dg-run" in block.flags or "dg-parse" in block.flags or block.attrs.get("dg-validate"):
            return True
    return False


def verify_registry_proofs(
    guide: Path, registry_entry: Mapping[str, object], test_names: set[str], blocks: Sequence[Block]
) -> List[str]:
    """Validate registry proof entries for a given guide."""
    errors: List[str] = []
    proofs_value = registry_entry.get("proofs")
    if not isinstance(proofs_value, list) or not proofs_value:
        errors.append(f"{guide}: registry entry missing proofs.")
        return errors

    has_doc_runner = False
    proofs_list = cast(List[object], proofs_value)
    for proof_value in proofs_list:
        proof_map = to_str_keyed_dict(proof_value)
        if proof_map is None:
            errors.append(f"{guide}: invalid proof entry {proof_value!r}")
            continue
        kind = proof_map.get("kind")
        if kind == "system-test":
            name = proof_map.get("name")
            if not isinstance(name, str) or not name:
                errors.append(f"{guide}: system-test proof missing name.")
            elif name not in test_names:
                errors.append(f"{guide}: system-test proof '{name}' not found in registry.")
        elif kind == "doc-runner":
            has_doc_runner = True
        elif isinstance(kind, str):
            errors.append(f"{guide}: unknown proof kind '{kind}'.")
        else:
            errors.append(f"{guide}: proof kind must be a string (got {kind!r}).")

    if has_doc_runner and not guide_has_verified_blocks(blocks):
        errors.append(f"{guide}: doc-runner proof present but no runnable blocks found.")
    return errors


def parse_test_registry() -> set[str]:
    """Load the system-test registry and return known test names."""
    registry_path = REPO_ROOT / "system-tests" / "test_registry.toml"
    if not registry_path.exists():
        return set()
    if tomllib is None:
        raise RuntimeError("Python 3.11+ required for toml parsing.")
    data = tomllib.loads(read_text(registry_path))
    tests_value = data.get("tests")
    if not isinstance(tests_value, list):
        return set()
    names: set[str] = set()
    tests_list = cast(List[object], tests_value)
    for entry_value in tests_list:
        entry = to_str_keyed_dict(entry_value)
        if entry is None:
            continue
        name = entry.get("name")
        if isinstance(name, str):
            names.add(name)
    return names


def parse_requires(value: Optional[str]) -> set[str]:
    """Parse comma-separated dg-requires values into a set."""
    if not value:
        return set()
    return {item.strip() for item in value.split(",") if item.strip()}


def requirements_satisfied(reqs: set[str]) -> bool:
    """Return True when all runtime requirements are present."""
    for req in reqs:
        if req == "docker":
            if not shutil_which("docker"):
                return False
        elif req == "node":
            if not shutil_which("node"):
                return False
        elif req == "python":
            if not shutil_which("python3") and not shutil_which("python"):
                return False
        else:
            if os.getenv(f"DG_DOC_REQUIRE_{req.upper()}") != "1":
                return False
    return True


def shutil_which(cmd: str) -> Optional[str]:
    """Minimal shutil.which replacement to avoid extra imports."""
    for path in os.getenv("PATH", "").split(os.pathsep):
        candidate = Path(path) / cmd
        if candidate.exists() and os.access(candidate, os.X_OK):
            return str(candidate)
    return None


def run_command(cmd: List[str], cwd: Path) -> None:
    """Run a command and raise with stdout/stderr on failure."""
    result = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True)
    if result.returncode != 0:
        raise RuntimeError(
            f"Command failed: {' '.join(cmd)}\nstdout:\n{result.stdout}\nstderr:\n{result.stderr}"
        )


def execute_blocks(blocks: List[Block], level: str) -> List[str]:
    """Execute runnable blocks while respecting dg-level and dg-session."""
    errors: List[str] = []
    sessions: Dict[str, Path] = {}

    for block in blocks:
        if "dg-skip" in block.flags:
            continue
        block_level = block.attrs.get("dg-level", "fast")
        if level == "fast" and block_level not in ("fast", "smoke"):
            continue

        reqs = parse_requires(block.attrs.get("dg-requires"))
        if reqs and not requirements_satisfied(reqs):
            continue

        session = block.attrs.get("dg-session")
        if session:
            if session not in sessions:
                sessions[session] = Path(tempfile.mkdtemp(prefix=f"dg-doc-{session}-"))
            session_dir = sessions[session]
        else:
            session_dir = Path(tempfile.mkdtemp(prefix="dg-doc-"))

        cwd_mode = block.attrs.get("dg-cwd", "repo")
        cwd = REPO_ROOT if cwd_mode == "repo" else session_dir

        try:
            if "dg-parse" in block.flags:
                run_parse(block)
            elif block.attrs.get("dg-validate"):
                run_validate(block, session_dir)
            elif "dg-run" in block.flags:
                run_shell(block, cwd)
        except Exception as exc:  # pragma: no cover - runtime diagnostics
            errors.append(f"{block.guide}:{block.start_line}: {exc}")

    return errors


def run_parse(block: Block) -> None:
    """Parse structured content without executing it."""
    if block.lang == "json":
        json.loads(block.content)
        return
    if block.lang == "toml":
        if tomllib is None:
            raise RuntimeError("Python 3.11+ required for toml parsing.")
        tomllib.loads(block.content)
        return
    raise RuntimeError(f"dg-parse unsupported for language '{block.lang}'")


def run_validate(block: Block, session_dir: Path) -> None:
    """Validate scenario/config content via decision-gate-cli."""
    kind = block.attrs.get("dg-validate")
    if not kind:
        raise RuntimeError("dg-validate requires a kind")
    if kind == "scenario":
        fmt = block.attrs.get("dg-format", block.lang or "json")
        ext = ".ron" if fmt == "ron" else ".json"
        path = session_dir / f"scenario{ext}"
        path.write_text(block.content, encoding="utf-8")
        cmd = [
            "cargo",
            "run",
            "-p",
            "decision-gate-cli",
            "--",
            "authoring",
            "validate",
            "--input",
            str(path),
            "--format",
            fmt,
        ]
        run_command(cmd, REPO_ROOT)
        return
    if kind == "config":
        path = session_dir / "decision-gate.toml"
        path.write_text(block.content, encoding="utf-8")
        cmd = [
            "cargo",
            "run",
            "-p",
            "decision-gate-cli",
            "--",
            "config",
            "validate",
            "--config",
            str(path),
        ]
        run_command(cmd, REPO_ROOT)
        return
    raise RuntimeError(f"dg-validate unsupported kind '{kind}'")


def run_shell(block: Block, cwd: Path) -> None:
    """Execute shell content in a strict bash environment."""
    cmd = ["bash", "-euo", "pipefail", "-c", block.content]
    run_command(cmd, cwd)


def main() -> int:
    """CLI entrypoint."""
    parser = argparse.ArgumentParser(description="Decision Gate documentation verifier")
    parser.add_argument("--run", action="store_true", help="Execute dg-run/dg-parse blocks")
    parser.add_argument(
        "--level",
        choices=["fast", "all"],
        default="fast",
        help="Execution level for dg-run blocks",
    )
    args = parser.parse_args()

    guide_paths = sorted(GUIDE_ROOT.glob("*.md"))
    registry = load_registry()
    test_names = parse_test_registry()

    errors: List[str] = []
    errors.extend(ensure_registry(guide_paths, registry))

    guide_blocks: Dict[Path, List[Block]] = {}
    for path in guide_paths:
        blocks = parse_blocks(path)
        guide_blocks[path] = blocks
        errors.extend(ensure_block_metadata(blocks))

    for path in guide_paths:
        entry = registry.get(path.name, {})
        errors.extend(verify_registry_proofs(path, entry, test_names, guide_blocks[path]))

    if errors:
        for err in errors:
            print(f"ERROR: {err}")
        return 1

    if args.run:
        run_errors: List[str] = []
        for blocks in guide_blocks.values():
            run_errors.extend(execute_blocks(blocks, args.level))
        if run_errors:
            for err in run_errors:
                print(f"ERROR: {err}")
            return 1

    print("Docs verification complete.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
