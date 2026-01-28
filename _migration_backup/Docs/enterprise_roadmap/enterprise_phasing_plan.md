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
Phase 1 implementation and hardening coverage are now complete in code. The
enterprise system-test suite has been fully enumerated and wired, but has not
yet been executed in this workspace (run via `scripts/test_runner.py` or
`cargo test -p enterprise-system-tests --features enterprise-system-tests`). This plan remains the authoritative
task tracker for moving from OSS kernel to paid cloud and enterprise tiers.

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
Status: **Complete (Phase‑1 scaffolding + enforcement seams implemented).**

### Must-Have Capabilities
- Tenant provisioning (create tenant + API key + namespaces).
- Tenant-bound authz: map tokens/subjects to tenant + namespace and enforce on all tools.
- Usage metering: tool calls, evidence queries, run starts, runpack exports.
- Quotas: hard limits per tenant (requests, storage, runpacks, schemas).
- Hosted run state store (durable, multi-tenant).
- Hosted registry + runpack storage.
- Minimal UI: list runs, download runpacks, rotate API keys.

### Phase-1 Ordering (Do These In This Order)
1) **Upfront decisions (lock the invariants)**
   - Tenant model: `org -> tenant -> namespace` or `tenant -> namespace` only.
   - Identity binding: what a token/subject maps to (single tenant vs multi-tenant).
   - Authz source of truth: local config vs external auth service (OIDC/JWT vs opaque tokens).
   - Quota semantics: what is billable (requests, runs, evidence, storage, runpacks).
   - Storage boundaries: per-tenant database vs shared DB with strict row-level isolation.
   - Audit retention policy: immutable log retention and export format.
   - Threat model scope: rate-limit evasion, tenant-hopping, replay, and log tampering.

2) **Tenant-bound authz (Phase-1 blocker)**
   - Introduce a `TenantResolver` (or equivalent) that maps auth context to allowed
     tenant(s)/namespace(s), not just tool allowlists.
   - Enforce tenant/namespace on all `decision-gate-mcp` tool calls.
   - Wire into `decision-gate-mcp/src/tools.rs` before any state mutation.
   - Emit audit events for tenant mismatch denials.
   - Status: **Implemented (OSS seam + audit event)**.
     - References: `decision-gate-mcp/src/tenant_authz.rs`,
       `decision-gate-mcp/src/tools.rs`,
       `decision-gate-mcp/src/audit.rs`,
       `decision-gate-mcp/src/server.rs`,
       `decision-gate-mcp/tests/tenant_authz.rs`,
       `enterprise/decision-gate-enterprise/src/tenant_authz.rs`,
       `Docs/architecture/decision_gate_auth_disclosure_architecture.md`.

3) **Usage metering + quota enforcement**
   - Add a `UsageSink` trait (or equivalent) and emit counters per tool action.
   - Add quota checks at the router layer; reject when exceeded.
   - Persist usage to a multi-tenant store with strict isolation.
   - Decide whether rate limiting is per-tenant or per-token (or both).
   - Status: **Implemented (OSS seam + enterprise enforcer scaffolding)**.
     - References: `decision-gate-mcp/src/usage.rs`,
       `decision-gate-mcp/src/tools.rs`,
       `decision-gate-mcp/src/audit.rs`,
       `enterprise/decision-gate-enterprise/src/usage.rs`,
       `enterprise/decision-gate-enterprise/src/usage_sqlite.rs`,
       `decision-gate-mcp/tests/usage_meter.rs`.

4) **Durable stores (multi-tenant safe)**
   - Implement enterprise run store + schema registry backends using the existing
     `RunStateStore` and `DataShapeRegistry` traits (`decision-gate-core/src/interfaces/mod.rs`).
   - Add object storage for runpacks (or blob store abstraction) with per-tenant prefixes.
   - Add backup/restore and corruption detection hooks for auditability.
   - Add server-level overrides to wire enterprise stores without OSS deps.
   - Provide enterprise helper to build MCP server with Postgres store wiring.
   - Status: **Implemented (Postgres store + S3 runpack store; config wiring + MCP export integration + runbook added)**.
      - References: `enterprise/decision-gate-store-enterprise/src/sqlite_store.rs`,
        `enterprise/decision-gate-store-enterprise/src/runpack_store.rs`,
        `enterprise/decision-gate-store-enterprise/src/postgres_store.rs`,
        `enterprise/decision-gate-store-enterprise/src/s3_runpack_store.rs`,
       `enterprise/decision-gate-enterprise/src/runpack_storage.rs`,
       `enterprise/decision-gate-enterprise/src/config.rs`,
       `examples/decision-gate-enterprise.toml`,
        `enterprise/decision-gate-enterprise/src/server.rs`,
        `decision-gate-mcp/src/server.rs`,
        `decision-gate-mcp/src/tools.rs`,
        `decision-gate-mcp/src/runpack_storage.rs`,
        `Docs/roadmap/enterprise/enterprise_backup_restore_runbook.md`.

5) **Audit immutability + export scaffolding**
   - Add append-only, hash-chained audit sink for tamper-evident logs.
   - Provide export-friendly JSONL format.
   - Status: **Implemented (enterprise sink)**.
     - References: `enterprise/decision-gate-enterprise/src/audit_chain.rs`.

6) **Tenant lifecycle + minimal UI**
   - Add minimal management API (or admin-only tool surface) for tenant lifecycle.
   - Build a barebones UI around those APIs (list runs, download runpacks, rotate keys).
   - Status: **Implemented (enterprise scaffolding)**.
     - References: `enterprise/decision-gate-enterprise/src/tenant_admin.rs`,
       `enterprise/decision-gate-enterprise/src/admin_ui.rs`,
       `enterprise/decision-gate-enterprise/src/server.rs`.

