<!--
Docs/verification/README.md
============================================================================
Document: Documentation Verification System
Description: How Decision Gate verifies documentation examples.
Purpose: Ensure docs are executable, auditable, and non-drifting.
============================================================================
-->

# Documentation Verification System

Decision Gate treats documentation examples as production surface area. Every
example must be executable, validated, or explicitly exempted with an expiry.

## How It Works

- `scripts/docs/docs_verify.py` parses fenced code blocks in `Docs/guides/*.md`
  and SDK READMEs under `sdks/*/README.md`.
- Each block must include `dg-*` metadata (run/parse/validate/skip).
- The registry (`Docs/verification/registry.toml`) declares proofs per doc
  (keys are repo-relative paths):
  - `system-test`: ties a doc to a system-test in `system-tests/test_registry.toml`.
  - `doc-runner`: requires runnable blocks in the doc itself.

## Required Fence Metadata

Every fenced block in tracked docs must include one of:

- `dg-run` (executed)
- `dg-parse` (parsed; JSON/TOML)
- `dg-validate=<scenario|config>` (validated via CLI)
- `dg-skip` (must include `dg-reason` and `dg-expires`)

Examples:

```toml
```toml dg-parse
[server]
transport = "http"
```
```

```bash
```bash dg-run dg-level=fast
cargo test -p system-tests --features system-tests --test smoke -- --exact smoke::smoke_define_start_next_status
```
```

```python
```python dg-run dg-level=fast dg-server=mcp dg-session=sdk-python dg-requires=python,cargo
import os
from decision_gate import DecisionGateClient
endpoint = os.environ.get("DG_ENDPOINT", "http://127.0.0.1:8080/rpc")
client = DecisionGateClient(endpoint=endpoint, auth_token=None)
print(client.providers_list({}))
```
```

```text
```text dg-skip dg-reason="output-only" dg-expires=2026-06-30
<output omitted>
```
```

## Running Verification

```bash
python3 scripts/docs/docs_verify.py
python3 scripts/docs/docs_verify.py --run --level=fast
```

`--level=all` runs slow/manual blocks when environment requirements are met.

## Execution Helpers

Additional metadata may be used to control execution:

- `dg-session=<name>`: reuse a temporary workspace across related blocks.
- `dg-server=mcp`: start a local MCP HTTP server for the session and set `DG_ENDPOINT`.
- `dg-requires=python,node,cargo`: skip blocks when required runtimes are missing.
- `dg-cwd=<repo-relative|repo|session>`: select the working directory for execution.
- `dg-script-root=<repo-relative>`: place generated scripts under a specific folder.

## Requirements & Exemptions

- Any `dg-skip` must include a reason and expiration date.
- Expired skips fail the build.
- Every tracked doc must have a registry entry and at least one proof.

This is intentionally strict. If a doc cannot be verified, it must say why
and for how long that exemption is acceptable.
