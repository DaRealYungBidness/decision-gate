<!--
Docs/roadmap/enterprise/decision_gate_enterprise/decision_gate_enterprise_migration_playbook.md
============================================================================
Document: Decision Gate Enterprise Migration Playbook (DG-E -> ASC Monorepo)
Description: Operational playbook for moving DG enterprise into the Asset Core
             monorepo and refitting it to ASC platform primitives.
Purpose: Provide an agent-executable, high-assurance migration plan with
         preflight checks, stop conditions, and validation gates.
Dependencies:
  - Docs/roadmap/enterprise/decision_gate_enterprise/decision_gate_enterprise_world_class_roadmap.md
  - Docs/roadmap/enterprise/enterprise_phasing_plan.md
  - Docs/roadmap/open_items.md
  - enterprise/decision-gate-enterprise/README.md
  - enterprise/decision-gate-store-enterprise/README.md
  - enterprise/enterprise-system-tests/README.md
  - /mnt/c/Users/Micha/Documents/GitHub/Asset-Core/README.md
============================================================================
Last Updated: 2026-01-27 (UTC)
============================================================================
-->

# Decision Gate Enterprise Migration Playbook (DG-E -> ASC Monorepo)

## Purpose

This playbook defines the migration of Decision Gate Enterprise (DG-E) into the
Asset Core (ASC) monorepo and the refit of DG-E to ASC platform primitives. It
is designed to be runnable by an automation agent (overnight) with explicit
preflight checks, stop conditions, and validation gates.

This is the operational companion to the world-class DG enterprise roadmap:

- `Docs/roadmap/enterprise/decision_gate_enterprise/decision_gate_enterprise_world_class_roadmap.md`

## Strategic Intent

1. **Preserve the closed-loop refactor advantage** by consolidating enterprise
   components into the private monorepo.
2. **Align DG-E semantics with ASC platform language** (tenants, auth, quotas,
   audit, telemetry) before OSS seams are frozen.
3. **Keep DG OSS clean and versioned** so public releases remain stable and
   deterministic.
4. **Prevent future firefighting** by enforcing shared primitives and invariant
   semantics across services from day one.

## Scope

**In scope:**

- Moving DG-E crates into ASC monorepo.
- Wiring DG-E to ASC platform crates.
- Establishing stable DG OSS seams and dependency contracts.
- Updating enterprise system tests to run in ASC monorepo context.

**Out of scope:**

- Public DG OSS repository refactors (beyond seam stabilization).
- New DG features unrelated to enterprise alignment.
- Full SaaS billing integration (planned in later phases).

## Assumptions

- DG OSS remains a separate public repo.
- ASC monorepo is private and authoritative for enterprise platform crates.
- Enterprise system tests are available and passing when dependencies are
  satisfied.
- We prioritize correctness and determinism over speed during migration.

---

# Section 1 — Target Repo Layout (ASC Monorepo)

This layout is the stable destination. Naming is explicit to avoid ambiguity.

```
asset-core/
├── platform/                         # Shared enterprise primitives
│   ├── platform-auth/                # wraps/re-exports assetcore-auth
│   ├── platform-oidc/                # wraps/re-exports assetcore-oidc
│   ├── platform-quota/               # wraps/re-exports assetcore-quota-backends
│   ├── platform-telemetry/           # wraps/re-exports assetcore-telemetry
│   ├── platform-correlation/         # wraps/re-exports assetcore-correlation
│   ├── platform-namespace/           # wraps/re-exports assetcore-namespace-catalog
│   └── platform-config/              # wraps/re-exports assetcore-config-runtime
├── decision-gate-enterprise/         # DG-E control-plane extension
├── decision-gate-store-enterprise/   # DG-E Postgres/S3 storage backends
├── decision-gate-enterprise-tests/   # DG-E system tests (optional rename)
└── vendor/decision-gate/             # DG OSS via git/pinned dependency
```

**Notes:**

- The `platform/` layer is optional but recommended for semantic clarity.
- `vendor/decision-gate/` can be a git submodule or a pinned dependency; local
  development may use a path override to preserve the refactor loop.

---

# Section 2 — Dependency Strategy (OSS vs Enterprise)

## 2.1 DG OSS Dependency Contracts

DG OSS exposes explicit seams used by DG-E:

- `TenantAuthorizer`
- `UsageMeter`
- `RunpackStorage`
- `McpAuditSink`
- `McpMetrics`
- `ServerOverrides` / `McpServer` wiring

**Rule:** DG-E must depend on DG OSS via stable interfaces only. No reverse
coupling is allowed.

## 2.2 Closed-Loop Iteration (Local Overrides)

To preserve rapid refactor iteration:

- Use a **local path override** for DG OSS during development.
- Use a **git SHA or tag** for CI and release builds.

This yields deterministic builds in CI, while preserving local speed.

---

# Section 3 — Preflight Checks (Must Pass Before Migration)

**Preflight Checklist:**

