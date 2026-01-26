<!--
System Tests README
============================================================================
Document: Decision Gate System-Tests README
Description: Usage and structure for the system-tests crate.
Purpose: Document how to run and extend end-to-end tests.
============================================================================
-->

# Decision Gate System-Tests

## Overview
The `system-tests` crate runs end-to-end Decision Gate validation against a real
MCP server. Tests cover the tool surface, runpack verification, evidence policy,
and provider federation. Every test emits auditable artifacts under a per-test
run root.

New in this phase:
- Concurrency and burst-load stress tests for registry writes, paging stability,
  and precheck request storms (`system-tests/tests/stress.rs`).
- Explicit TODOs to add fuzz/property and long-running soak/perf tests.

## Scenario Guardrails (No Hacks)
System-tests must mirror production behavior end-to-end.
- No fail-open logic; assert required behavior explicitly.
- No sleeps for correctness; use readiness probes and explicit polling.
- Use production types and schemas from `decision-gate-core` and `decision-gate-mcp`.
- Reuse helpers in `system-tests/tests/helpers` instead of ad-hoc harness logic.
- Keep tests deterministic; do not rely on wall-clock time.

## Quick Start
```bash
# Run the full system-tests suite
cargo test -p system-tests

# Run a single test
cargo test -p system-tests --test smoke -- --exact smoke_define_start_next_status
```

## Test Runner (Registry Driven)
```bash
python scripts/test_runner.py --priority P0
python scripts/test_runner.py --category runpack
```

## Adding or Updating Tests
When you add, rename, or remove a test:
- Register it in `system-tests/test_registry.toml`.
- Add/update gaps in `system-tests/test_gaps.toml` if coverage is missing.
- Regenerate coverage docs: `python scripts/coverage_report.py generate`.
- Update `system-tests/README.md` and `system-tests/TEST_MATRIX.md` tables if referenced.
- Ensure required artifacts are written (`summary.json`, `summary.md`, `tool_transcript.json`).
- Update `Docs/security/threat_model.md` or note "Threat Model Delta: none".

## Stress Tests
Stress tests are in `system-tests/tests/stress.rs` and are intended to run under
CI timeouts (not load-test infrastructure). They validate concurrency safety
and fail-closed behavior, not throughput SLAs.
Planned (not yet implemented): fuzz/property tests and long-running soak/perf.

## Environment Variables
- `DECISION_GATE_SYSTEM_TEST_RUN_ROOT`: Per-test artifact root (set by runner).
- `DECISION_GATE_SYSTEM_TEST_HTTP_BIND`: Optional bind override (e.g. 127.0.0.1:18080).
- `DECISION_GATE_SYSTEM_TEST_PROVIDER_URL`: Optional external MCP provider URL.
- `DECISION_GATE_SYSTEM_TEST_TIMEOUT_SEC`: Optional timeout override.

## Artifact Contract
Each test writes the following artifacts under the run root:
- `summary.json` (canonical JSON summary)
- `summary.md` (human-readable summary)
- `tool_transcript.json` (JSON-RPC requests and responses)

Runpack tests additionally write:
- `runpack/` (exported artifacts)

## Registry and Gaps
- `system-tests/test_registry.toml` is the authoritative test inventory.
- `system-tests/test_gaps.toml` tracks missing coverage with acceptance criteria.

## References
- `system-tests/AGENTS.md`
- `Docs/roadmap/world_class_implementation_roadmap.md`
- `Docs/testing/decision_gate_test_coverage.md`
- `Docs/testing/test_infrastructure_guide.md`
