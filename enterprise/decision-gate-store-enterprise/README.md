# Decision Gate Enterprise Store

## Overview
This crate hosts private, production-grade storage backends for Decision Gate
enterprise deployments (multi-tenant Postgres stores, object storage for
runpacks, and audit retention support).

Current scaffolding includes:
- `EnterpriseSqliteStore` wrapper for early managed deployments.
- `FilesystemRunpackStore` as a local runpack backend.
- `PostgresStore` for durable multi-tenant run state + schema registry storage.
- `S3RunpackStore` for object storage runpacks (feature: `s3`).
- `shared_postgres_store` helper for wiring Postgres into shared store wrappers.

## Architecture
Enterprise current-state docs (kept in `Docs/architecture/enterprise/` for repo split):
- `Docs/architecture/enterprise/decision_gate_enterprise_storage_architecture.md`

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
- Implement Decision Gate store traits without changing OSS semantics.
