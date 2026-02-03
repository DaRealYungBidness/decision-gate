#!/usr/bin/env python3
# scripts/docs/docs_verify.py
# =============================================================================
# Module: Documentation Verifier
# Description: Validate fenced code metadata and run optional doc checks.
# Purpose: Keep Docs/guides runnable and registry-aligned.
# =============================================================================
"""Decision Gate documentation verifier.

This script scans guides in Docs/guides and SDK READMEs, validates fenced-code
metadata, checks the verification registry, and optionally executes runnable
blocks.
It is safe to run in CI without --run; execution only happens when requested.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import shlex
import socket
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, IO, Iterable, List, Mapping, Optional, Sequence, cast

try:
    import tomllib  # Python 3.11+
except ModuleNotFoundError:  # pragma: no cover
    tomllib = None
try:
    import yaml  # type: ignore
except ModuleNotFoundError:  # pragma: no cover
    yaml = None


REPO_ROOT = Path(__file__).resolve().parents[2]
GUIDE_ROOT = REPO_ROOT / "Docs" / "guides"
SDK_READMES = [
    REPO_ROOT / "sdks" / "python" / "README.md",
    REPO_ROOT / "sdks" / "typescript" / "README.md",
]
REGISTRY_PATH = REPO_ROOT / "Docs" / "verification" / "registry.toml"
MCP_READY_PATH = "/readyz"
MCP_STARTUP_TIMEOUT_SEC = 15.0


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


@dataclass
class SessionState:
    """Tracks shared state for doc-runner sessions."""

    path: Path
    server: Optional[subprocess.Popen] = None
    server_stdout: Optional[Path] = None
    server_stderr: Optional[Path] = None
    server_stdout_handle: Optional[IO[str]] = None
    server_stderr_handle: Optional[IO[str]] = None
    endpoint: Optional[str] = None


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
    """Load the documentation verification registry from TOML."""
    if not REGISTRY_PATH.exists():
        return {}
    raw = read_text(REGISTRY_PATH)
    if tomllib is None:
        raise RuntimeError("Python 3.11+ required for toml parsing.")
    data = tomllib.loads(raw)
    entries = data.get("docs", data.get("guides", {}))
    if not isinstance(entries, dict):
        return {}
    return cast(Dict[str, Dict[str, object]], entries)


def doc_key(path: Path) -> str:
    """Return the registry key for a documentation path."""
    return path.relative_to(REPO_ROOT).as_posix()


def ensure_registry(doc_paths: Iterable[Path], registry: Dict[str, Dict[str, object]]) -> List[str]:
    """Confirm every documentation file has a registry entry."""
    errors: List[str] = []
    for path in doc_paths:
        key = doc_key(path)
        if key not in registry:
            errors.append(f"Missing registry entry for doc {path}.")
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
                errors.append(f"{block.guide}:{block.start_line}: dg-expires must be YYYY-MM-DD")
                continue
            if expiry < dt.date.today():
                errors.append(f"{block.guide}:{block.start_line}: dg-skip expired on {expiry}")
    return errors


def guide_has_verified_blocks(blocks: Sequence[Block]) -> bool:
    """Return True when a doc has at least one runnable/parsable block."""
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
            if not node_runtime_available():
                return False
        elif req == "cargo":
            if not shutil_which("cargo"):
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


def run_command(cmd: List[str], cwd: Path, env: Optional[Dict[str, str]] = None) -> None:
    """Run a command and raise with stdout/stderr on failure."""
    result = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, env=env)
    if result.returncode != 0:
        raise RuntimeError(
            f"Command failed: {' '.join(cmd)}\nstdout:\n{result.stdout}\nstderr:\n{result.stderr}"
        )


def execute_blocks(blocks: List[Block], level: str, sessions: Dict[str, SessionState]) -> List[str]:
    """Execute runnable blocks while respecting dg-level and dg-session."""
    errors: List[str] = []

    for block in blocks:
        if "dg-skip" in block.flags:
            continue
        block_level = block.attrs.get("dg-level", "fast")
        if level == "fast" and block_level not in ("fast", "smoke"):
            continue

        reqs = parse_requires(block.attrs.get("dg-requires"))
        if reqs and not requirements_satisfied(reqs):
            continue

        session_state = resolve_session_state(block.attrs.get("dg-session"), sessions)
        session_dir = session_state.path

        cwd = resolve_cwd(block.attrs.get("dg-cwd", "repo"), session_dir)

        env = os.environ.copy()
        if block.attrs.get("dg-server"):
            ensure_server(session_state, block.attrs["dg-server"])
        if session_state.endpoint:
            env["DG_ENDPOINT"] = session_state.endpoint

        try:
            if "dg-parse" in block.flags:
                run_parse(block)
            elif block.attrs.get("dg-validate"):
                run_validate(block, session_dir)
            elif "dg-run" in block.flags:
                run_executable(block, cwd, session_dir, env)
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
    if block.lang in ("yaml", "yml"):
        if yaml is None:
            raise RuntimeError(
                "PyYAML required for yaml parsing. Install with: python3 -m pip install pyyaml"
            )
        list(yaml.safe_load_all(block.content))
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


def run_shell(block: Block, cwd: Path, env: Dict[str, str]) -> None:
    """Execute shell content in a strict bash environment."""
    cmd = ["bash", "-euo", "pipefail", "-c", block.content]
    run_command(cmd, cwd, env=env)


def run_executable(block: Block, cwd: Path, session_dir: Path, env: Dict[str, str]) -> None:
    """Execute a runnable block using the appropriate runner."""
    lang = block.lang
    if lang in ("bash", "sh", "console") or not lang:
        run_shell(block, cwd, env)
        return
    if lang in ("python", "py"):
        run_python(block, session_dir, env, cwd)
        return
    if lang in ("typescript", "ts", "tsx"):
        run_typescript(block, session_dir, env, cwd)
        return
    raise RuntimeError(f"dg-run unsupported for language '{lang}'")


def run_python(block: Block, session_dir: Path, env: Dict[str, str], cwd: Path) -> None:
    """Execute a Python code block."""
    interpreter = resolve_python_runtime()
    env = env.copy()
    env["PYTHONPATH"] = build_pythonpath(env)
    script_path = session_dir / f"doc_block_{block.start_line}.py"
    script_path.write_text(block.content, encoding="utf-8")
    run_command([interpreter, str(script_path)], cwd, env=env)


def run_typescript(block: Block, session_dir: Path, env: Dict[str, str], cwd: Path) -> None:
    """Execute a TypeScript code block with Node."""
    node = resolve_node_runtime()
    env = env.copy()
    loader_path = resolve_ts_loader()
    env["NODE_OPTIONS"] = build_node_options(env.get("NODE_OPTIONS"), loader_path)

    script_root = resolve_script_root(block.attrs.get("dg-script-root"), session_dir)
    handle = tempfile.NamedTemporaryFile(
        mode="w",
        encoding="utf-8",
        suffix=".ts",
        prefix="dg-doc-",
        dir=script_root,
        delete=False,
    )
    script_path = Path(handle.name)
    try:
        handle.write(block.content)
        handle.flush()
        handle.close()
        cmd = [node, "--experimental-strip-types", str(script_path)]
        run_command(cmd, cwd, env=env)
    finally:
        if script_path.exists():
            script_path.unlink()


def resolve_session_state(name: Optional[str], sessions: Dict[str, SessionState]) -> SessionState:
    """Return a SessionState for a named or ephemeral session."""
    if not name:
        return SessionState(path=Path(tempfile.mkdtemp(prefix="dg-doc-")))
    if name not in sessions:
        sessions[name] = SessionState(path=Path(tempfile.mkdtemp(prefix=f"dg-doc-{name}-")))
    return sessions[name]


def resolve_cwd(cwd_mode: str, session_dir: Path) -> Path:
    """Resolve dg-cwd into an absolute path."""
    if cwd_mode == "repo":
        return REPO_ROOT
    if cwd_mode in ("session", "tmp"):
        return session_dir
    path = Path(cwd_mode)
    if path.is_absolute():
        return path
    return (REPO_ROOT / cwd_mode).resolve()


def resolve_script_root(value: Optional[str], session_dir: Path) -> Path:
    """Resolve the directory used for generated runnable scripts."""
    if not value:
        return session_dir
    path = Path(value)
    if path.is_absolute():
        return path
    return (REPO_ROOT / value).resolve()


def build_pythonpath(env: Mapping[str, str]) -> str:
    """Build a PYTHONPATH that includes the SDK sources."""
    paths: List[str] = [str(REPO_ROOT / "sdks" / "python")]
    existing = env.get("PYTHONPATH")
    if existing:
        paths.extend(existing.split(os.pathsep))
    return os.pathsep.join(paths)


def build_node_options(existing: Optional[str], loader_path: Optional[Path]) -> str:
    """Ensure Node options include strict unhandled rejections and a loader."""
    base = existing or ""
    options = base
    if "--unhandled-rejections=strict" not in options:
        options = f"{options} --unhandled-rejections=strict".strip()
    if loader_path and "--experimental-loader" not in options:
        loader_url = loader_path.as_uri()
        options = f"{options} --experimental-loader={loader_url}".strip()
    return options


def resolve_ts_loader() -> Optional[Path]:
    """Return the TypeScript loader shim path when available."""
    loader = REPO_ROOT / "system-tests" / "tests" / "fixtures" / "ts_loader.mjs"
    return loader if loader.exists() else None


def resolve_python_runtime() -> str:
    """Return the Python interpreter to use."""
    for candidate in ("python3", "python"):
        if shutil_which(candidate):
            return candidate
    raise RuntimeError("python runtime not available")


def node_runtime_available() -> bool:
    """Return True when Node supports TypeScript execution."""
    node = shutil_which("node")
    if not node:
        return False
    try:
        output = subprocess.run(
            [
                node,
                "--experimental-strip-types",
                "-e",
                "process.exit(typeof fetch === 'function' ? 0 : 2)",
            ],
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError:
        return False
    return output.returncode == 0


def resolve_node_runtime() -> str:
    """Return the Node interpreter to use."""
    node = shutil_which("node")
    if not node:
        raise RuntimeError("node runtime not available")
    if not node_runtime_available():
        raise RuntimeError("node runtime lacks --experimental-strip-types support")
    return node


def ensure_server(session: SessionState, mode: str) -> None:
    """Ensure a local MCP server is running for the session."""
    if session.server:
        return
    if mode != "mcp":
        raise RuntimeError(f"unsupported dg-server mode '{mode}'")
    (
        session.server,
        session.endpoint,
        session.server_stdout,
        session.server_stderr,
        session.server_stdout_handle,
        session.server_stderr_handle,
    ) = start_mcp_server(session.path)


def start_mcp_server(
    session_dir: Path,
) -> tuple[subprocess.Popen, str, Path, Path, IO[str], IO[str]]:
    """Start a local MCP HTTP server for doc verification."""
    bind_port = allocate_loopback_port()
    config_path = session_dir / "decision-gate-docs.toml"
    write_doc_config(config_path, session_dir, bind_port)

    stdout_path = session_dir / "doc_server.stdout.log"
    stderr_path = session_dir / "doc_server.stderr.log"
    stdout_handle = stdout_path.open("w", encoding="utf-8")
    stderr_handle = stderr_path.open("w", encoding="utf-8")

    cmd = [
        "cargo",
        "run",
        "-p",
        "decision-gate-cli",
        "--",
        "serve",
        "--config",
        str(config_path),
    ]
    process = subprocess.Popen(
        cmd,
        cwd=REPO_ROOT,
        stdout=stdout_handle,
        stderr=stderr_handle,
        text=True,
    )
    endpoint = f"http://127.0.0.1:{bind_port}/rpc"
    try:
        wait_for_ready(bind_port)
    except Exception as exc:
        process.terminate()
        stdout_handle.close()
        stderr_handle.close()
        raise RuntimeError(
            f"mcp server failed to start: {exc}\nstdout:\n{read_tail(stdout_path)}\n"
            f"stderr:\n{read_tail(stderr_path)}"
        ) from exc
    return process, endpoint, stdout_path, stderr_path, stdout_handle, stderr_handle


def wait_for_ready(port: int) -> None:
    """Wait for the MCP server readiness endpoint."""
    deadline = time.time() + MCP_STARTUP_TIMEOUT_SEC
    url = f"http://127.0.0.1:{port}{MCP_READY_PATH}"
    while time.time() < deadline:
        try:
            with urllib.request.urlopen(url, timeout=1) as resp:
                if resp.status == 200:
                    return
        except urllib.error.URLError:
            time.sleep(0.2)
        except TimeoutError:
            time.sleep(0.2)
    raise RuntimeError("readyz timeout")


def allocate_loopback_port() -> int:
    """Allocate a free loopback port."""
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def write_doc_config(config_path: Path, session_dir: Path, port: int) -> None:
    """Write a minimal MCP server config for doc verification."""
    db_path = (session_dir / "decision-gate.db").as_posix()
    registry_path = (session_dir / "decision-gate-registry.db").as_posix()
    config_path.write_text(
        "\n".join(
            [
                "# Decision Gate doc verification config",
                "[server]",
                'transport = "http"',
                f'bind = "127.0.0.1:{port}"',
                'mode = "strict"',
                "",
                "[server.auth]",
                'mode = "local_only"',
                "",
                "[dev]",
                "permissive = true",
                "permissive_warn = true",
                "",
                "[namespace]",
                "allow_default = true",
                "default_tenants = [1]",
                "",
                "[trust]",
                'default_policy = "audit"',
                'min_lane = "verified"',
                "",
                "[evidence]",
                "allow_raw_values = false",
                "require_provider_opt_in = true",
                "",
                "[schema_registry]",
                'type = "sqlite"',
                f'path = "{registry_path}"',
                "",
                "[schema_registry.acl]",
                "allow_local_only = true",
                "require_signing = false",
                "",
                "[run_state_store]",
                'type = "sqlite"',
                f'path = "{db_path}"',
                'journal_mode = "wal"',
                'sync_mode = "full"',
                "busy_timeout_ms = 5000",
                "",
                "[[providers]]",
                'name = "time"',
                'type = "builtin"',
                "",
                "[[providers]]",
                'name = "env"',
                'type = "builtin"',
                "",
                "[[providers]]",
                'name = "json"',
                'type = "builtin"',
                "",
                "[[providers]]",
                'name = "http"',
                'type = "builtin"',
                "",
            ]
        )
        + "\n",
        encoding="utf-8",
    )


def read_tail(path: Path, max_lines: int = 40) -> str:
    """Read the last few lines of a log file."""
    if not path.exists():
        return ""
    lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    return "\n".join(lines[-max_lines:])


def shutdown_sessions(sessions: Dict[str, SessionState]) -> None:
    """Terminate any running doc servers."""
    for session in sessions.values():
        if session.server:
            session.server.terminate()
            try:
                session.server.wait(timeout=5)
            except subprocess.TimeoutExpired:
                session.server.kill()
        if session.server_stdout_handle:
            session.server_stdout_handle.close()
        if session.server_stderr_handle:
            session.server_stderr_handle.close()


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

    doc_paths = list(GUIDE_ROOT.glob("*.md"))
    for readme in SDK_READMES:
        if readme.exists():
            doc_paths.append(readme)
    doc_paths = sorted(doc_paths)
    registry = load_registry()
    test_names = parse_test_registry()

    errors: List[str] = []
    errors.extend(ensure_registry(doc_paths, registry))

    doc_blocks: Dict[Path, List[Block]] = {}
    for path in doc_paths:
        blocks = parse_blocks(path)
        doc_blocks[path] = blocks
        errors.extend(ensure_block_metadata(blocks))

    for path in doc_paths:
        entry = registry.get(doc_key(path), {})
        errors.extend(verify_registry_proofs(path, entry, test_names, doc_blocks[path]))

    if errors:
        for err in errors:
            print(f"ERROR: {err}")
        return 1

    if args.run:
        run_errors: List[str] = []
        sessions: Dict[str, SessionState] = {}
        try:
            for blocks in doc_blocks.values():
                run_errors.extend(execute_blocks(blocks, args.level, sessions))
        finally:
            shutdown_sessions(sessions)
        if run_errors:
            for err in run_errors:
                print(f"ERROR: {err}")
            return 1

    print("Docs verification complete.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
