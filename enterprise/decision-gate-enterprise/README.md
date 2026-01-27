# Decision Gate Enterprise

## Overview
This crate hosts private, enterprise-only control-plane extensions for Decision
Gate (tenant authz, usage metering, quota enforcement, and admin lifecycle APIs).
It must not modify or fork Decision Gate core semantics.

Current scaffolding includes:
- A quota-enforcing usage meter backed by an append-only ledger (`src/usage.rs`).
- A hash-chained audit sink for tamper-evident logs (`src/audit_chain.rs`).
- Tenant administration primitives with API key issuance (`src/tenant_admin.rs`).
- Minimal admin UI shell (`src/admin_ui.rs`).
- Principal-to-tenant/namespace authorization policy (`src/tenant_authz.rs`).
- SQLite-backed usage ledger (`src/usage_sqlite.rs`).
- Enterprise server builder with overrides + Postgres wiring (`src/server.rs`).
- Enterprise config loader for Postgres/S3/usage wiring (`src/config.rs`).
- Runpack storage adapter for S3 (`src/runpack_storage.rs`).

Example configuration:
- `examples/decision-gate-enterprise.toml`

## Architecture
Enterprise current-state docs (kept in `Docs/architecture/enterprise/` for repo split):
- `Docs/architecture/enterprise/decision_gate_enterprise_server_wiring_architecture.md`
- `Docs/architecture/enterprise/decision_gate_enterprise_tenant_authz_admin_architecture.md`
- `Docs/architecture/enterprise/decision_gate_enterprise_usage_quota_architecture.md`
- `Docs/architecture/enterprise/decision_gate_enterprise_audit_chain_architecture.md`

## Governance and Standards
Follow the repository standards and security posture:

- `Docs/standards/codebase_formatting_standards.md`
- `Docs/standards/codebase_engineering_standards.md`
- `Docs/security/threat_model.md`

If any change affects security posture or trust boundaries, update
`Docs/security/threat_model.md` and the relevant docs in `Docs/architecture/`.

## OSS Boundary
- This crate may depend on OSS crates.
- OSS crates must not depend on this crate.
- Add functionality via explicit seams (traits/config), never by changing core
  Decision Gate semantics.
