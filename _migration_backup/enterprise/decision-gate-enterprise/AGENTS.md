# AGENTS.md (decision-gate-enterprise)

> **Audience:** Agents and automation working on Decision Gate enterprise code.
> **Goal:** Implement managed-cloud features without contaminating OSS crates.

---

## 0) TL;DR (one screen)

- **No OSS contamination:** OSS crates must not depend on enterprise crates.
- **Seams, not forks:** extend via traits/config; do not alter core semantics.
- **Fail closed:** authz, quotas, and metering must be enforced by default.
- **Auditability:** all access denials and quota decisions must be logged.
- **Standards:** follow **Docs/standards/codebase_engineering_standards.md** and
  **Docs/standards/codebase_formatting_standards.md**.
- **Threat model:** update **Docs/security/threat_model.md** when boundaries change.
- **Architecture docs:** keep `Docs/architecture/enterprise/*.md` current for new behavior.

---

## 1) In scope
- Tenant authz enforcement and principal-to-tenant/namespace binding.
- Usage metering, quotas, and billing-grade counters.
- Admin lifecycle APIs (tenant provisioning, key rotation).
- Enterprise audit sinks and export pipelines.

## 2) Out of scope (design approval required)
- Changes to Decision Gate core semantics or trust model.
- Any weakening of fail-closed security posture.
- Adding enterprise dependencies to OSS crates.

## 3) Non-negotiables
- Deterministic, auditable behavior.
- Explicit multi-tenant isolation checks on every tool call.
- Production defaults are secure and fail-closed.

## 4) References
- Docs/standards/codebase_formatting_standards.md
- Docs/standards/codebase_engineering_standards.md
- Docs/security/threat_model.md
- Docs/roadmap/enterprise/enterprise_phasing_plan.md
- Docs/architecture/enterprise/decision_gate_enterprise_server_wiring_architecture.md
- Docs/architecture/enterprise/decision_gate_enterprise_tenant_authz_admin_architecture.md
- Docs/architecture/enterprise/decision_gate_enterprise_usage_quota_architecture.md
- Docs/architecture/enterprise/decision_gate_enterprise_audit_chain_architecture.md
