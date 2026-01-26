<!--
Docs/roadmap/enterprise/enterprise_phasing_plan.md
============================================================================
Document: Decision Gate Enterprise/Cloud Phasing Plan
Description: Mechanical roadmap from OSS kernel to paid cloud and enterprise tiers.
Purpose: Track concrete implementation steps per phase with repo structure guidance.
Dependencies:
  - Docs/business/decision_gate_product_phasing_open_core_vs_enterprise_and_initial_cloud_strategy.md
  - Docs/business/open_core_strategy.md
  - Docs/architecture/decision_gate_namespace_registry_rbac_architecture.md
  - decision-gate-mcp/src/auth.rs
  - decision-gate-mcp/src/tools.rs
  - decision-gate-mcp/src/server.rs
  - decision-gate-core/src/interfaces/mod.rs
============================================================================
-->

# Decision Gate Enterprise/Cloud Phasing Plan

## Status
Draft. This plan is the authoritative task tracker for moving from OSS kernel to
paid cloud and enterprise tiers.

## Scope
- Define what is required to reach Phase 1/2/3 in mechanical terms.
- Identify implementation seams so enterprise features can live in private crates.
- Keep OSS trust surface intact.

## Current State (Phase 0 Snapshot)
Phase 0 (OSS kernel) is largely complete:
- Deterministic engine, RET logic, runpacks, CLI, MCP server, provider SDKs.
- Namespace authority + registry ACL + audit logging exist.
- Storage is memory/SQLite; ops are single-node.

Primary gaps for monetization:
- Tenant-aware authz enforcement (beyond schema registry ACL).
- Usage metering + quotas.
- Tenant provisioning and basic hosted UI.

## Design Principles
- Enterprise features are operational/organizational, not semantic.
- Keep OSS core deterministic and auditable.
- Add enterprise via explicit seams and configuration, not forked semantics.

## Phase 1 — DG Cloud (Developer Tier)
Goal: first paid tier with minimal ops overhead.

### Must-Have Capabilities
- Tenant provisioning (create tenant + API key + namespaces).
- Tenant-bound authz: map tokens/subjects to tenant + namespace and enforce on all tools.
- Usage metering: tool calls, evidence queries, run starts, runpack exports.
- Quotas: hard limits per tenant (requests, storage, runpacks, schemas).
- Hosted run state store (durable, multi-tenant).
- Hosted registry + runpack storage.
- Minimal UI: list runs, download runpacks, rotate API keys.

### Mechanical Workstreams
1) **Identity + Tenant Binding**
   - Introduce a `TenantResolver` (or equivalent) that maps auth context to allowed
     tenant(s)/namespace(s), not just tool allowlists.
   - Enforce tenant/namespace on all `decision-gate-mcp` tool calls.
   - Wire into `decision-gate-mcp/src/tools.rs` before any state mutation.

2) **Usage Metering + Quotas**
   - Add a `UsageSink` trait (or equivalent) and emit counters per tool action.
   - Add quota checks at the router layer; reject when exceeded.
   - Persist usage to a multi-tenant store.

3) **Durable Stores**
   - Implement enterprise run store + schema registry backends using the existing
     `RunStateStore` and `DataShapeRegistry` traits (`decision-gate-core/src/interfaces/mod.rs`).
   - Add object storage for runpacks (or blob store abstraction).

4) **Hosted UX + Tenant Ops**
   - Add minimal management API (or admin-only tool surface) for tenant lifecycle.
   - Build a barebones UI around those APIs.

### Exit Criteria
- Tenant isolation enforced for all tool calls.
- Metering + quotas enforced for at least: tool calls, runpack exports, schemas.
- Hosted storage works end-to-end for a multi-tenant deployment.

## Phase 2 — DG Cloud (Org / Compliance Tier)
Goal: higher ACV via org features without bespoke consulting.

### Must-Have Capabilities
- Org accounts + role-based access (org admin, project admin, auditor).
- SSO (OIDC/SAML) + SCIM or basic user provisioning.
- Retention policies and legal-hold flags.
- Audit dashboards and export (SIEM-ready).
- SLA tiers + reliability targets.

### Mechanical Workstreams
1) **Org/Role Model**
   - Add org/user model in enterprise auth service.
   - Bind roles to tenants/namespaces and map into tool authz.

2) **SSO Integration**
   - Add OIDC/SAML provider config + token validation.
   - Map identity claims to org roles.

3) **Retention + Audit Export**
   - Add retention timers for run state, runpacks, logs.
   - Add export pipelines for audit events.

### Exit Criteria
- Org + SSO working end-to-end.
- Audit export and retention policies verified.
- Access control covers tools, runpacks, and registry.

## Phase 3 — Premium / AssetCore Tier
Goal: high-stakes deployments with immutable audit and optional on-prem.

### Must-Have Capabilities
- AssetCore-anchored runs (already supported at evidence level).
- Immutable log backend (WORM, append-only store, or AssetCore anchoring).
- Attestation chains for runpacks.
- On-prem / air-gapped SKU packaging.
- FIPS builds / compliance hardening.

### Mechanical Workstreams
1) **Immutable Logs + Attestations**
   - Add an `AuditSink` implementation that writes to WORM/append-only storage.
   - Extend runpack manifest with attestation references (if needed).

2) **Packaging**
   - Produce hardened builds, minimal images, config baselines.

### Exit Criteria
- Immutable audit logging verified.
- Attestation chain verifiable offline.
- On-prem deployment reproducible.

## Repo Structure Strategy (Private Now, Open Later)

### Recommended Structure (Now)
- Keep enterprise code in a separate top-level folder (private):
  - `enterprise/decision-gate-enterprise` (authz, tenant mgmt, usage, metering)
  - `enterprise/decision-gate-store-enterprise` (Postgres/Redis/S3 backends)
  - `enterprise/enterprise-system-tests`
- Add these as workspace members while the repo is private.
- Avoid changing OSS semantics; only implement interfaces or extension hooks.

### Seam Locations to Reuse
- Authz: `decision-gate-mcp/src/auth.rs` (add a pluggable authz implementation).
- Tool routing: `decision-gate-mcp/src/tools.rs` (pre-dispatch checks).
- Stores: `decision-gate-core/src/interfaces/mod.rs` (store + registry traits).
- Audit: `decision-gate-mcp/src/audit.rs` (sink implementations).

### Public Split Plan (Later)
- Move `enterprise/*` into a private repo.
- OSS repo depends on enterprise crates only via optional feature flags or
  external integration points (no hard dependency).
- Private repo depends on released OSS crates via git or internal registry.

## Testing Strategy
- Keep `system-tests` open and OSS-only.
- Add `enterprise-system-tests` for private features (tenant mgmt, SSO, quotas).
- If repo goes public, move enterprise tests to private repo.

## Next Actions (Short-Term)
1) Add tenant-bound authz enforcement (Phase 1 blocker).
2) Define a usage/metering data model and plug it into MCP tool calls.
3) Create enterprise crate skeletons and wire them behind traits.

