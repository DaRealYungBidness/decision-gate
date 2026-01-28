<!--
Docs/roadmap/enterprise/decision_gate_enterprise/decision_gate_enterprise_world_class_roadmap.md
============================================================================
Document: Decision Gate Enterprise World-Class Roadmap (DG + Asset Core)
Description: Defines a world-class DG enterprise target and maps Asset Core
             platform crates to DG-E capabilities for a unified roadmap.
Purpose: Single source of truth for DG enterprise scope, platform alignment,
         and migration sequencing across DG OSS and the Asset Core monorepo.
Dependencies:
  - Docs/roadmap/enterprise/enterprise_phasing_plan.md
  - Docs/roadmap/enterprise/decision_gate_enterprise/decision_gate_enterprise_migration_playbook.md
  - Docs/business/decision_gate_product_phasing_open_core_vs_enterprise_and_initial_cloud_strategy.md
  - enterprise/decision-gate-enterprise/README.md
  - enterprise/decision-gate-store-enterprise/README.md
  - enterprise/enterprise-system-tests/README.md
  - /mnt/c/Users/Micha/Documents/GitHub/Asset-Core/README.md
============================================================================
Last Updated: 2026-01-27 (UTC)
============================================================================
-->

# Decision Gate Enterprise World-Class Roadmap (DG + Asset Core)

## Strategic Objectives

1. **One platform, many services**: Build a single enterprise platform layer
   that serves Decision Gate (DG), Asset Core (ASC), and future services.
2. **OSS integrity**: Keep DG OSS clean, deterministic, and auditable while
   enabling enterprise capabilities via explicit seams.
3. **Closed-loop correctness**: Preserve the invariance-driven refactor loop
   (rapid iteration + deterministic propagation) inside the private monorepo.
4. **World-class enterprise posture**: Treat security, auditability, and
   reliability as foundational invariants, not optional add-ons.
5. **Inevitability via coherence**: Make the DG + ASC ecosystem speak a single
   operational language (tenants, namespaces, auth, quotas, audit, telemetry),
   so new services naturally align without rework.

## Repo Split (Operational Strategy)

**Two-repo model (authoritative):**

- **Repo A: Decision Gate OSS** (public/open-core)
  - Self-contained, publishable crates.
  - Stable interfaces (traits + config contracts).
  - No dependency on enterprise or Asset Core crates.

- **Repo B: Asset Core Monorepo** (private)
  - Asset Core services and platform crates.
  - DG enterprise (DG-E) + enterprise system tests.
  - Shared enterprise platform layer used by DG-E and ASC.

**Why this matters operationally:**

- **Iteration speed**: Private monorepo preserves the closed-loop refactor
  advantage (fix once, validate across services).
- **Correctness guarantees**: Shared platform crates enforce consistent
  semantics and reduce drift.
- **OSS stability**: DG OSS remains decoupled and versioned with explicit seams.
- **Future-proofing**: Adding service N+1 becomes integration, not reinvention.

## Definitions (Shared Vocabulary)

- **DG OSS**: Public Decision Gate crates (core semantics, MCP tooling).
- **DG-E**: Private enterprise deployment of Decision Gate.
- **ASC**: Asset Core platform (deterministic world-state substrate).
- **Platform Layer**: Shared enterprise primitives (auth, quota, audit, etc.).
- **Seams**: Explicit traits/interfaces in DG OSS for enterprise overrides.

## Related Roadmaps

- `Docs/roadmap/enterprise/decision_gate_enterprise/decision_gate_enterprise_migration_playbook.md`

---

# Part I — What “World-Class DG Enterprise” Means

This is the full target state for a finished enterprise deployment of DG.
It is intentionally exhaustive.

## A) Identity, AuthN, and AuthZ

- OIDC/JWKS validation with rotation.
- API key issuance, hashing, revocation, rotation.
- Principal mapping (user/service/token) with stable subject IDs.
- Tenant + namespace authoritative registry.
- Role model (org admin, project admin, auditor, service).
- Policy engine for tool-level and registry-level enforcement.
- Authz decisions always audited (allow/deny).

## B) Usage, Quotas, and Billing

