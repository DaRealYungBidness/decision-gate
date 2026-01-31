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

- `scripts/docs_verify.py` parses fenced code blocks in `Docs/guides/*.md`.
- Each block must include `dg-*` metadata (run/parse/validate/skip).
- The registry (`Docs/verification/registry.toml`) declares proofs per guide:
  - `system-test`: ties a guide to a system-test in `system-tests/test_registry.toml`.
  - `doc-runner`: requires runnable blocks in the guide itself.

## Required Fence Metadata

Every fenced block in guides must include one of:

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

```text
```text dg-skip dg-reason="output-only" dg-expires=2026-06-30
<output omitted>
```
```

## Running Verification

```bash
python3 scripts/docs_verify.py
python3 scripts/docs_verify.py --run --level=fast
```

`--level=all` runs slow/manual blocks when environment requirements are met.

## Requirements & Exemptions

- Any `dg-skip` must include a reason and expiration date.
- Expired skips fail the build.
- Every guide must have a registry entry and at least one proof.

This is intentionally strict. If a guide can’t be verified, it must say why
and for how long that exemption is acceptable.
