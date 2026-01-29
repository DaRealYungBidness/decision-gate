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
Last Updated: 2026-01-29 (UTC)
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
[F:system-tests/README.md L10-L87]

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

[F:system-tests/AGENTS.md L12-L57][F:system-tests/README.md L30-L61]

---

## Registry-Driven Test Inventory

`system-tests/test_registry.toml` is the authoritative inventory. Each test
specifies:

- name, category, priority
- file locations and run command
- required artifacts

[F:system-tests/test_registry.toml L1-L43]

Suite entrypoints live in `system-tests/tests/` and include test implementations
under `system-tests/tests/suites/`. This reduces test binary proliferation while
preserving category-driven inventory in the registry. Each registry
`run_command` targets the suite binary with `--test <suite> -- --exact <module>::<test>`.

---

## Coverage Matrix

The system test matrix documents P0/P1/P2 coverage with goals and categories.
This is the human-readable snapshot of what the registry enforces.
[F:system-tests/TEST_MATRIX.md L10-L66]

---

## Gap Tracking

`system-tests/test_gaps.toml` captures missing coverage and acceptance criteria.
It provides a durable backlog for high-priority security, reliability, and audit
coverage gaps.
[F:system-tests/test_gaps.toml L1-L101]

---

## Artifacts and Determinism

Every system test writes deterministic artifacts under a per-test run root. The
run root is supplied by `DECISION_GATE_SYSTEM_TEST_RUN_ROOT` and each test must
emit canonical JSON artifacts plus a tool transcript.
[F:system-tests/README.md L74-L82][F:system-tests/AGENTS.md L63-L67]

## Execution Workflow

Recommended entry points:

System test crates are feature-gated to avoid running in default unit-test
passes. Use explicit features or the registry-driven runner.

- `cargo test -p system-tests --features system-tests`
- `cargo nextest run -p system-tests --features system-tests`
- `python scripts/test_runner.py --priority P0`

Tests should be registered and coverage docs regenerated when changes occur.
[F:system-tests/README.md L38-L59]

---

## File-by-File Cross Reference

| Area | File | Notes |
| --- | --- | --- |
| System test contract | `system-tests/AGENTS.md` | Determinism and audit requirements. |
| Usage and workflow | `system-tests/README.md` | Running tests and artifact contract. |
| Coverage matrix | `system-tests/TEST_MATRIX.md` | P0/P1/P2 summary. |
| Test registry | `system-tests/test_registry.toml` | Inventory + metadata. |
| Gap tracking | `system-tests/test_gaps.toml` | Missing coverage + acceptance criteria. |
| SDK system tests | `system-tests/tests/suites/sdk_client.rs` | Python + TypeScript SDK lifecycle + auth tests. |
| SDK example tests | `system-tests/tests/suites/sdk_examples.rs` | Repository Python/TypeScript examples executed as system tests. |
| SDK fixtures | `system-tests/tests/fixtures/` | Language-specific SDK driver scripts. |
