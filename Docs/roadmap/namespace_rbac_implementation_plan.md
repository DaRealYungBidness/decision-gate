<!--
Docs/roadmap/namespace_rbac_implementation_plan.md
============================================================================
Document: Namespace Policy + Registry RBAC/ACL Implementation Plan
Description: Full implementation plan for dev-permissive mode, namespace policy,
  and schema registry RBAC/ACL with ASC interop.
Purpose: Authoritative implementation spec for LLMs and engineers.
Dependencies:
  - Docs/roadmap/open_items.md
  - Docs/architecture/decision_gate_assetcore_integration_contract.md
  - Docs/integrations/assetcore/README.md
============================================================================
-->

# Namespace Policy + Registry RBAC/ACL Implementation Plan

## Status
Implemented. This document now reflects the shipped behavior and config surface.

## Scope
This document defines a world-class implementation plan for:
- Dev-permissive mode (explicit relaxation for development only).
- Default namespace policy (standalone vs ASC-integrated).
- Schema registry RBAC/ACL enforcement and audit events.

It is designed for an industrial-grade security posture and a nation-state
threat model while remaining easy to use via explicit, well-documented defaults.

## Non-Negotiable Principles
- **Fail-closed defaults**: Unknown or unreachable authorities deny access.
- **Explicitness**: Any relaxation requires explicit config and audit logging.
- **Separation of concerns**:
  - **ASC integration RBAC mapping** = external mapping of ASC principals to DG
    tool groups.
  - **DG registry ACL** = internal policy for schema registry operations.
- **Determinism first**: Same inputs yield identical outcomes and runpacks.
- **Auditability**: All security-relevant decisions are logged.

## Definitions
- **Namespace authority**: System of record for valid namespace IDs.
- **Standalone DG**: DG running without ASC integration.
- **ASC-integrated**: DG configured to use ASC as namespace authority.
- **Dev-permissive**: Explicit opt-in relaxation for asserted evidence only.

## Policy Decisions (Final)

### 1) Dev-Permissive Mode
- Allowed relaxation: **asserted evidence only**, non-ASC providers only.
- Disallowed relaxations by default:
  - schema registry writes
  - scenario authoring
  - any override of namespace authority
- Dev-permissive must be **disabled** if `namespace.authority.mode = "assetcore_http"`.
- Dev-permissive must emit:
  - startup warning
  - audit event
  - runpack metadata marker

### 2) Default Namespace Policy
- **Standalone DG**: `namespace.authority.mode = "none"`
- **ASC-integrated**: `namespace.authority.mode = "assetcore_http"`
- Default behavior for unknown namespaces: **deny**
- No implicit "single-tenant" allowances; explicit tenant allowlist required.

### 3) Schema Registry RBAC/ACL
- Registry ACL is enforced **after** RBAC mapping (if integrated).
- Schema registry operations:
  - `schemas_register` (write)
  - `schemas_list` (read)
  - `schemas_get` (read)
- **Default ACL (prod)**:
  - Read: `NamespaceReader+` within namespace.
  - Write: `NamespaceAdmin` or `TenantAdmin` only.
  - `SchemaManager` may write only when `policy_class` is non-prod.
- Schema writes are **immutable**; no overwrites. Versioning required.
- Optional but recommended in prod: schema signing key enforcement.

## Configuration Surface (Implemented)

### Namespace Policy
- `namespace.allow_default = true | false`
- `namespace.default_tenants = [1, ... ]` (required when allow_default is true)
- `namespace.authority.mode = "none" | "assetcore_http"`
- `namespace.authority.assetcore.base_url = "<url>"`
- `namespace.authority.assetcore.auth_token = "<token>"` (optional)
- `namespace.authority.assetcore.connect_timeout_ms = <int>`
- `namespace.authority.assetcore.request_timeout_ms = <int>`

### Dev-Permissive
- `dev.permissive = true | false`
- `dev.permissive_scope = asserted_evidence_only`
- `dev.permissive_ttl_days = <int>` (warn on expiry)
- `dev.permissive_warn = true | false`
- `dev.permissive_exempt_providers = [ ... ]`

### Registry ACL
- `schema_registry.acl.mode = builtin | custom`
- `schema_registry.acl.default = deny`
- `schema_registry.acl.rules = [ ... ]`
- `schema_registry.acl.require_signing = true | false`

## Implementation Summary

### A) Namespace Authority + Dev-Permissive (Standalone vs ASC)
Implemented:
- Config parsing/validation for `namespace.*` and `dev.*`.
- Asset Core authority requires `namespace.authority.assetcore` and disallows dev-permissive.
- Namespace validation applied on every tool call and evidence query.
- Fail-closed on unknown or unreachable namespace authority.
- Startup warnings for dev-permissive + TTL checks.
- `security_audit` event with `kind = "dev_permissive_enabled"` and runpack
  security metadata emitted when enabled.

### B) Registry ACL Enforcement + Audit
Implemented:
- `schema_registry.acl` evaluation for register/list/get.
- Built-in and custom ACL modes with per-tenant/namespace scoping.
- Registry allow/deny decisions emit `registry_audit` events.
- Schema overwrites rejected (immutability preserved).
- Optional `require_signing` enforces signing metadata presence.

### C) Integration Layer Alignment
Implemented:
- Integration contract updated to explicitly separate tool RBAC mapping from
  registry ACL enforcement.
- Registry ACL is evaluated after integration-layer allowlists.

## Required Audit Events
- `security_audit` with `kind = "dev_permissive_enabled"` (startup when enabled)
- `registry_audit` for registry allow/deny decisions
- `mcp_audit` entries for namespace validation failures (existing)

## Required Runpack Metadata
- `security.dev_permissive: true | false`
- `security.namespace_authority: dg_registry | assetcore_catalog`

## Tests (Must-Have)

### Namespace Policy
- Fail-closed when namespace unknown or authority unreachable.
- ASC authority cannot be used with dev-permissive.
- Namespace IDs must be numeric; invalid IDs are rejected.

### Dev-Permissive
- Asserted evidence accepted only when enabled.
- Non-ASC providers only; ASC-backed evidence unaffected.
- TTL warnings emitted when expired.

### Registry ACL
- Deny by default.
- Read allowed for NamespaceReader+ within namespace.
- Write allowed only for NamespaceAdmin/TenantAdmin in prod.
- SchemaManager only in non-prod policy_class.
- Registry decisions are audited.
- Overwrite attempts rejected.

## Open Questions (Post-Implementation)
- Should schema signing **verification** (not just presence) be required in prod?
- Should standalone deployments validate `policy_class` against a fixed enum?

## References
- `Docs/roadmap/open_items.md`
- `Docs/architecture/decision_gate_assetcore_integration_contract.md`
- `Docs/integrations/assetcore/README.md`
