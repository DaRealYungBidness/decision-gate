<!--
Docs/architecture/enterprise/decision_gate_enterprise_usage_quota_architecture.md
============================================================================
Document: Decision Gate Enterprise Usage + Quota Architecture
Description: Current-state reference for usage metering, quota policies, and
             ledger-backed enforcement.
Purpose: Provide an implementation-grade map of billing-grade usage tracking
         and fail-closed quota enforcement.
Dependencies:
  - enterprise/decision-gate-enterprise/src/usage.rs
  - enterprise/decision-gate-enterprise/src/usage_sqlite.rs
  - enterprise/decision-gate-enterprise/src/config.rs
  - decision-gate-mcp/src/usage.rs
  - decision-gate-mcp/src/tools.rs
============================================================================
Last Updated: 2026-01-27 (UTC)
============================================================================
-->

# Decision Gate Enterprise Usage + Quota Architecture

> **Audience:** Engineers implementing or reviewing enterprise metering,
> billing controls, and quota enforcement.

---

## Table of Contents

1. [Executive Overview](#executive-overview)
2. [Usage Metrics](#usage-metrics)
3. [Quota Policy Model](#quota-policy-model)
4. [Ledger Backends](#ledger-backends)
5. [Enforcement Flow](#enforcement-flow)
6. [Failure Modes](#failure-modes)
7. [File-by-File Cross Reference](#file-by-file-cross-reference)

---

## Executive Overview

Usage metering is enforced through the OSS `UsageMeter` seam. Enterprise
provides a quota enforcer backed by an append-only ledger, with explicit
idempotency support and fail-closed behavior on storage errors. Usage checks
run before tool execution; usage records are emitted after successful actions.

---

## Usage Metrics

Canonical counters follow `decision-gate-mcp::UsageMetric`:
- `tool_calls`
- `runs_started`
- `evidence_queries`
- `runpack_exports`
- `schemas_written`
- `registry_entries`
- `storage_bytes`

Metric labels are stable strings used for audit/event storage.

---

## Quota Policy Model

`QuotaPolicy` defines a list of `QuotaLimit` entries:
- `metric`: which usage counter to limit
- `max_units`: maximum allowed units
- `window_ms`: rolling time window
- `scope`: `Tenant` or `Namespace`

Quota scope keys are encoded as `tenant/namespace` (namespace `*` for tenant
scopes). The policy is evaluated per request.

---

## Ledger Backends

### In-Memory Ledger
- Used for dev/test.
- Stores usage events and idempotency keys in memory.

### SQLite Ledger
- Append-only table with idempotency key index.
- Uses WAL + synchronous FULL for durability.
- All operations fail closed on errors.

Ledger interface (`UsageLedger`):
- `append(event)`
- `sum_since(scope_key, metric, since_ms)`
- `seen_idempotency(key)`

---

## Enforcement Flow

1. Tool router calls `UsageMeter::check` before executing a tool.
2. Quota enforcer loads ledger totals for matching limits.
3. If any limit would be exceeded, request is denied (`quota_exceeded`).
4. After successful tool execution, `UsageMeter::record` is invoked.
5. Idempotency keys prevent double counting.

---

## Failure Modes

- Ledger read/write errors: deny request with `UsageDecision` set to disallow.
- Missing tenant/namespace for a usage check: fail closed.
- Overflow handling is saturating and bounded.

---

## File-by-File Cross Reference

- Usage policy + ledger traits: `enterprise/decision-gate-enterprise/src/usage.rs`
- SQLite ledger implementation: `enterprise/decision-gate-enterprise/src/usage_sqlite.rs`
- Enterprise config wiring: `enterprise/decision-gate-enterprise/src/config.rs`
- OSS usage seam: `decision-gate-mcp/src/usage.rs`
- Enforcement + audit: `decision-gate-mcp/src/tools.rs`
