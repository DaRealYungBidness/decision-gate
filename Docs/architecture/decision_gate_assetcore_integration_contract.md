<!--
Docs/architecture/decision_gate_assetcore_integration_contract.md
============================================================================
Document: Decision Gate + Asset Core Integration Contract
Description: Canonical boundary contract for namespace, auth, and evidence anchors.
Purpose: Define the exact DG/ASC integration rules without code coupling.
Dependencies:
  - Docs/roadmap/asc_dg_alignment_engineering_now.md
  - Docs/security/threat_model.md
  - Docs/guides/assetcore_interop_runbook.md
============================================================================
-->

# Decision Gate + Asset Core Integration Contract

## Overview
This document is the canonical, implementation-ready contract for integrating
Decision Gate (DG) with Asset Core (ASC). It defines the boundaries, authority,
and evidence semantics needed to preserve independence while enabling
high-confidence overlap.

**Design intent**: DG remains a standalone control plane. ASC remains a standalone
world-state substrate. Integration is optional and explicit.

## Non-Negotiable Boundaries
- **No code coupling**: DG does not link against ASC crates or share internal APIs.
- **No write-path gating (v1)**: DG must not sit on ASC write paths.
- **Fail-closed trust boundaries**: Missing or unauthorized namespaces must deny.
- **Determinism first**: Same inputs yield identical outputs and runpacks.

## Namespace Authority
### Source of Truth
- **Integrated deployments**: ASC namespace catalog is authoritative.
- **Standalone DG deployments**: DG registry is authoritative.
Dev-permissive is **disallowed** when `namespace.authority.mode = "assetcore_http"`.

### Validation Rules
- DG must validate `namespace_id` for every tool call and evidence query.
- Unknown namespace -> **fail closed**.
- Catalog unreachable -> **fail closed**.

### Namespace ID Rules
- DG accepts only numeric namespace IDs (>= 1).
- Asset Core authority validation is direct; no mapping table or translation.

## Auth/RBAC Mapping
### Integration Layer
DG must not parse ASC tokens or share ASC auth internals. A narrow integration
layer verifies ASC authentication and forwards a **minimal PrincipalContext**:
- `tenant_id`
- `principal_id`
- `roles`
- `policy_class`
- `groups`

### Default Posture
Auth defaults are conservative: no scenario authoring or registry writes unless
explicitly granted in the mapping matrix.

### Mapping Matrix (Default)
The integration layer maps ASC roles + policy class into a DG tool allowlist.
This mapping is **fail-closed**: missing or unknown roles grant no access.

**DG tool groups (reference):**
- **Authoring**: `scenario_define`, `schemas_register`
- **Run operations**: `scenario_start`, `scenario_trigger`, `scenario_next`,
  `scenario_submit`, `precheck`
- **Read-only**: `scenario_status`, `scenarios_list`, `schemas_list`,
  `schemas_get`, `providers_list`, `evidence_query`
- **Audit**: `runpack_export`, `runpack_verify`

**Default mapping (recommended):**

| ASC Role | Policy Class Restriction | DG Tool Groups |
| --- | --- | --- |
| TenantAdmin | None | Authoring + Run operations + Read-only + Audit |
| NamespaceOwner | None | Authoring + Run operations + Read-only + Audit |
| NamespaceAdmin | None | Authoring + Run operations + Read-only + Audit |
| NamespaceWriter | None | Run operations + Read-only + Audit (verify only) |
| NamespaceReader | None | Read-only + Audit (verify only) |
| SchemaManager | Scratch, Project only | Authoring (schemas only) + Read-only |
| AgentSandbox | Scratch only | Run operations + Read-only |
| NamespaceDeleteAdmin | None | Read-only |

**Policy class enforcement:**
- If `policy_class` is `prod`, **SchemaManager** and **AgentSandbox** do not
  grant any DG permissions (ASC already restricts these roles).
- Unknown or missing `policy_class` -> deny.

**Audit note:** `runpack_verify` is safe to allow broadly; `runpack_export`
should be restricted to admin/owner roles unless explicitly granted.

## Registry ACL (DG Internal)
Schema registry access is enforced by a **DG-internal ACL layer** that runs
after the integration-layer tool allowlist. This is intentionally separate:

- **RBAC mapping** (integration) decides which tools are callable.
- **Registry ACL** (DG) decides who may `schemas_register/list/get` per
  tenant/namespace, using principals and policy class metadata.

Built-in ACL defaults:
- Read: NamespaceReader+ within namespace.
- Write: NamespaceAdmin/TenantAdmin only.
- SchemaManager may write only when `policy_class` is non-prod.

Integration implementations must not assume tool allowlists imply registry
write permission; both layers must allow the operation.

## Evidence Anchors (ASC Canon)
All ASC-backed evidence **must** include the following anchors:
- `assetcore.namespace_id`
- `assetcore.commit_id`
- `assetcore.world_seq`

Optional upgrade anchor:
- `assetcore.chain_hash` (when ASC enables chain hashing)

These anchors must be captured in runpacks and used by offline verification.

## Evidence Provider Contract
### Required Inputs
- `namespace_id` (DG)
- ASC namespace identifier (mapped or numeric)
- Check parameters (container_id, class_id, etc.)

### Required Outputs
- Evidence value or hash
- Evidence hash (canonical JSON)
- Evidence anchor (ASC anchor set above)
- Content type
- Determinism class (must be deterministic for ASC reads)

### Limits and Failures
- Strict timeouts; retry only on transport errors.
- Timeout or provider error -> `Unknown` (fail closed).

## Correlation IDs
### End-to-End Requirements
- DG must propagate the client correlation ID to ASC evidence queries.
- DG must emit server correlation IDs in audit logs and runpacks.
- Correlation IDs are logged and captured in tool transcripts.

### Sanitization
Client-provided correlation IDs are **unsafe** and strictly validated at
ingress. Invalid values are rejected; only sanitized IDs propagate to ASC
providers. Server-issued correlation IDs are always present for audit trails.

## Runpack Verification
Runpack verification must confirm:
- All ASC evidence entries include required anchors.
- Anchor values are well-formed and match the expected schema.
- Optional `assetcore.chain_hash` is present when configured as required.

## Out of Scope (v1)
- ASC write-path gating by DG.
- ASC namespace creation from DG.
- ASC state mutation via DG.
