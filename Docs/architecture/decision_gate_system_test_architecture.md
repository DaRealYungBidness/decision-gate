<!--
Docs/architecture/decision_gate_system_test_architecture.md
============================================================================
Document: Decision Gate System Test + Validation Architecture
Description: Current-state reference for end-to-end system-tests, registry-driven
             test inventory, and gap tracking.
Purpose: Provide an implementation-grade map of how DG validates behavior
         end-to-end and how coverage is tracked.
Dependencies:
  - system-tests/README.md
  - system-tests/AGENTS.md
  - system-tests/TEST_MATRIX.md
  - system-tests/test_registry.toml
  - system-tests/test_gaps.toml
============================================================================
Last Updated: 2026-02-04 (UTC)
============================================================================
-->

# Decision Gate System Test + Validation Architecture

> **Audience:** Engineers maintaining end-to-end coverage, release gating, and
> auditability for Decision Gate.

---

## Table of Contents

1. [Executive Overview](#executive-overview)
2. [System Test Contract](#system-test-contract)
3. [Registry-Driven Test Inventory](#registry-driven-test-inventory)
4. [Coverage Matrix](#coverage-matrix)
5. [Gap Tracking](#gap-tracking)
6. [Artifacts and Determinism](#artifacts-and-determinism)
7. [Execution Workflow](#execution-workflow)
8. [File-by-File Cross Reference](#file-by-file-cross-reference)

---

## Executive Overview

The `system-tests` crate provides the highest-rigor end-to-end validation layer
for OSS Decision Gate. Tests execute against a live MCP server and emit auditable
artifacts. Coverage is tracked in a registry file and a formal gap list. SDK
client tests execute the Python and TypeScript SDKs against live MCP HTTP
servers to validate transport correctness, auth behavior, and repository
examples (which are treated as runnable system tests).
[F:system-tests/README.md L14-L90](system-tests/README.md#L14-L90)

The agentic flow harness extends this model with canonical scenario packs that
run across raw MCP, SDKs, and framework adapters. Scenario packs live under
`system-tests/tests/fixtures/agentic/` and are mirrored into `examples/agentic/`
for discoverability.
Runpack hash expectations are OS-aware when necessary; harnesses may supply
`runpack_root_hash.<os>.txt` alongside the default `runpack_root_hash.txt` to
capture platform-specific but deterministic hashes (for example, Windows vs.
Unix serialization differences).

Operational posture coverage includes HTTP liveness/readiness probes to ensure
containerized deployments advertise correct health semantics.

---

## System Test Contract

System-tests must mirror production behavior and remain deterministic:

- No fail-open logic and no sleep-based correctness.
- Use production types and schemas.
- Reserve loopback ports via the harness allocator to prevent parallel bind
  collisions; stub servers bind directly to ephemeral port 0.
- Always emit required artifacts (`summary.json`, `summary.md`,
  `tool_transcript.json`).
- Register every test in `test_registry.toml` and gaps in `test_gaps.toml`.
- SDK client and example tests rely on Python 3 and Node (18+ with
  `--experimental-strip-types`); tests skip with explicit summaries if runtimes
  are unavailable.
- Optional agentic adapter drivers (LangChain/CrewAI/AutoGen/OpenAI Agents)
  downgrade adapter failures to skips unless
  `DECISION_GATE_STRICT_AGENTIC_ADAPTERS=1` is set, keeping OSS runs deterministic
  without requiring optional dependencies.

[F:system-tests/AGENTS.md L12-L62](system-tests/AGENTS.md#L12-L62) [F:system-tests/README.md L31-L90](system-tests/README.md#L31-L90)

---

## Registry-Driven Test Inventory

`system-tests/test_registry.toml` is the authoritative inventory. Each test
specifies:

- name, category, priority
- file locations and run command
- required artifacts

[F:system-tests/test_registry.toml L1-L43](system-tests/test_registry.toml#L1-L43)

Suite entrypoints live in `system-tests/tests/` and include test implementations
under `system-tests/tests/suites/`. This reduces test binary proliferation while
preserving category-driven inventory in the registry. Each registry
`run_command` targets the suite binary with `--test <suite> -- --exact <module>::<test>`.

---

## Coverage Matrix

The system test matrix documents P0/P1/P2 coverage with goals and categories.
This is the human-readable snapshot of what the registry enforces.
[F:system-tests/TEST_MATRIX.md L10-L75](system-tests/TEST_MATRIX.md#L10-L75)

---

## Gap Tracking

`system-tests/test_gaps.toml` captures missing coverage and acceptance criteria.
It provides a durable backlog for high-priority security, reliability, and audit
coverage gaps.
[F:system-tests/test_gaps.toml L1-L101](system-tests/test_gaps.toml#L1-L101)

---

## Artifacts and Determinism

Every system test writes deterministic artifacts under a per-test run root. The
run root is supplied by `DECISION_GATE_SYSTEM_TEST_RUN_ROOT`; when unset, tests
default to `target/system-tests/<run-id>/<test-name>` under the workspace root.
When executing under `nextest`, the `<run-id>` is `NEXTEST_RUN_ID`, which keeps
all binaries in a single run tree. Each test must emit canonical JSON artifacts
plus a tool transcript. Run roots must be unique and the harness fails closed
if the target directory already exists unless
`DECISION_GATE_SYSTEM_TEST_ALLOW_OVERWRITE=1` is set. Failing tests also emit a
one-line stderr summary that includes the artifact root for rapid triage.

Deterministic HTTP fixtures (for example the agentic harness HTTP stub) use
stable, scenario-derived ports to prevent runpack hash drift; the port can be
overridden via `DECISION_GATE_SYSTEM_TEST_HTTP_STUB_PORT` when needed.
[F:system-tests/README.md L82-L104](system-tests/README.md#L82-L104) [F:system-tests/AGENTS.md L71-L84](system-tests/AGENTS.md#L71-L84)

JSON evidence tests configure a per-test `json` provider root and use **relative**
file paths so evidence anchors are stable across operating systems. Absolute
paths are rejected by the provider to avoid runpack hash drift.

## Execution Workflow

Recommended entry points:

System test crates are feature-gated to avoid running in default unit-test
passes. Use explicit features or the registry-driven runner.

- `cargo test -p system-tests --features system-tests`
- `cargo nextest run -p system-tests --features system-tests`
- `python scripts/system_tests/test_runner.py --priority P0`

`cargo nextest` defaults are tuned in `.config/nextest.toml`, including emitting
failure output at the end of the run for easier triage.

Tests should be registered and coverage docs regenerated when changes occur.
[F:system-tests/README.md L45-L118](system-tests/README.md#L45-L118)

---

## File-by-File Cross Reference

| Area | File | Notes |
| --- | --- | --- |
| System test contract | `system-tests/AGENTS.md` | Determinism and audit requirements. |
| Usage and workflow | `system-tests/README.md` | Running tests and artifact contract. |
| Agentic harness bootstrap | `scripts/agentic/agentic_harness_bootstrap.sh` | Installs Python deps for agentic driver matrix. |
| Agentic harness runner | `scripts/agentic/agentic_harness.sh` | Deterministic entry point for agentic flow harness. |
| Adapter smoke tests | `scripts/adapters/adapter_tests.sh` | Installs adapter deps, runs conformance, and executes adapter examples. |
| Adapter conformance | `scripts/adapters/adapter_conformance.py` | Verifies adapter tool surfaces match MCP tooling.json. |
| Adapter roundtrip | `scripts/adapters/adapter_roundtrip.py` | Runs per-tool roundtrip calls through each adapter against live MCP server. |
| Adapter typecheck | `scripts/adapters/typecheck_adapters.sh` | Runs Pyright strict typing gate for adapters + adapter scripts. |
| Coverage matrix | `system-tests/TEST_MATRIX.md` | P0/P1/P2 summary. |
| Test registry | `system-tests/test_registry.toml` | Inventory + metadata. |
| Gap tracking | `system-tests/test_gaps.toml` | Missing coverage + acceptance criteria. |
| SDK system tests | `system-tests/tests/suites/sdk_client.rs` | Python + TypeScript SDK lifecycle + auth tests. |
| SDK example tests | `system-tests/tests/suites/sdk_examples.rs` | Repository Python/TypeScript examples executed as system tests. |
| Contract CLI tests | `system-tests/tests/suites/contract_cli.rs` | Contract generator CLI generate/check and drift detection. |
| Broker integration tests | `system-tests/tests/suites/broker_integration.rs` | CompositeBroker file/http/inline source wiring validation. |
| SDK fixtures | `system-tests/tests/fixtures/` | Language-specific SDK driver scripts. |
| Agentic scenario registry | `system-tests/tests/fixtures/agentic/scenario_registry.toml` | Canonical scenario inventory for the harness. |
| Agentic scenario packs | `system-tests/tests/fixtures/agentic/` | Deterministic fixtures for agentic flows. |
| Agentic harness suite | `system-tests/tests/suites/agentic_harness.rs` | Cross-projection scenario execution + invariance checks. |
