<!--
Docs/architecture/enterprise/decision_gate_enterprise_tenant_authz_admin_architecture.md
============================================================================
Document: Decision Gate Enterprise Tenant Authz + Admin Architecture
Description: Current-state reference for tenant/namespace authorization,
             admin lifecycle primitives, and key issuance.
Purpose: Provide an implementation-grade map of enterprise identity scoping
         and tenant administration behavior.
Dependencies:
  - enterprise/decision-gate-enterprise/src/tenant_authz.rs
  - enterprise/decision-gate-enterprise/src/tenant_admin.rs
  - enterprise/decision-gate-enterprise/src/admin_ui.rs
  - enterprise/decision-gate-enterprise/src/server.rs
  - decision-gate-mcp/src/tenant_authz.rs
  - decision-gate-mcp/src/tools.rs
============================================================================
Last Updated: 2026-01-27 (UTC)
============================================================================
-->

# Decision Gate Enterprise Tenant Authz + Admin Architecture

> **Audience:** Engineers implementing or reviewing enterprise identity
> scoping, tenant administration, and key management.

---

## Table of Contents

1. [Executive Overview](#executive-overview)
2. [Tenant Authorization Model](#tenant-authorization-model)
3. [Policy Evaluation Flow](#policy-evaluation-flow)
4. [Tenant Administration Primitives](#tenant-administration-primitives)
5. [Admin UI Surface](#admin-ui-surface)
6. [Security Invariants](#security-invariants)
7. [File-by-File Cross Reference](#file-by-file-cross-reference)

---

## Executive Overview

Enterprise tenant authorization maps principals to explicit tenant and namespace
scopes. Authorization is enforced on every MCP tool call via the OSS
`TenantAuthorizer` seam, with denials audited in `decision-gate-mcp`.
Tenant administration primitives provide in-memory scaffolding for tenant
creation, namespace registration, and API key issuance with hashed storage.

---

## Tenant Authorization Model

The enterprise policy model is hierarchical:

- **PrincipalScope**: a principal id maps to one or more tenant scopes.
- **TenantScope**: a tenant id plus namespace scope (`All` or allowlist).
- **NamespaceScope**: either all namespaces or an explicit allowlist.

`TenantAuthzPolicy.require_tenant` defaults to `true`, enforcing that every
request include tenant + namespace identifiers. Missing fields are denied unless
explicitly configured otherwise.

---

## Policy Evaluation Flow

1. Tool router builds a `TenantAccessRequest` (tool + tenant + namespace).
2. `MappedTenantAuthorizer` looks up scopes for the principal id.
3. If no scopes exist, access is denied (`principal_unmapped`).
4. If tenant/namespace are missing and `require_tenant = true`, access is denied.
5. The request is allowed only if a tenant scope matches the namespace policy.
6. The router emits a tenant authz audit event with allow/deny reason.

This logic is fail-closed by default. No implicit access is granted on missing
identity or scope.

---

## Tenant Administration Primitives

`TenantAdminStore` provides minimal lifecycle operations:
- Create tenant records
- Register namespaces
- Issue API keys
- List tenants

The in-memory implementation:
- Stores tenant records + namespaces in a map keyed by tenant id
- Issues API keys using random 32-byte tokens
- Hashes keys with SHA-256 for storage (plaintext keys are never persisted)

This is intentionally lightweight scaffolding for early phases; production
backends should replace it with durable stores.

---

## Admin UI Surface

`admin_ui.rs` provides a static HTML scaffold for future admin surfaces (tenant
overview, key rotation, run list). It is not a full UI, but ensures an explicit
entry point for enterprise admin exposure.

---

## Security Invariants

- Tenant + namespace checks are mandatory when `require_tenant = true`.
- Authorization is evaluated before any state mutation.
- API keys are generated with cryptographic randomness and stored only as hashes.
- Denied requests are audited with explicit reason codes.

---

## File-by-File Cross Reference

- Enterprise tenant policy: `enterprise/decision-gate-enterprise/src/tenant_authz.rs`
- Tenant administration: `enterprise/decision-gate-enterprise/src/tenant_admin.rs`
- Admin UI scaffold: `enterprise/decision-gate-enterprise/src/admin_ui.rs`
- MCP tenant authz interface: `decision-gate-mcp/src/tenant_authz.rs`
- Enforcement + audit: `decision-gate-mcp/src/tools.rs`