1. DG OSS seams are documented and stable (no pending refactors).
2. DG enterprise system tests are passing in the DG repo.
3. ASC monorepo builds cleanly.
4. Namespace/tenant vocabulary is aligned (documented).
5. Target layout path decisions are agreed and documented.
6. All migration paths are prepared (no pending local changes).

**Stop Condition:**
If any preflight item fails, abort migration.

---

# Section 4 — Migration Phases (Step-by-Step)

## Phase 1 — Reorg (Move DG-E into ASC Monorepo)

**Goal:** Move DG-E _as-is_ without changing behavior.

**Steps:**

1. Create `decision-gate-enterprise`, `decision-gate-store-enterprise`, and
   `decision-gate-enterprise-tests` directories in ASC monorepo.
2. Copy DG-E crates from DG repo into ASC monorepo.
3. Update ASC workspace to include new crates.
4. Update Cargo dependencies:
   - Replace path deps to DG OSS with git/tag deps or local overrides.
5. Build DG-E in ASC monorepo.

**Stop Conditions:**

- Build fails with missing DG OSS seams.
- Workspace dependency cycles are detected.

## Phase 2 — Platform Refactor (Replace Internals)

**Goal:** Swap DG-E implementations to use ASC platform primitives.

**Module Mapping:**

| DG-E Module       | Replace With ASC Platform                    | Notes                                                |
| ----------------- | -------------------------------------------- | ---------------------------------------------------- |
| `tenant_authz.rs` | `assetcore-auth` + `assetcore-oidc`          | Map principals/roles to DG TenantAuthorizer.         |
| `usage.rs`        | `assetcore-quota-backends`                   | Preserve DG usage counters.                          |
| `usage_sqlite.rs` | Remove/replace                               | Keep only for dev fallback if required.              |
| `config.rs`       | `assetcore-config-runtime` + platform-config | Translate DG enterprise config into platform config. |
| `tenant_admin.rs` | Platform tenant registry                     | Align with namespace catalog.                        |
| `admin_ui.rs`     | Keep (DG-specific)                           | UI remains DG-specific.                              |

**Stop Conditions:**

- Any replacement breaks deterministic behavior or audit invariants.
- Usage enforcement no longer blocks mutations on quota exceed.
- Tenant isolation is degraded.

## Phase 3 — Semantic Alignment

**Goal:** Align operational semantics across DG-E and ASC.

**Required Alignments:**

- Correlation ID propagation.
- Audit event schema field parity.
- Failure mode taxonomy (authz vs quota vs registry ACL).
- Retry/idempotency semantics.

**Stop Conditions:**

- Audit events lose required fields.
- Tenant/namespace semantics diverge from ASC vocabulary.

## Phase 4 — Validation and Hardening

**Required Tests:**

- DG-E enterprise system tests (full suite).
- ASC system tests (baseline).
- Cross-service smoke test (DG evidence via ASC provider, if configured).

**Stop Conditions:**

- Any enterprise system test fails.
- Audit chain integrity checks fail.
- Storage integrity checks fail.

---

# Section 5 — Validation Matrix (Minimum Required)

| Area              | Test Gate                                             | Expected Result |
| ----------------- | ----------------------------------------------------- | --------------- |
| Tenant isolation  | enterprise-system-tests/tenancy                       | Pass            |
| Usage quotas      | enterprise-system-tests/usage                         | Pass            |
| Audit integrity   | enterprise-system-tests/audit                         | Pass            |
| Storage integrity | enterprise-system-tests/storage_postgres + storage_s3 | Pass            |
| Transport parity  | enterprise-system-tests/mcp_transport                 | Pass            |
| TLS/mTLS          | enterprise-system-tests/security                      | Pass            |

---

# Section 6 — Rollback Strategy

If migration fails:

1. Revert ASC monorepo workspace changes.
2. Remove DG-E directories from ASC monorepo.
3. Keep DG-E in DG repo and re-evaluate failed step.

No partial migration should be left in place if tests fail.

---

# Section 7 — Risk Register

| Risk                        | Impact | Mitigation                                     |
| --------------------------- | ------ | ---------------------------------------------- |
| Policy vocabulary mismatch  | High   | Define shared vocabulary doc before Phase 2.   |
| Telemetry crate instability | Medium | Harden `assetcore-telemetry` before refit.     |
| Quota semantics mismatch    | High   | Map DG counters explicitly to ASC quota model. |
| Config schema divergence    | Medium | Add translation layer; avoid hard coupling.    |
| OSS seam drift              | High   | Freeze DG OSS seams prior to migration.        |

---

# Section 8 — Deliverables

**At the end of migration:**

- DG-E lives in ASC monorepo.
- DG-E uses ASC platform crates for auth, quota, namespace, telemetry, and
  correlation.
- DG OSS remains clean and publishes stable seam contracts.
- Enterprise tests pass in ASC monorepo.

---

# Section 9 — Cross-Links

- World-class roadmap:
  - `Docs/roadmap/enterprise/decision_gate_enterprise/decision_gate_enterprise_world_class_roadmap.md`
- Enterprise phasing plan:
  - `Docs/roadmap/enterprise/enterprise_phasing_plan.md`
- Open items:
  - `Docs/roadmap/open_items.md`
