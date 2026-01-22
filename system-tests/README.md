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
- `Docs/roadmap/system_tests_world_class_plan.md`
- `Docs/testing/decision_gate_test_coverage.md`
- `Docs/testing/test_infrastructure_guide.md`
