<!--
System Tests README
============================================================================
Document: Decision Gate System-Tests README
Description: Usage and structure for the system-tests crate.
Purpose: Document how to run and extend end-to-end tests.
Dependencies:
  - ./AGENTS.md
  - ./TEST_MATRIX.md
  - ../../Docs/testing/decision_gate_test_coverage.md
============================================================================
-->

# Decision Gate System-Tests

End-to-end tests for Decision Gate. The system-tests crate drives a real MCP
server and validates tool behavior, runpack integrity, evidence policy, and
provider federation.

## Table of Contents

- [Overview](#overview)
- [Quick Start](#quick-start)
- [Suite Layout](#suite-layout)
- [Artifacts](#artifacts)
- [Environment Variables](#environment-variables)
- [Registry and Gaps](#registry-and-gaps)
- [Testing Utilities](#testing-utilities)
- [References](#references)

## Overview

System-tests mirror production behavior end-to-end:

- No fail-open logic; required checks must be asserted.
- Avoid sleeps for correctness; use readiness probes and polling.
- Use production types from `decision-gate-core` and `decision-gate-mcp`.
- Keep tests deterministic (no wall-clock dependencies).

AssetCore integration tests default to local stub servers. To run against a
real AssetCore deployment, update the test config to point at live endpoints.

## Quick Start

```bash
# Run the full system-tests suite (opt-in feature)
cargo test -p system-tests --features system-tests

# Run with nextest (recommended for CI)
cargo nextest run -p system-tests --features system-tests

# Run a single test
cargo test -p system-tests --features system-tests --test smoke -- \
  --exact smoke::smoke_define_start_next_status
```

## Suite Layout

Test entry points live in `system-tests/tests/`. Implementations live in
`system-tests/tests/suites/`.

Entry points:
- `smoke`
- `functional`
- `providers`
- `mcp_transport`
- `sdk_client`
- `sdk_examples`
- `runpack`
- `reliability`
- `security`
- `contract`
- `operations`
- `performance`

To run a specific suite:

```bash
cargo test -p system-tests --features system-tests --test security
```

## Artifacts

Each test writes artifacts under the run root:

- `summary.json` (canonical JSON summary)
- `summary.md` (human-readable summary)
- `tool_transcript.json` (JSON-RPC request/response log)

Runpack tests additionally emit `runpack/` artifacts.

Run roots are treated as immutable evidence directories. Reusing a run root
without explicit opt-in fails closed to avoid silently overwriting diagnostics.

## Environment Variables

- `DECISION_GATE_SYSTEM_TEST_RUN_ROOT`: per-test artifact root.
- `DECISION_GATE_SYSTEM_TEST_HTTP_BIND`: MCP HTTP bind override.
- `DECISION_GATE_SYSTEM_TEST_PROVIDER_URL`: external MCP provider URL.
- `DECISION_GATE_SYSTEM_TEST_TIMEOUT_SEC`: timeout override.
- `DECISION_GATE_SYSTEM_TEST_ALLOW_OVERWRITE`: allow reuse of an existing run root.
- `DECISION_GATE_SYSTEM_TEST_HTTP_STUB_PORT`: force HTTP stub port for deterministic fixtures.
- `DECISION_GATE_SYSTEM_TEST_HTTP_STUB_PORT_BASE`: base port for deterministic HTTP stub allocation.
- `DECISION_GATE_SYSTEM_TEST_HTTP_STUB_PORT_RANGE`: port range for deterministic HTTP stub allocation.

## Registry and Gaps

- `system-tests/test_registry.toml` is the authoritative test inventory.
- `system-tests/test_gaps.toml` tracks missing coverage and acceptance criteria.

## Testing Utilities

Helper scripts live in `scripts/` at the repo root:

```bash
python3 scripts/test_runner.py --priority P0
python3 scripts/coverage_report.py generate
```

## References

Upon A Burning Body. (2025). _Living in a Matrix_ [Audio recording]. YouTube. https://www.youtube.com/watch?v=cG-Xyxt8K9s

Upon A Burning Body. (2016). _Straight From The Barrio (210)_ [Audio recording]. YouTube. https://www.youtube.com/watch?v=jxmkoKHbOU4
