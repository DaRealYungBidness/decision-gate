<!--
Docs/architecture/decision_gate_namespace_registry_rbac_architecture.md
============================================================================
Document: Decision Gate Namespace + Registry RBAC/ACL Architecture
Description: Comprehensive reference for namespace policy, namespace authority
             integration, and schema registry ACL enforcement.
Purpose: Provide an implementation-grade map of namespace enforcement and
         registry RBAC/ACL behavior with security invariants.
Dependencies:
  - decision-gate-config/src/config.rs
  - decision-gate-mcp/src/tools.rs
  - decision-gate-mcp/src/namespace_authority.rs
  - decision-gate-mcp/src/registry_acl.rs
  - decision-gate-mcp/src/auth.rs
  - decision-gate-mcp/src/server.rs
  - decision-gate-mcp/src/audit.rs
  - decision-gate-core/src/core/data_shape.rs
  - decision-gate-core/src/core/runpack.rs
  - decision-gate-store-sqlite/src/store.rs
  - Docs/configuration/decision-gate.toml.md
  - Docs/architecture/decision_gate_assetcore_integration_contract.md
============================================================================
Last Updated: 2026-01-31 (UTC)
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
  `namespace.default_tenants`.[F:decision-gate-config/src/config.rs L736-L765](decision-gate-config/src/config.rs#L736-L765)[F:decision-gate-mcp/src/tools.rs L1269-L1294](decision-gate-mcp/src/tools.rs#L1269-L1294)
- Asset Core integration is explicit and bounded to configured endpoints and
  timeouts; no implicit namespace mapping is allowed.[F:decision-gate-config/src/config.rs L829-L900](decision-gate-config/src/config.rs#L829-L900)[F:decision-gate-mcp/src/namespace_authority.rs L109-L133](decision-gate-mcp/src/namespace_authority.rs#L109-L133)
- Schema registry access is guarded by a dedicated Registry ACL layer that is
  **independent** of tool allowlists; both must allow the action.[F:decision-gate-mcp/src/registry_acl.rs L137-L186](decision-gate-mcp/src/registry_acl.rs#L137-L186)[F:decision-gate-mcp/src/tools.rs L1296-L1322](decision-gate-mcp/src/tools.rs#L1296-L1322)

Registry RBAC/ACL is scoped by **tenant + namespace + subject** and can be
configured as either builtin or custom. Builtin policy is explicitly defined by
role names with policy class gating for write access.[F:decision-gate-mcp/src/registry_acl.rs L189-L248](decision-gate-mcp/src/registry_acl.rs#L189-L248)

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
requests. The reserved namespace literal is **`default`** and must pass
additional checks before any operation proceeds.[F:decision-gate-mcp/src/tools.rs L120-L1294](decision-gate-mcp/src/tools.rs#L120-L1294)

### Schema Registry Records
Schema registry entries are immutable `DataShapeRecord` values. Each record is
scoped by tenant + namespace + schema id + version, and may include optional
signing metadata (key id, signature, optional algorithm).[F:decision-gate-core/src/core/data_shape.rs L32-L64](decision-gate-core/src/core/data_shape.rs#L32-L64)

---

## Namespace Policy

### Default Namespace Guard
The default namespace is a hard-coded reserved string (`"default"`). Its
behavior is intentionally narrow:

- If `namespace.allow_default` is false, **all** requests targeting the default
  namespace are rejected.
- If `namespace.allow_default` is true, `namespace.default_tenants` must be
  non-empty and the caller must supply a `tenant_id` in the allowlist.
- The default namespace guard is enforced **before** external authority checks.

Implementation:
- Config validation enforces the allowlist requirement.[F:decision-gate-config/src/config.rs L736-L765](decision-gate-config/src/config.rs#L736-L765)
- Tool router enforces the guard per request.[F:decision-gate-mcp/src/tools.rs L1269-L1294](decision-gate-mcp/src/tools.rs#L1269-L1294)

### Namespace Authority Modes
Namespace authority determines how DG validates namespace existence:

| Mode | Behavior | Source |
| --- | --- | --- |
| `none` | No external authority checks (DG-local namespace policy only) | `NamespaceAuthorityMode::None`[F:decision-gate-config/src/config.rs L767-L827](decision-gate-config/src/config.rs#L767-L827) |
| `assetcore_http` | Validate namespace via Asset Core write daemon HTTP API | `NamespaceAuthorityMode::AssetcoreHttp`[F:decision-gate-config/src/config.rs L767-L827](decision-gate-config/src/config.rs#L767-L827)[F:decision-gate-mcp/src/namespace_authority.rs L65-L187](decision-gate-mcp/src/namespace_authority.rs#L65-L187) |

When `assetcore_http` is enabled, DG validates namespaces by issuing a GET
request to `/{base_url}/v1/write/namespaces/{resolved_id}`. HTTP 200 = allowed;
404 or 401/403 = denied; other statuses and transport errors are treated as
unavailable (fail closed).[F:decision-gate-mcp/src/namespace_authority.rs L159-L186](decision-gate-mcp/src/namespace_authority.rs#L159-L186)

Asset Core authority requests can include an optional bearer token and an
`x-correlation-id` header derived from the **unsafe** client-provided
correlation header when available (falling back to the JSON-RPC request id or
server-issued correlation id). Client correlation IDs are strictly validated
and rejected when invalid; only sanitized values are forwarded to the namespace
authority to prevent header injection and log spoofing.[F:decision-gate-mcp/src/server.rs L1246-L1308](decision-gate-mcp/src/server.rs#L1246-L1308)[F:decision-gate-mcp/src/namespace_authority.rs L135-L200](decision-gate-mcp/src/namespace_authority.rs#L135-L200)

**Integration constraint:** dev-permissive mode is **disallowed** when
`namespace.authority.mode = assetcore_http` to avoid weakening namespace
security in integrated deployments.[F:decision-gate-config/src/config.rs L354-L377](decision-gate-config/src/config.rs#L354-L377)

### Asset Core Namespace Rules
Namespace identifiers are numeric everywhere (>= 1). Asset Core authority
validation is direct and does not apply any mapping or translation. Any parse
failure yields a namespace validation error (fail closed).[F:decision-gate-config/src/config.rs L829-L900](decision-gate-config/src/config.rs#L829-L900)[F:decision-gate-mcp/src/namespace_authority.rs L109-L133](decision-gate-mcp/src/namespace_authority.rs#L109-L133)

Config validation enforces required Asset Core settings (base URL, timeout
ranges) when Asset Core authority is enabled.[F:decision-gate-config/src/config.rs L829-L886](decision-gate-config/src/config.rs#L829-L886)

### Failure Posture
Namespace authority failures are mapped as follows:

- Invalid namespace input -> `InvalidParams` (caller error)
- Denied or unavailable authority -> `Unauthorized` (fail closed)

This ensures missing namespaces and upstream outages are treated as access
failures rather than allowed paths.[F:decision-gate-mcp/src/tools.rs L1495-L1502](decision-gate-mcp/src/tools.rs#L1495-L1502)

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
- Principal id derivation.[F:decision-gate-mcp/src/auth.rs L106-L141](decision-gate-mcp/src/auth.rs#L106-L141)
- Principal configuration and validation.[F:decision-gate-config/src/config.rs L565-L714](decision-gate-config/src/config.rs#L565-L714)
- Principal mapping resolver and role scoping logic.[F:decision-gate-mcp/src/registry_acl.rs L41-L333](decision-gate-mcp/src/registry_acl.rs#L41-L333)

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
SchemaManager writes).[F:decision-gate-mcp/src/registry_acl.rs L189-L248](decision-gate-mcp/src/registry_acl.rs#L189-L248)

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
[F:decision-gate-mcp/src/registry_acl.rs L250-L311](decision-gate-mcp/src/registry_acl.rs#L250-L311)[F:decision-gate-config/src/config.rs L1265-L1395](decision-gate-config/src/config.rs#L1265-L1395)

### Signing Requirement
Registry ACL can require schema signing metadata:

- `schema_registry.acl.require_signing = true` enforces presence of `signing`
  metadata on schema records.
- Missing or empty signing metadata is rejected as unauthorized before registry
  mutation.[F:decision-gate-config/src/config.rs L1355-L1394](decision-gate-config/src/config.rs#L1355-L1394)[F:decision-gate-mcp/src/tools.rs L1324-L1336](decision-gate-mcp/src/tools.rs#L1324-L1336)

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
fallbacks or bypasses.[F:decision-gate-mcp/src/tools.rs L1269-L1294](decision-gate-mcp/src/tools.rs#L1269-L1294)

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
- Registry ACL enforcement + auditing.[F:decision-gate-mcp/src/tools.rs L1296-L1366](decision-gate-mcp/src/tools.rs#L1296-L1366)
- Registry ACL evaluator (builtin/custom).[F:decision-gate-mcp/src/registry_acl.rs L137-L333](decision-gate-mcp/src/registry_acl.rs#L137-L333)

---

## Audit and Observability

DG emits explicit audit records for registry access decisions and security
posture changes:

- `RegistryAuditEvent` captures tenant, namespace, action, allow/deny decision,
  reason, principal roles, schema identity, and correlation identifiers (unsafe
  client + server-issued) for audit traceability.[F:decision-gate-mcp/src/audit.rs L104-L141](decision-gate-mcp/src/audit.rs#L104-L141)
- `SecurityAuditEvent` records dev-permissive activation (and invalid
  correlation rejections) along with namespace authority posture; correlation
  identifiers are included when the event is tied to a request.
  [F:decision-gate-mcp/src/audit.rs L186-L207](decision-gate-mcp/src/audit.rs#L186-L207)[F:decision-gate-mcp/src/server.rs L1235-L1260](decision-gate-mcp/src/server.rs#L1235-L1260)
- Runpack exports embed `RunpackSecurityContext` with dev-permissive and
  namespace authority metadata, making security posture verifiable offline.
  [F:decision-gate-core/src/core/runpack.rs L70-L92](decision-gate-core/src/core/runpack.rs#L70-L92)[F:decision-gate-mcp/src/server.rs L362-L380](decision-gate-mcp/src/server.rs#L362-L380)

---

## Storage and Persistence

Schema registry records include signing metadata end-to-end:

- `DataShapeRecord` includes optional signing fields.
- SQLite registry persists signing metadata in dedicated columns and migrates
  schema version 3 -> 4 to add the signing columns.

References:
- Data shape type definition.[F:decision-gate-core/src/core/data_shape.rs L32-L64](decision-gate-core/src/core/data_shape.rs#L32-L64)
- SQLite registry storage and migration.[F:decision-gate-store-sqlite/src/store.rs L250-L360](decision-gate-store-sqlite/src/store.rs#L250-L360)[F:decision-gate-store-sqlite/src/store.rs L640-L708](decision-gate-store-sqlite/src/store.rs#L640-L708)

---

## Security Invariants

1. **Fail-closed namespace enforcement:** Invalid, unknown, or unreachable
   namespace authority always denies access.
   [F:decision-gate-mcp/src/namespace_authority.rs L159-L205](decision-gate-mcp/src/namespace_authority.rs#L159-L205)[F:decision-gate-mcp/src/tools.rs L1495-L1502](decision-gate-mcp/src/tools.rs#L1495-L1502)
2. **No implicit default namespace:** `default` requires explicit allowlist and
   tenant match; otherwise denied.
   [F:decision-gate-config/src/config.rs L736-L765](decision-gate-config/src/config.rs#L736-L765)[F:decision-gate-mcp/src/tools.rs L1269-L1294](decision-gate-mcp/src/tools.rs#L1269-L1294)
3. **Asset Core integration is strict:** Mapping mode cannot be `none`, and
   dev-permissive is disallowed when using Asset Core authority.
   [F:decision-gate-config/src/config.rs L354-L377](decision-gate-config/src/config.rs#L354-L377)[F:decision-gate-config/src/config.rs L829-L900](decision-gate-config/src/config.rs#L829-L900)
4. **Registry ACL is authoritative:** Tool allowlists do not bypass registry
   ACL; registry access is enforced and audited for every registry action.
   [F:decision-gate-mcp/src/tools.rs L1296-L1366](decision-gate-mcp/src/tools.rs#L1296-L1366)
5. **Local-only registry access is explicit:** `schema_registry.acl.allow_local_only`
   defaults to `false`. When enabled, the built-in ACL can allow loopback/stdio
   subjects to bypass principal mapping; this does not apply to custom ACL rules.
   [F:decision-gate-config/src/config.rs L1478-L1506](decision-gate-config/src/config.rs#L1478-L1506)[F:decision-gate-mcp/src/registry_acl.rs L168-L230](decision-gate-mcp/src/registry_acl.rs#L168-L230)
6. **Schema signing enforcement is explicit:** When enabled, signing metadata
   is mandatory for registry writes.
   [F:decision-gate-mcp/src/tools.rs L1324-L1336](decision-gate-mcp/src/tools.rs#L1324-L1336)

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
| Namespace config + validation | `decision-gate-config/src/config.rs` | Namespace policy + Asset Core authority config and validation. |
| Default namespace + authority enforcement | `decision-gate-mcp/src/tools.rs` | `ensure_namespace_allowed` and namespace error mapping. |
| Namespace authority integration | `decision-gate-mcp/src/namespace_authority.rs` | HTTP validation, mapping rules, fail-closed semantics. |
| Registry ACL engine | `decision-gate-mcp/src/registry_acl.rs` | Principal mapping + builtin/custom ACL evaluation. |
| Registry ACL enforcement | `decision-gate-mcp/src/tools.rs` | `ensure_registry_access` + audit emission + signing checks. |
| Auth principal identifiers | `decision-gate-mcp/src/auth.rs` | Stable principal ids for ACL mapping. |
| Audit event schemas | `decision-gate-mcp/src/audit.rs` | Registry + security audit payloads. |
| Runpack security context | `decision-gate-core/src/core/runpack.rs` | Security metadata embedded in runpacks. |
| Schema registry persistence | `decision-gate-store-sqlite/src/store.rs` | Signing metadata columns and migrations. |

---

## Related Documentation

- `Docs/configuration/decision-gate.toml.md`
- `Docs/architecture/decision_gate_assetcore_integration_contract.md`
- `Docs/guides/security_guide.md`
