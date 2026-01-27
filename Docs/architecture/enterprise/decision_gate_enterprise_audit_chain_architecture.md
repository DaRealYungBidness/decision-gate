<!--
Docs/architecture/enterprise/decision_gate_enterprise_audit_chain_architecture.md
============================================================================
Document: Decision Gate Enterprise Audit Chain Architecture
Description: Current-state reference for hash-chained, tamper-evident audit
             logging and export behavior.
Purpose: Provide an implementation-grade map of audit immutability guarantees.
Dependencies:
  - enterprise/decision-gate-enterprise/src/audit_chain.rs
  - enterprise/enterprise-system-tests/tests/suites/audit.rs
============================================================================
Last Updated: 2026-01-27 (UTC)
============================================================================
-->

# Decision Gate Enterprise Audit Chain Architecture

> **Audience:** Engineers implementing or reviewing audit immutability and
> export integrity for enterprise deployments.

---

## Table of Contents

1. [Executive Overview](#executive-overview)
2. [Hash Chain Model](#hash-chain-model)
3. [Event Serialization + Storage](#event-serialization--storage)
4. [Tamper Detection](#tamper-detection)
5. [Export Format](#export-format)
6. [File-by-File Cross Reference](#file-by-file-cross-reference)

---

## Executive Overview

Enterprise audit logging is implemented as an append-only, hash-chained log.
Each record includes a hash of the prior record, producing a tamper-evident
chain. The audit sink is designed to be export-friendly (JSONL) and fail closed
on storage errors.

---

## Hash Chain Model

Each audit record includes:
- Canonical JSON payload
- Hash of the payload
- Hash of the previous record (or a fixed genesis value)

The chain is linear and ordered. Any deletion or mutation breaks subsequent
hashes, enabling offline verification.

---

## Event Serialization + Storage

- Events are serialized to JSONL for append-only storage.
- The sink enforces explicit file handling modes and avoids silent truncation.
- Failures are surfaced to callers; no best-effort partial writes.

---

## Tamper Detection

Verification recomputes hashes across the chain and fails on the first mismatch.
System tests cover:
- Missing records
- Corrupted payloads
- Corrupted previous-hash links

---

## Export Format

Audit export is JSONL (one JSON object per line), enabling streaming ingestion
into SIEM pipelines and offline verification tooling.

---

## File-by-File Cross Reference

- Audit sink implementation: `enterprise/decision-gate-enterprise/src/audit_chain.rs`
- Audit system tests: `enterprise/enterprise-system-tests/tests/suites/audit.rs`