- Idempotent usage ledger.
- Quota enforcement before mutation (fail-closed).
- Dual rate limiting (tenant + token).
- Canonical usage counters:
  - tool_calls, runs_started, evidence_queries, runpack_exports,
    schemas_written, registry_entries, storage_bytes.
- Billing export pipeline + plan entitlements + overage rules.

## C) Audit, Compliance, and Retention

- Hash-chained audit sink (tamper-evident).
- Exportable JSONL audit format.
- Retention policies with legal hold support.
- SIEM/export pipeline.
- Timestamp source integrity (trusted time).

## D) Storage and Integrity

- Durable run state store (Postgres) with hash verification.
- Durable schema registry store (Postgres) with signing metadata.
- Runpack storage (S3/WORM) with hardening:
  - path traversal rejection
  - symlink/special entry rejection
  - size limits
  - metadata hash verification
- Backup/restore runbook + corruption detection.

## E) Service Interfaces and Admin UX

- MCP server (HTTP/SSE/stdio) with tenant isolation.
- Minimal admin UI:
  - list runs
  - download runpacks
  - rotate keys
  - view audit exports
- Tenant + namespace lifecycle APIs.

## F) Security Hardening

- TLS/mTLS enforcement.
- Strict comparator validation.
- Fail-closed everywhere (tenant, quota, authz, registry ACL).
- Config path hardening + size limits.
- Dev-permissive mode fenced off from production.

## G) Operations + Reliability

- Multi-AZ DB and object storage replication.
- Horizontal scaling for MCP layer (stateless).
- Metrics, alerting, and SLOs.
- Incident runbooks + debug bundle workflow.

## H) Premium / Phase-3 Enhancements (Optional)

- Immutable log backend (WORM or AssetCore anchoring).
- Runpack attestations and offline verification chains.
- Air-gapped/on-prem packaging.
- FIPS builds and HSM integration.

---

# Part II — Asset Core Platform Coverage Map

This section maps the world-class DG-E target to existing ASC crates.
(Per Asset Core README status legend: Production-ready / Operational / Foundation / Planned.)

## A) Platform Primitives (Reusable by DG-E)

| Capability | Asset Core Crate(s) | Status (ASC) | Notes for DG-E |
| --- | --- | --- | --- |
| OIDC/JWKS validation | `assetcore-oidc` | Operational | Reuse for DG-E authn verification. |
| Auth/RBAC policy model | `assetcore-auth` | Operational | DG-E should align roles/permissions to ASC policy model. |
| Quota ledger (atomic check+consume) | `assetcore-quota-backends` | Operational | Replace DG-E sqlite usage ledger in enterprise deployments. |
| Namespace catalog | `assetcore-namespace-catalog` | Operational | Authoritative tenant/namespace registry for DG-E. |
| Telemetry conventions | `assetcore-telemetry` | Foundation | Reuse canonical metric names + RED metrics; needs stability hardening. |
| Correlation IDs | `assetcore-correlation` | Operational | Standardize request ID format across services. |
| Config policy + redaction | `assetcore-config-runtime` | Operational | Align config provenance, redaction, and validation. |

## B) ASC Services (Not Directly Reused, but Aligned Semantics)

| Capability | ASC Service | Status (ASC) | Alignment Need |
| --- | --- | --- | --- |
| Commit log + replay | `daemon-write`, `daemon-read` | Operational/Production-ready | DG audit chain should mirror integrity expectations. |
| Adapter ecosystem | `assetcore-adapters` | Production-ready | DG MCP conventions should remain compatible. |
| System tests | `system-tests` | Operational | Align testing disciplines and coverage reporting. |

---

# Part III — Current DG-E Implementation (Today)

**Implemented in DG-E (Decision Gate repo):**

- Tenant authorizer seam + enforcement in OSS:
  - `decision-gate-mcp/src/tenant_authz.rs`
  - `decision-gate-mcp/src/tools.rs`
- Enterprise tenant mapping:
  - `enterprise/decision-gate-enterprise/src/tenant_authz.rs`
