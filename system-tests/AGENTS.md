<!--
System Tests Agent Instructions
============================================================================
Document: Decision Gate System-Tests Agent Guide
Description: Standards for writing and maintaining system-tests.
Purpose: Keep system-test behavior deterministic, auditable, and world-class.
============================================================================
-->

# Decision Gate System-Tests Agent Guide

## Mission
System-tests are the highest-rigor, end-to-end validation layer for Decision Gate.
They must be deterministic, fail-closed, and auditable. Every test must emit
artifacts so results can be inspected by humans and machines.

Scope note:
- Stress tests validate concurrency and fail-closed behavior, not throughput SLAs.
- Fuzz/property tests and long-running soak/perf tests are planned but not yet added.

## Test Contract Standards
- No fail-open logic. If a check is required, assert it explicitly.
- No sleeps for correctness. Use readiness probes and explicit polling.
- Use production types from `decision-gate-core` and `decision-gate-mcp`.
- Record artifacts for every test: `summary.json`, `summary.md`, and
  `tool_transcript.json` at minimum.
- Each test must be listed in `system-tests/test_registry.toml`.
- If a test is incomplete or blocked, register a gap in `system-tests/test_gaps.toml`.

## No Hacks Policy (Real Tests Only)
System-tests must mirror production behavior end-to-end. Tests validate contracts,
not workarounds.

Core rules:
- Never fail-open (no fallbacks, no optional checks for required behavior).
- Never use sleep for correctness (always use readiness/health probes).
- Never accept multiple response shapes to avoid updating producers.
- Always use exact production schemas and types.

Disallowed:
- Sleep-based synchronization that hides missing readiness signals.
- Swallowing errors to keep tests green (e.g., `filter_map(Result::ok)`).
- "Helper-only" flows that bypass real MCP tools or transports.

Allowed:
- Best-effort diagnostics that do not affect pass/fail.
- Explicit readiness/handshake primitives in the harness.
- Versioned schemas with explicit deprecation windows.

## Adding or Updating System Tests
When you add, rename, or remove a test:
- Register it in `system-tests/test_registry.toml`.
- Add/update gaps in `system-tests/test_gaps.toml` if coverage is missing.
- Regenerate coverage docs: `python scripts/coverage_report.py generate`.
- Update `system-tests/README.md` and `system-tests/TEST_MATRIX.md` tables if referenced.
- Keep tests deterministic (no wall-clock time) and emit required artifacts.
- Reuse helpers in `system-tests/tests/helpers` rather than building ad-hoc harnesses.

## Threat Model Delta
If a test changes inputs, trust boundaries, or outputs, update
`Docs/security/threat_model.md` or note "Threat Model Delta: none" in your change summary.

## Artifact Expectations
System-tests write artifacts beneath the run root:
- `DECISION_GATE_SYSTEM_TEST_RUN_ROOT` is the per-test root.
- Use canonical JSON (JCS) for deterministic artifacts.

## Environment Variables
- `DECISION_GATE_SYSTEM_TEST_RUN_ROOT`: Run root for artifacts (set by runner).
- `DECISION_GATE_SYSTEM_TEST_HTTP_BIND`: Optional bind override for MCP server.
- `DECISION_GATE_SYSTEM_TEST_PROVIDER_URL`: Optional external provider URL.
- `DECISION_GATE_SYSTEM_TEST_TIMEOUT_SEC`: Optional timeout override.

## Running Tests
System-tests are feature-gated to avoid running by default in unit-test passes.

```bash
cargo test -p system-tests --features system-tests
cargo nextest run -p system-tests --features system-tests
```

## Formatting and Hygiene
- Format with `cargo +nightly fmt --all` (required).
- Keep test data deterministic. Do not call wall-clock time in tests.
- Use ASCII in documentation unless a file already uses Unicode.

## References
- `Docs/security/threat_model.md`
- `Docs/standards/codebase_engineering_standards.md`
- `Docs/standards/codebase_formatting_standards.md`
