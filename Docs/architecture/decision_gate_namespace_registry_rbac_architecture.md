<!--
Docs/architecture/decision_gate_namespace_registry_rbac_architecture.md
============================================================================
Document: Decision Gate Namespace + Registry RBAC/ACL Architecture
Description: Comprehensive reference for namespace policy, namespace authority
             integration, and schema registry ACL enforcement.
Purpose: Provide an implementation-grade map of namespace enforcement and
         registry RBAC/ACL behavior with security invariants.
Dependencies:
  - crates/decision-gate-config/src/config.rs
  - crates/decision-gate-mcp/src/tools.rs
  - crates/decision-gate-mcp/src/namespace_authority.rs
  - crates/decision-gate-mcp/src/registry_acl.rs
  - crates/decision-gate-mcp/src/auth.rs
  - crates/decision-gate-mcp/src/server.rs
  - crates/decision-gate-mcp/src/audit.rs
  - crates/decision-gate-core/src/core/data_shape.rs
  - crates/decision-gate-core/src/core/runpack.rs
  - crates/decision-gate-store-sqlite/src/store.rs
  - Docs/configuration/decision-gate.toml.md
  - Docs/architecture/decision_gate_assetcore_integration_contract.md
============================================================================
Last Updated: 2026-02-04 (UTC)
============================================================================
-->

# Decision Gate Namespace + Registry RBAC/ACL Architecture

> **Audience:** Engineers implementing or reviewing namespace policy, Asset Core
> namespace authority integration, and schema registry access control.
> This document is the canonical, implementation-aligned description of how
> Decision Gate (DG) enforces namespace boundaries and registry RBAC/ACL.

---

## Table of Contents