- Usage metering + quota enforcement:
  - `decision-gate-mcp/src/usage.rs`
  - `enterprise/decision-gate-enterprise/src/usage.rs`
  - `enterprise/decision-gate-enterprise/src/usage_sqlite.rs`
- Postgres run state + schema registry store:
  - `enterprise/decision-gate-store-enterprise/src/postgres_store.rs`
- S3 runpack store + hardening:
  - `enterprise/decision-gate-store-enterprise/src/s3_runpack_store.rs`
- Audit hash chain:
  - `enterprise/decision-gate-enterprise/src/audit_chain.rs`
- Enterprise config wiring:
  - `enterprise/decision-gate-enterprise/src/config.rs`
- Enterprise server builder:
  - `enterprise/decision-gate-enterprise/src/server.rs`
- Tenant admin scaffolding + minimal UI:
  - `enterprise/decision-gate-enterprise/src/tenant_admin.rs`
  - `enterprise/decision-gate-enterprise/src/admin_ui.rs`

**Enterprise system-test coverage is enumerated and passing (assumed):**
- `enterprise/enterprise-system-tests/`

---

# Part IV — Gap Analysis (DG-E vs World-Class Target)

## A) Platform-Level Gaps (should be shared with ASC)

- **OIDC / JWT integration** in DG-E runtime (beyond scaffolding).
- **Persistent API key lifecycle** (issuing, hashing, revocation, rotation).
- **Org-level RBAC / SCIM provisioning** (Phase-2 tier).
- **Usage + billing export pipeline** (plan entitlements, invoice hooks).
- **Audit export pipeline + retention enforcement**.
- **Unified telemetry implementation** (ASC telemetry crate still Foundation).
- **Centralized config registry + provenance** for multi-service deployments.

## B) DG-Specific Gaps

- **Precheck hash-only audit logging** (P1 open item).
- **Durable runpack storage in OSS** (object store adapter is enterprise only).
- **Admin UI beyond scaffolding** (runs list, download, rotate keys).

## C) Operational Gaps

- **SLO/SLA definitions** and alerting baselines.
- **HA deployment patterns** (multi-AZ, multi-region).
- **Supportability runbooks** (debug bundle workflows + incident response).

---

# Part V — Migration Roadmap (Unified Platform Alignment)

## Phase 0: Contract Lock-In (Immediate)
- Freeze the OSS seams:
  - `TenantAuthorizer`, `UsageMeter`, `RunpackStorage`, audit event schema.
- Align DG OSS config schema with ASC policy vocabulary.

## Phase 1: Reorg + Platform Adoption (Near-Term)
- Move DG-E into Asset Core monorepo as a private service.
- Replace DG-E internal implementations with ASC platform crates:
  - Auth -> `assetcore-auth`
  - OIDC -> `assetcore-oidc`
  - Quotas -> `assetcore-quota-backends`
  - Namespace -> `assetcore-namespace-catalog`
  - Telemetry -> `assetcore-telemetry`
  - Correlation -> `assetcore-correlation`

## Phase 2: Enterprise Completion
- Implement missing platform pieces (billing, retention, audit export).
- Extend DG admin UI and lifecycle APIs.
- Align HA semantics with ASC (idempotency, failure modes, retry policies).

## Phase 3: Premium Tier
- Immutable audit backend (WORM/AssetCore anchoring).
- Runpack attestation + offline verification chains.
- Air-gapped/on-prem packaging and FIPS builds.

---

# Part VI — “One Roadmap to Rule Them All” Checklist

**World-Class DG-E is "done" when:**

- Platform layer handles auth, RBAC, quotas, audit export, and telemetry.
- DG-E uses platform primitives for all universal concerns.
- DG OSS remains clean and stable via explicit seams.
- Enterprise tests validate tenant isolation, audit integrity, and storage
  hardening end-to-end.
- ASC and DG share a single operational language for tenants, namespaces,
  audit, and usage.

---

# Appendix — Notes on OSS vs Enterprise Boundary

DG OSS must remain deterministic and auditable. Enterprise behavior must be
introduced only via seams (traits/config). This roadmap is compatible with the
OSS/Enterprise boundary rules in `AGENTS.md`.
