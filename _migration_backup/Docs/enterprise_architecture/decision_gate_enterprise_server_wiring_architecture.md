<!--
Docs/architecture/enterprise/decision_gate_enterprise_server_wiring_architecture.md
============================================================================
Document: Decision Gate Enterprise Server Wiring Architecture
Description: Current-state reference for enterprise server assembly and config
             wiring (tenant authz, usage, storage, audit, metrics).
Purpose: Provide an implementation-grade map of how enterprise configuration
         builds an MCP server with hardened overrides.
Dependencies:
  - enterprise/decision-gate-enterprise/src/config.rs
  - enterprise/decision-gate-enterprise/src/server.rs
  - enterprise/decision-gate-enterprise/src/runpack_storage.rs
  - decision-gate-mcp/src/server.rs
  - decision-gate-mcp/src/config.rs
============================================================================
Last Updated: 2026-01-27 (UTC)
============================================================================
-->

# Decision Gate Enterprise Server Wiring Architecture

> **Audience:** Engineers wiring enterprise deployments or reviewing server
> assembly paths for managed cloud tiers.

---

## Table of Contents

1. [Executive Overview](#executive-overview)
2. [Configuration Inputs](#configuration-inputs)
3. [Server Assembly Flow](#server-assembly-flow)
4. [Override Surface](#override-surface)
5. [Failure Modes and Validation](#failure-modes-and-validation)
6. [Security + Isolation Invariants](#security--isolation-invariants)
7. [File-by-File Cross Reference](#file-by-file-cross-reference)

---

## Executive Overview

Enterprise server wiring builds a standard MCP server (`decision-gate-mcp`) with
explicit overrides for tenant authorization, usage metering, storage backends,
runpack storage, audit sinks, and metrics. The enterprise crate never mutates
OSS semantics; it only supplies override implementations via the existing seams.
Configuration is split between the OSS `DecisionGateConfig` and the enterprise
`EnterpriseConfig`, with validation performed before wiring any overrides.

---

## Configuration Inputs

### OSS Config (`DecisionGateConfig`)
Defines core MCP behavior: transport, auth mode, provider registry, validation,
trust policies, and server limits.

### Enterprise Config (`EnterpriseConfig`)
Adds managed-service wiring for:
- Postgres storage (run state + schema registry)
- S3-compatible runpack storage
- Usage metering configuration (ledger backend + quotas)

Configuration is loaded from `decision-gate-enterprise.toml` (or
`DECISION_GATE_ENTERPRISE_CONFIG`). The loader validates:
- File size and path length limits
- Required fields (e.g., Postgres connection string)
- Ledger backend requirements (e.g., sqlite path for SQLite ledger)

---

## Server Assembly Flow

1. Load `DecisionGateConfig` (OSS config).
2. Load and validate `EnterpriseConfig`.
3. Create enterprise overrides:
   - `TenantAuthorizer` (enterprise policy implementation)
   - `UsageMeter` (quota enforcer + ledger)
   - Optional `RunStateStore` + `DataShapeRegistry` (Postgres)
   - Optional `RunpackStorage` (S3 adapter)
   - Audit sink + metrics sink
4. Build `EnterpriseServerOptions` and assemble a `ServerOverrides` struct.
5. Call `McpServer::from_config_with_observability_and_overrides`.

This keeps the control plane, tool routing, and disclosure logic identical to
OSS, while adding enterprise-only wiring.

---

## Override Surface

Enterprise overrides map directly to the OSS extension points:
- `tenant_authorizer`: enforced for every tool call.
- `usage_meter`: fail-closed checks before tool execution.
- `run_state_store` + `schema_registry`: shared Postgres-backed stores.
- `runpack_storage`: S3-backed runpack storage adapter.
- `audit_sink` + `metrics`: observability hooks.

---

## Failure Modes and Validation

Enterprise wiring fails closed if:
- Config files are invalid or exceed size limits.
- Required paths or connection strings are missing.
- Storage backends cannot be initialized.

Errors are surfaced as `McpServerError::Init` to prevent partial startup with
missing enforcement.

---

## Security + Isolation Invariants

- Enterprise wiring must not weaken OSS defaults.
- Tenant authz and usage metering are enforced before any state mutation.
- Storage overrides must preserve determinism and auditability.
- Runpack storage must never accept unvalidated paths or unsafe prefixes.

---

## File-by-File Cross Reference

- Enterprise config loader: `enterprise/decision-gate-enterprise/src/config.rs`
- Enterprise server builder: `enterprise/decision-gate-enterprise/src/server.rs`
- Runpack storage adapter: `enterprise/decision-gate-enterprise/src/runpack_storage.rs`
- MCP server assembly: `decision-gate-mcp/src/server.rs`
- MCP config schema: `decision-gate-mcp/src/config.rs`
