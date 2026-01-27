<!--
Docs/architecture/enterprise/decision_gate_enterprise_system_test_architecture.md
============================================================================
Document: Decision Gate Enterprise System Test Architecture
Description: Current-state reference for enterprise system test harness,
             fixtures, and artifact contracts.
Purpose: Provide an implementation-grade map of how enterprise system tests are
         structured, deterministic, and audit-ready.
Dependencies:
  - enterprise/enterprise-system-tests/README.md
  - enterprise/enterprise-system-tests/AGENTS.md
  - enterprise/enterprise-system-tests/tests/helpers/harness.rs
  - enterprise/enterprise-system-tests/tests/helpers/infra.rs
  - enterprise/enterprise-system-tests/tests/helpers/artifacts.rs
  - enterprise/enterprise-system-tests/tests/helpers/mcp_client.rs
============================================================================
Last Updated: 2026-01-27 (UTC)
============================================================================
-->

# Decision Gate Enterprise System Test Architecture

> **Audience:** Engineers adding or reviewing enterprise system tests.

---

## Table of Contents

1. [Executive Overview](#executive-overview)
2. [Test Harness](#test-harness)
3. [Infrastructure Fixtures](#infrastructure-fixtures)
4. [Artifact Contract](#artifact-contract)
5. [Execution Workflow](#execution-workflow)
6. [Determinism and Fail-Closed Rules](#determinism-and-fail-closed-rules)
7. [File-by-File Cross Reference](#file-by-file-cross-reference)

---

## Executive Overview

Enterprise system tests run end-to-end against the MCP server using production
request shapes. The harness provides deterministic server startup, fixtures for
Postgres + S3-compatible storage (via Docker or external endpoints), and a
strict artifact contract for audit review.

---

## Test Harness

The harness provides:
- Loopback server allocation (free ports).
- Base MCP configuration builders (HTTP, TLS, mTLS).
- Typed HTTP client for tool calls.
- Lifecycle management for server tasks.

All tests use explicit readiness checks and must not rely on wall-clock sleeps
for correctness.

---

## Infrastructure Fixtures

Postgres and S3 fixtures use `testcontainers` when no external service is
provided. Environment overrides allow running tests against external
infrastructure:

- `DECISION_GATE_ENTERPRISE_PG_URL`
- `DECISION_GATE_ENTERPRISE_S3_ENDPOINT`
- `DECISION_GATE_ENTERPRISE_S3_BUCKET`
- `DECISION_GATE_ENTERPRISE_S3_ACCESS_KEY`
- `DECISION_GATE_ENTERPRISE_S3_SECRET_KEY`

A deterministic preflight check validates Docker availability before starting
containers.

---

## Artifact Contract

Every enterprise system test emits:
- `summary.json`
- `summary.md`
- `tool_transcript.json`

Artifacts are required for auditability and cross-run diffing.

---

## Execution Workflow

Enterprise system tests are feature-gated to avoid running in default unit-test
passes. Run them explicitly:

- `cargo test -p enterprise-system-tests --features enterprise-system-tests`
- `cargo nextest run -p enterprise-system-tests --features enterprise-system-tests`

---

## Determinism and Fail-Closed Rules

- All tests must be deterministic and explicit about failures.
- No fail-open logic is permitted in assertions.
- Tests should use readiness probes rather than fixed sleeps.

---

## File-by-File Cross Reference

- Harness: `enterprise/enterprise-system-tests/tests/helpers/harness.rs`
- Infra fixtures: `enterprise/enterprise-system-tests/tests/helpers/infra.rs`
- Artifacts: `enterprise/enterprise-system-tests/tests/helpers/artifacts.rs`
- MCP client: `enterprise/enterprise-system-tests/tests/helpers/mcp_client.rs`
- Standards: `enterprise/enterprise-system-tests/README.md`
