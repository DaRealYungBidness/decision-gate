# AGENTS.md (decision-gate-store-enterprise)

> **Audience:** Agents and automation working on enterprise storage backends.
> **Goal:** Provide durable, multi-tenant storage without changing OSS semantics.

---

## 0) TL;DR (one screen)

- **No OSS contamination:** OSS crates must not depend on enterprise crates.
- **Fail closed:** storage errors must never lead to silent data loss.
- **Deterministic:** persistence must preserve Decision Gate determinism.
- **Auditability:** persistence must support audit retention requirements.
- **Standards:** follow **Docs/standards/codebase_engineering_standards.md** and
  **Docs/standards/codebase_formatting_standards.md**.
- **Threat model:** update **Docs/security/threat_model.md** when boundaries change.
- **Architecture docs:** keep `Docs/architecture/enterprise/*.md` current for new behavior.

---

## 1) In scope
- Postgres-based `RunStateStore` and `DataShapeRegistry` implementations.
- Object storage integration for runpacks and artifacts.
- Backup/restore hooks and corruption detection for auditability.

## 2) Out of scope (design approval required)
- Changing core store interfaces in OSS crates.
- Weakening data integrity or fail-closed behavior.

## 3) Non-negotiables
- Strict tenant isolation across all storage operations.
- Deterministic serialization and canonical hashing preserved.
- Explicit, audited error paths for storage failures.

## 4) References
- Docs/standards/codebase_formatting_standards.md
- Docs/standards/codebase_engineering_standards.md
- Docs/security/threat_model.md
- Docs/roadmap/enterprise/enterprise_phasing_plan.md
- Docs/architecture/enterprise/decision_gate_enterprise_storage_architecture.md
