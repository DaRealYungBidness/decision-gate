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

## Test Contract Standards
- No fail-open logic. If a check is required, assert it explicitly.
- No sleeps for correctness. Use readiness probes and explicit polling.
- Use production types from `decision-gate-core` and `decision-gate-mcp`.
- Record artifacts for every test: `summary.json`, `summary.md`, and
  `tool_transcript.json` at minimum.
- Each test must be listed in `system-tests/test_registry.toml`.
- If a test is incomplete or blocked, register a gap in `system-tests/test_gaps.toml`.

## Artifact Expectations
System-tests write artifacts beneath the run root:
- `DECISION_GATE_SYSTEM_TEST_RUN_ROOT` is the per-test root.
- Use canonical JSON (JCS) for deterministic artifacts.

## Environment Variables
- `DECISION_GATE_SYSTEM_TEST_RUN_ROOT`: Run root for artifacts (set by runner).
- `DECISION_GATE_SYSTEM_TEST_HTTP_BIND`: Optional bind override for MCP server.
- `DECISION_GATE_SYSTEM_TEST_PROVIDER_URL`: Optional external provider URL.
- `DECISION_GATE_SYSTEM_TEST_TIMEOUT_SEC`: Optional timeout override.

## Formatting and Hygiene
- Format with `cargo +nightly fmt --all` (required).
- Keep test data deterministic. Do not call wall-clock time in tests.
- Use ASCII in documentation unless a file already uses Unicode.

## References
- `Docs/security/threat_model.md`
- `Docs/standards/codebase_engineering_standards.md`
- `Docs/standards/codebase_formatting_standards.md`