### Phase-1 Hardening (System-Tests + Integrity)
Status: **Implemented (tests added; execution validated for non-container suites; Docker-backed suites pending).**

- **Execution status (local):** passing `audit`, `config_limits`, `runpack_hardening`,
  `tenant_authz`, `transport_parity`, `transport_tls`, `usage`.
- **Execution status (Docker required):** `backup_restore`, `tenant_isolation`,
  `postgres_store`, `s3_runpack_store`, `config_wiring` (requires container runtime).

- **Enterprise test registry + gap closure:** `enterprise/enterprise-system-tests/test_registry.toml`,
  `enterprise/enterprise-system-tests/test_gaps.toml`.
- **Audit integrity + JSONL export:** `enterprise/enterprise-system-tests/tests/suites/audit.rs`
  (hash-chain immutability, deny-path coverage, JSONL format).
- **Postgres store integrity:** `enterprise/enterprise-system-tests/tests/suites/postgres_store.rs`
  (roundtrip, corruption detection, concurrency, pagination, signing metadata).
- **S3 runpack integrity:** `enterprise/enterprise-system-tests/tests/suites/s3_runpack_store.rs`
  (roundtrip, SSE enforcement, metadata tamper detection, archive hardening).
- **Runpack export hygiene:** `enterprise/enterprise-system-tests/tests/suites/runpack_hardening.rs`
  (temporary directory cleanup).
- **Transport parity + TLS/mTLS:** `enterprise/enterprise-system-tests/tests/suites/transport_parity.rs`,
  `enterprise/enterprise-system-tests/tests/suites/transport_tls.rs`.
- **Backup/restore validation:** `enterprise/enterprise-system-tests/tests/suites/backup_restore.rs`.
- **Config hardening:** `enterprise/enterprise-system-tests/tests/suites/config_limits.rs`.

### Phase-1 Design Decisions (Make These Upfront)
These are the “lock early” choices that avoid long-term pain:
- **Auth model**: JWT/OIDC vs opaque tokens; claim format for tenant/namespace scoping.
- **Isolation model**: per-tenant DB/schema vs shared DB with hard partition keys.
- **Rate limiting vs quotas**: rate limit is DOS protection; quotas are billing controls.
- **Usage counters**: define the canonical counters and make them consistent across tools.
- **Audit immutability**: whether audit logs are append-only and tamper-evident from day one.
- **Runpack storage**: object store layout and retention strategy (per-tenant WORM option).
- **Namespace authority**: stay `none` for DG Cloud or enforce external authority immediately.
- **Backwards compatibility**: define how API and schema changes are versioned.

### Phase-1 Security Invariants (Non-Negotiable)
- Fail-closed for all tenant/namespace mismatches.
- No implicit tenant access via tool allowlists.
- All authz denials are audited.
- Usage metering cannot be disabled in production configs.

### Phase-1 Decision Matrix (Decided Defaults)
Status: **Decided.** These defaults are now the authoritative Phase‑1 choices.
They are optimized for world-class security, long-term scale, and clean
separation between OSS and enterprise add-ons.

- **Auth model (decided)**: OIDC/JWT for user sessions + opaque API keys for
  service access. JWTs are short-lived and verified via JWKS; API keys are hashed
  and mapped to a principal profile with explicit tenant/namespace scopes.
  Rationale: best-of-breed identity integration + safe machine access without
  leaking tenant scope in bearer tokens.

- **Tenant model (decided)**: `org -> tenant -> namespace`, with tenant as
  the billing and isolation unit. Default tokens are single-tenant; multi-tenant
  tokens are explicitly scoped and audited.
  Rationale: aligns with future org/SSO requirements without breaking Phase 1.

- **Isolation model (decided)**: shared Postgres with strict partition keys
  + row-level security (RLS) enforced at the DB layer; optional per-tenant DB for
  premium tiers later.
  Rationale: immediate multi-tenant safety with a migration path to stronger
  physical isolation.

- **Usage counters (decided)**: append-only usage ledger with idempotent
  events keyed by request_id/run_id and tool action. Canonical counters:
  tool_calls, runs_started, evidence_queries, runpack_exports, schemas_written,
  storage_bytes, registry_entries.
  Rationale: deterministic billing + audit-friendly reconciliation.

- **Quotas vs rate limits (decided)**: enforce both. Rate limits per token
  and per tenant at the edge; quotas enforced via the usage ledger with hard
  denial when exceeded.
  Rationale: protects availability and enforces billing/contracts independently.

- **Audit immutability (decided)**: append-only audit log with hash chaining
  (Merkle or simple hash chain) and periodic snapshots in object storage.
  Rationale: tamper-evident by default, enterprise-ready without refactor.

- **Runpack storage (decided)**: object storage (S3-compatible) with
  per-tenant prefixing, server-side encryption, and retention policies. Support
  WORM/immutable buckets when enabled.
  Rationale: scalable, inexpensive, and compliant storage pattern.

- **Namespace authority (decided)**: DG Cloud starts with internal authority
  (no external dependency), but keep the authority interface pluggable for
  AssetCore or external catalogs.
  Rationale: minimizes Phase-1 ops risk while preserving enterprise path.

- **API/versioning (decided)**: explicit versioning of tool schemas and
  contract artifacts; changes gated by compatibility tests. Avoid breaking
  changes in tool shapes; add new tools instead.
  Rationale: prevents tenant breakage and keeps generated docs stable.

- **Secrets + key management (decided)**: store only hashed API keys; use
  KMS-backed encryption for stored secrets and audit every key use. Design for
  HSM integration later without changing interfaces.
  Rationale: eliminates plaintext risk and enables compliance later.

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