1. [Executive Overview](#executive-overview)
2. [Scope and Non-Goals](#scope-and-non-goals)
3. [Core Concepts and Types](#core-concepts-and-types)
4. [Namespace Policy](#namespace-policy)
   1. [Default Namespace Guard](#default-namespace-guard)
   2. [Namespace Authority Modes](#namespace-authority-modes)
   3. [Asset Core Mapping Rules](#asset-core-mapping-rules)
   4. [Failure Posture](#failure-posture)
5. [Registry RBAC/ACL](#registry-rbacacl)
   1. [Principal Resolution](#principal-resolution)
   2. [Builtin ACL Policy](#builtin-acl-policy)
   3. [Custom ACL Policy](#custom-acl-policy)
   4. [Signing Requirement](#signing-requirement)
6. [Authorization Decision Flows](#authorization-decision-flows)
7. [Audit and Observability](#audit-and-observability)
8. [Storage and Persistence](#storage-and-persistence)
9. [Security Invariants](#security-invariants)
10. [Testing and Validation](#testing-and-validation)
11. [File-by-File Cross Reference](#file-by-file-cross-reference)
12. [Related Documentation](#related-documentation)

---

## Executive Overview

Decision Gate enforces namespace isolation and schema registry authorization as
**first-class security boundaries**. All namespace and registry checks are
**fail-closed** and run before any registry or scenario state mutation. The
system is intentionally conservative:

- Namespace routing requires explicit validation; unknown namespaces are denied.
- The reserved default namespace id (1) is **blocked by default** and allowed only
  when both `namespace.allow_default = true` and the caller's tenant appears in
  `namespace.default_tenants`.[F:crates/decision-gate-config/src/config.rs L1025-L1052](crates/decision-gate-config/src/config.rs#L1025-L1052) [F:crates/decision-gate-mcp/src/tools.rs L2821-L2841](crates/decision-gate-mcp/src/tools.rs#L2821-L2841)
- Asset Core integration is explicit and bounded to configured endpoints and
  timeouts; no implicit namespace mapping is allowed.[F:crates/decision-gate-config/src/config.rs L1118-L1165](crates/decision-gate-config/src/config.rs#L1118-L1165) [F:crates/decision-gate-mcp/src/namespace_authority.rs L103-L159](crates/decision-gate-mcp/src/namespace_authority.rs#L103-L159)
- Schema registry access is guarded by a dedicated Registry ACL layer that is
  **independent** of tool allowlists; both must allow the action.[F:crates/decision-gate-mcp/src/registry_acl.rs L146-L215](crates/decision-gate-mcp/src/registry_acl.rs#L146-L215) [F:crates/decision-gate-mcp/src/tools.rs L2881-L2910](crates/decision-gate-mcp/src/tools.rs#L2881-L2910)

Registry RBAC/ACL is scoped by **tenant + namespace + subject** and can be
configured as either builtin or custom. Builtin policy is explicitly defined by
role names with policy class gating for write access.[F:crates/decision-gate-mcp/src/registry_acl.rs L218-L255](crates/decision-gate-mcp/src/registry_acl.rs#L218-L255)

---

## Scope and Non-Goals

**In scope**:
- Namespace validation in MCP tools.
- Default namespace restrictions.
- Asset Core namespace authority integration.
- Schema registry RBAC/ACL policy and signing requirements.
- Audit and runpack security context for these decisions.

**Out of scope** (handled elsewhere or intentionally excluded):
- Asset Core write-path gating (explicitly out of scope in the integration
  contract).
- Scenario-level RBAC or tool allowlists (covered in server auth docs).
- Evidence provider trust enforcement (documented in security guide).

---

## Core Concepts and Types

### Namespace and Tenant Identifiers
Namespaces and tenants are treated as explicit identifiers carried by tool
requests. The reserved default namespace identifier is **`1`** and must pass
additional checks before any operation proceeds.[F:crates/decision-gate-mcp/src/tools.rs L162-L168](crates/decision-gate-mcp/src/tools.rs#L162-L168) [F:crates/decision-gate-mcp/src/tools.rs L2821-L2841](crates/decision-gate-mcp/src/tools.rs#L2821-L2841)

### Schema Registry Records
Schema registry entries are immutable `DataShapeRecord` values. Each record is
scoped by tenant + namespace + schema id + version, and may include optional
signing metadata (key id, signature, optional algorithm).[F:crates/decision-gate-core/src/core/data_shape.rs L49-L72](crates/decision-gate-core/src/core/data_shape.rs#L49-L72)

---

## Namespace Policy

### Default Namespace Guard
The default namespace is a hard-coded reserved identifier (`1`). Its
behavior is intentionally narrow:

- If `namespace.allow_default` is false, **all** requests targeting the default
  namespace are rejected.
- If `namespace.allow_default` is true, `namespace.default_tenants` must be
  non-empty and the caller must supply a `tenant_id` in the allowlist.
- The default namespace guard is enforced **before** external authority checks.

Implementation:
- Config validation enforces the allowlist requirement.[F:crates/decision-gate-config/src/config.rs L1025-L1052](crates/decision-gate-config/src/config.rs#L1025-L1052)
- Tool router enforces the guard per request.[F:crates/decision-gate-mcp/src/tools.rs L2821-L2841](crates/decision-gate-mcp/src/tools.rs#L2821-L2841)

### Namespace Authority Modes
Namespace authority determines how DG validates namespace existence:

| Mode | Behavior | Source |
| --- | --- | --- |
| `none` | No external authority checks (DG-local namespace policy only) | `NamespaceAuthorityMode::None`[F:crates/decision-gate-config/src/config.rs L1056-L1115](crates/decision-gate-config/src/config.rs#L1056-L1115) |
| `assetcore_http` | Validate namespace via Asset Core write daemon HTTP API | `NamespaceAuthorityMode::AssetcoreHttp`[F:crates/decision-gate-config/src/config.rs L1056-L1115](crates/decision-gate-config/src/config.rs#L1056-L1115) [F:crates/decision-gate-mcp/src/namespace_authority.rs L67-L159](crates/decision-gate-mcp/src/namespace_authority.rs#L67-L159) |

When `assetcore_http` is enabled, DG validates namespaces by issuing a GET
request to `/{base_url}/v1/write/namespaces/{resolved_id}`. HTTP 200 = allowed;
404 or 401/403 = denied; other statuses and transport errors are treated as
unavailable (fail closed).[F:crates/decision-gate-mcp/src/namespace_authority.rs L130-L158](crates/decision-gate-mcp/src/namespace_authority.rs#L130-L158)

Asset Core authority requests can include an optional bearer token and an
`x-correlation-id` header derived from the **unsafe** client-provided
correlation header when available (falling back to the JSON-RPC request id or
server-issued correlation id). Client correlation IDs are strictly validated
and rejected when invalid; only sanitized values are forwarded to the namespace
authority to prevent header injection and log spoofing.[F:crates/decision-gate-mcp/src/tools.rs L2821-L2852](crates/decision-gate-mcp/src/tools.rs#L2821-L2852) [F:crates/decision-gate-mcp/src/namespace_authority.rs L103-L126](crates/decision-gate-mcp/src/namespace_authority.rs#L103-L126) [F:crates/decision-gate-mcp/src/server.rs L1648-L1657](crates/decision-gate-mcp/src/server.rs#L1648-L1657)

**Integration constraint:** dev-permissive mode is **disallowed** when
`namespace.authority.mode = assetcore_http` to avoid weakening namespace
security in integrated deployments.[F:crates/decision-gate-config/src/config.rs L580-L603](crates/decision-gate-config/src/config.rs#L580-L603)

### Asset Core Namespace Rules
Namespace identifiers are numeric everywhere (>= 1). Asset Core authority
validation is direct and does not apply any mapping or translation. Any parse
failure yields a namespace validation error (fail closed).[F:crates/decision-gate-config/src/config.rs L1118-L1165](crates/decision-gate-config/src/config.rs#L1118-L1165) [F:crates/decision-gate-mcp/src/namespace_authority.rs L130-L158](crates/decision-gate-mcp/src/namespace_authority.rs#L130-L158)

Config validation enforces required Asset Core settings (base URL, timeout
ranges) when Asset Core authority is enabled.[F:crates/decision-gate-config/src/config.rs L1118-L1165](crates/decision-gate-config/src/config.rs#L1118-L1165)

### Failure Posture
Namespace authority failures are mapped as follows:

- Invalid namespace input -> `InvalidParams` (caller error)
- Denied or unavailable authority -> `Unauthorized` (fail closed)

This ensures missing namespaces and upstream outages are treated as access
failures rather than allowed paths.[F:crates/decision-gate-mcp/src/tools.rs L3217-L3222](crates/decision-gate-mcp/src/tools.rs#L3217-L3222)

---

## Registry RBAC/ACL

### Principal Resolution
Registry ACL is based on a principal derived from the MCP auth context:

- `AuthContext.principal_id()` resolves to subject, token fingerprint, or a
  stable fallback label (local/token/mtls).
- Principal profiles are configured in `server.auth.principals`, each with
  optional policy class and role bindings.
- Role bindings can be globally scoped or restricted to a tenant and/or
  namespace.

Implementation references:
- Principal id derivation.[F:crates/decision-gate-mcp/src/auth.rs L181-L216](crates/decision-gate-mcp/src/auth.rs#L181-L216)
- Principal configuration and validation.[F:crates/decision-gate-config/src/config.rs L802-L977](crates/decision-gate-config/src/config.rs#L802-L977)
- Principal mapping resolver and role scoping logic.[F:crates/decision-gate-mcp/src/registry_acl.rs L54-L143](crates/decision-gate-mcp/src/registry_acl.rs#L54-L143)

### Builtin ACL Policy
Builtin policy is the default (`schema_registry.acl.mode = builtin`). The
behavior is intentionally conservative and anchored on canonical role names:

**Read (List/Get) allowed when the principal holds any of:**
- TenantAdmin, NamespaceOwner, NamespaceAdmin, NamespaceWriter,
  NamespaceReader, SchemaManager

**Write (Register) allowed when:**
- TenantAdmin, NamespaceOwner, or NamespaceAdmin; OR
- SchemaManager **and** policy class is not `prod`

If no policy class is supplied, it is treated as `prod` (fail closed for
SchemaManager writes).[F:crates/decision-gate-mcp/src/registry_acl.rs L218-L283](crates/decision-gate-mcp/src/registry_acl.rs#L218-L283)

### Custom ACL Policy
Custom policy (`schema_registry.acl.mode = custom`) evaluates rules in order
and returns the first match. A rule matches when all non-empty dimensions match:

- action
- tenant
- namespace
- subject (principal id)
- roles (scoped role match)
- policy class

If no rules match, the default effect (`allow` or `deny`) is applied.
[F:crates/decision-gate-mcp/src/registry_acl.rs L287-L339](crates/decision-gate-mcp/src/registry_acl.rs#L287-L339) [F:crates/decision-gate-config/src/config.rs L1723-L1812](crates/decision-gate-config/src/config.rs#L1723-L1812)

### Signing Requirement
Registry ACL can require schema signing metadata:

- `schema_registry.acl.require_signing = true` enforces presence of `signing`
  metadata on schema records.
- Missing or empty signing metadata is rejected as unauthorized before registry
  mutation.[F:crates/decision-gate-config/src/config.rs L1768-L1785](crates/decision-gate-config/src/config.rs#L1768-L1785) [F:crates/decision-gate-mcp/src/tools.rs L3032-L3041](crates/decision-gate-mcp/src/tools.rs#L3032-L3041)

---

## Authorization Decision Flows

### Namespace Enforcement Flow (All Namespace-Scoped Tools)

```
Request
  -> Tool auth (DefaultToolAuthz)
  -> ensure_namespace_allowed
     -> default namespace guard
     -> namespace authority check (optional)
  -> tool execution
```

The default namespace guard runs before authority checks to prevent any implicit
fallbacks or bypasses.[F:crates/decision-gate-mcp/src/tools.rs L2821-L2841](crates/decision-gate-mcp/src/tools.rs#L2821-L2841)

### Registry ACL Flow (schemas_register/list/get)

```
Request
  -> Tool auth (DefaultToolAuthz)
  -> ensure_namespace_allowed
  -> ensure_registry_access
     -> resolve principal (auth context -> profile)
     -> evaluate registry ACL (builtin or custom)
     -> emit registry audit event
  -> validate signing metadata (optional)
  -> registry mutation or read
```

Implementation references:
- Registry ACL enforcement + auditing.[F:crates/decision-gate-mcp/src/tools.rs L2881-L2910](crates/decision-gate-mcp/src/tools.rs#L2881-L2910)
- Registry ACL evaluator (builtin/custom).[F:crates/decision-gate-mcp/src/registry_acl.rs L146-L339](crates/decision-gate-mcp/src/registry_acl.rs#L146-L339)

---

## Audit and Observability

DG emits explicit audit records for registry access decisions and security
posture changes:

- `RegistryAuditEvent` captures tenant, namespace, action, allow/deny decision,
  reason, principal roles, schema identity, and correlation identifiers (unsafe
  client + server-issued) for audit traceability.[F:crates/decision-gate-mcp/src/audit.rs L112-L145](crates/decision-gate-mcp/src/audit.rs#L112-L145)
- `SecurityAuditEvent` records dev-permissive activation (and invalid
  correlation rejections) along with namespace authority posture; correlation
  identifiers are included when the event is tied to a request.
  [F:crates/decision-gate-mcp/src/audit.rs L204-L223](crates/decision-gate-mcp/src/audit.rs#L204-L223) [F:crates/decision-gate-mcp/src/server.rs L1739-L1749](crates/decision-gate-mcp/src/server.rs#L1739-L1749)
- Runpack exports embed `RunpackSecurityContext` with dev-permissive and
  namespace authority metadata, making security posture verifiable offline.
  [F:crates/decision-gate-core/src/core/runpack.rs L94-L104](crates/decision-gate-core/src/core/runpack.rs#L94-L104) [F:crates/decision-gate-mcp/src/server.rs L534-L543](crates/decision-gate-mcp/src/server.rs#L534-L543)

---

## Storage and Persistence

Schema registry records include signing metadata end-to-end:

- `DataShapeRecord` includes optional signing fields.
- SQLite registry persists signing metadata in dedicated columns and migrates
  schema version 3 -> 4 to add the signing columns.

References:
- Data shape type definition.[F:crates/decision-gate-core/src/core/data_shape.rs L49-L72](crates/decision-gate-core/src/core/data_shape.rs#L49-L72)
- SQLite registry storage and migration.[F:crates/decision-gate-store-sqlite/src/store.rs L318-L480](crates/decision-gate-store-sqlite/src/store.rs#L318-L480) [F:crates/decision-gate-store-sqlite/src/store.rs L1013-L1037](crates/decision-gate-store-sqlite/src/store.rs#L1013-L1037)

---

## Security Invariants

1. **Fail-closed namespace enforcement:** Invalid, unknown, or unreachable
   namespace authority always denies access.
   [F:crates/decision-gate-mcp/src/namespace_authority.rs L130-L158](crates/decision-gate-mcp/src/namespace_authority.rs#L130-L158) [F:crates/decision-gate-mcp/src/tools.rs L3217-L3222](crates/decision-gate-mcp/src/tools.rs#L3217-L3222)
2. **No implicit default namespace:** id `1` requires explicit allowlist and
   tenant match; otherwise denied.
   [F:crates/decision-gate-config/src/config.rs L1025-L1052](crates/decision-gate-config/src/config.rs#L1025-L1052) [F:crates/decision-gate-mcp/src/tools.rs L2821-L2841](crates/decision-gate-mcp/src/tools.rs#L2821-L2841)
3. **Asset Core integration is strict:** Asset Core config is required when
   `namespace.authority.mode = assetcore_http`, and dev-permissive is disallowed
   when using Asset Core authority.
   [F:crates/decision-gate-config/src/config.rs L580-L603](crates/decision-gate-config/src/config.rs#L580-L603) [F:crates/decision-gate-config/src/config.rs L1118-L1165](crates/decision-gate-config/src/config.rs#L1118-L1165)
4. **Registry ACL is authoritative:** Tool allowlists do not bypass registry
   ACL; registry access is enforced and audited for every registry action.
   [F:crates/decision-gate-mcp/src/tools.rs L2881-L2910](crates/decision-gate-mcp/src/tools.rs#L2881-L2910)
5. **Local-only registry access is explicit:** `schema_registry.acl.allow_local_only`
   defaults to `false`. When enabled, the built-in ACL can allow loopback/stdio
   subjects to bypass principal mapping; this does not apply to custom ACL rules.
   [F:crates/decision-gate-config/src/config.rs L1768-L1785](crates/decision-gate-config/src/config.rs#L1768-L1785) [F:crates/decision-gate-mcp/src/registry_acl.rs L218-L230](crates/decision-gate-mcp/src/registry_acl.rs#L218-L230)
6. **Schema signing enforcement is explicit:** When enabled, signing metadata
   is mandatory for registry writes.
   [F:crates/decision-gate-mcp/src/tools.rs L3032-L3041](crates/decision-gate-mcp/src/tools.rs#L3032-L3041)

---

## Testing and Validation

The registry and namespace layers must be validated at both unit and system
levels. Recommended coverage includes:

- Namespace authority mapping (explicit vs numeric) and fail-closed behavior.
- Default namespace allowlist enforcement.
- Builtin registry ACL matrix (read/write by role and policy class).
- Custom ACL matching precedence + default effects.
- Signing-required enforcement and persistence of signing metadata.

System-test gaps are tracked in `system-tests/test_gaps.toml` and should be
maintained alongside changes to these policies.

---

## File-by-File Cross Reference

| Area | File | Notes |
| --- | --- | --- |
| Namespace config + validation | `crates/decision-gate-config/src/config.rs` | Namespace policy + Asset Core authority config and validation. |
| Default namespace + authority enforcement | `crates/decision-gate-mcp/src/tools.rs` | `ensure_namespace_allowed` and namespace error mapping. |
| Namespace authority integration | `crates/decision-gate-mcp/src/namespace_authority.rs` | HTTP validation, mapping rules, fail-closed semantics. |
| Registry ACL engine | `crates/decision-gate-mcp/src/registry_acl.rs` | Principal mapping + builtin/custom ACL evaluation. |
| Registry ACL enforcement | `crates/decision-gate-mcp/src/tools.rs` | `ensure_registry_access` + audit emission + signing checks. |
| Auth principal identifiers | `crates/decision-gate-mcp/src/auth.rs` | Stable principal ids for ACL mapping. |
| Audit event schemas | `crates/decision-gate-mcp/src/audit.rs` | Registry + security audit payloads. |
| Runpack security context | `crates/decision-gate-core/src/core/runpack.rs` | Security metadata embedded in runpacks. |
| Schema registry persistence | `crates/decision-gate-store-sqlite/src/store.rs` | Signing metadata columns and migrations. |

---

## Related Documentation

- `Docs/configuration/decision-gate.toml.md`
- `Docs/architecture/decision_gate_assetcore_integration_contract.md`
- `Docs/guides/security_guide.md`
