<!--
Docs/architecture/enterprise/decision_gate_enterprise_storage_architecture.md
============================================================================
Document: Decision Gate Enterprise Storage Architecture
Description: Current-state reference for enterprise storage backends, including
             Postgres stores, runpack storage, and backup/restore hooks.
Purpose: Provide an implementation-grade map of durable, multi-tenant storage
         for managed deployments.
Dependencies:
  - enterprise/decision-gate-store-enterprise/src/postgres_store.rs
  - enterprise/decision-gate-store-enterprise/src/sqlite_store.rs
  - enterprise/decision-gate-store-enterprise/src/runpack_store.rs
  - enterprise/decision-gate-store-enterprise/src/s3_runpack_store.rs
  - enterprise/decision-gate-enterprise/src/runpack_storage.rs
  - enterprise/decision-gate-enterprise/src/config.rs
  - Docs/roadmap/enterprise/enterprise_backup_restore_runbook.md
============================================================================
Last Updated: 2026-01-27 (UTC)
============================================================================
-->

# Decision Gate Enterprise Storage Architecture

> **Audience:** Engineers implementing or reviewing enterprise storage
> backends and durability guarantees.

---

## Table of Contents

1. [Executive Overview](#executive-overview)
2. [Run State + Schema Registry Stores](#run-state--schema-registry-stores)
3. [Runpack Storage](#runpack-storage)
4. [Backup / Restore Hooks](#backup--restore-hooks)
5. [Configuration Wiring](#configuration-wiring)
6. [Security + Integrity Invariants](#security--integrity-invariants)
7. [File-by-File Cross Reference](#file-by-file-cross-reference)

---

## Executive Overview

Enterprise storage uses OSS-defined traits (`RunStateStore`, `DataShapeRegistry`)
with private implementations for Postgres and S3-compatible object storage.
Storage backends are configured via enterprise config and injected via server
overrides, preserving OSS semantics and determinism.

OSS now supports per-artifact object-store runpacks; enterprise adapters remain
the path for WORM/compliance workflows and bundled archive storage.

---

## Run State + Schema Registry Stores

### Postgres Store
The Postgres backend implements both run state persistence and schema registry
operations. It is designed for multi-tenant isolation via explicit tenant +
namespace keys and uses deterministic serialization for stored artifacts.

### Enterprise SQLite Wrapper
An enterprise wrapper around the OSS SQLite store provides a single-node
fallback or early managed-deployment option.

---

## Runpack Storage

### Filesystem Runpack Store
- Local filesystem storage for development or single-node deployments.
- Rejects symlinks and validates path segments.

### S3 Runpack Store
- Uses S3-compatible object storage with per-tenant prefixes.
- Supports server-side encryption (SSE-S3 or SSE-KMS).
- Enforces archive size limits and path safety for runpack bundles.

### WORM and Compliance (Enterprise Only)
- WORM retention and legal hold are enforced via S3 Object Lock on upload.
- Configuration lives under `runpacks.s3.object_lock`:
  - `mode` (`governance` or `compliance`) + `retain_until` (RFC-3339) are
    required together for retention.
  - `legal_hold` is optional and can be used independently.
- Buckets must have Object Lock enabled and credentials must allow the
  `s3:PutObjectRetention`/`s3:PutObjectLegalHold` permissions as applicable.
- Compliance attestations beyond Object Lock metadata are not emitted in OSS
  logs; enterprise audit pipelines may extend this.

### MCP Adapter
The enterprise runpack storage adapter bridges the S3 store to the MCP
`RunpackStorage` interface so managed deployments can export runpacks directly
into object storage.

---

## Backup / Restore Hooks

Backup and restore procedures are documented in:
`Docs/roadmap/enterprise/enterprise_backup_restore_runbook.md`.
Tests validate that backups round-trip and detect corruption.

---

## Configuration Wiring

Enterprise config selects:
- Postgres store via `storage.postgres`.
- S3 runpack store via `runpacks.s3`.
- Usage ledger via `usage.ledger`.

Wiring is performed by `EnterpriseConfig::build_server_options_with_metrics`.

---

## Security + Integrity Invariants

- Storage backends must be tenant/namespace aware and fail closed.
- Runpack storage must validate keys and reject symlinks.
- Encryption is enforced for S3 when configured.
- Corruption detection is mandatory for backups and restores.

---

## File-by-File Cross Reference

- Postgres store: `enterprise/decision-gate-store-enterprise/src/postgres_store.rs`
- SQLite wrapper: `enterprise/decision-gate-store-enterprise/src/sqlite_store.rs`
- Runpack store traits + filesystem: `enterprise/decision-gate-store-enterprise/src/runpack_store.rs`
- S3 runpack store: `enterprise/decision-gate-store-enterprise/src/s3_runpack_store.rs`
- MCP runpack adapter: `enterprise/decision-gate-enterprise/src/runpack_storage.rs`
- Enterprise config wiring: `enterprise/decision-gate-enterprise/src/config.rs`
- Backup runbook: `Docs/roadmap/enterprise/enterprise_backup_restore_runbook.md`
