# AGENTS.md (decision-gate-store-sqlite)

> **Audience:** Agents and automation working on SQLite persistence.
> **Goal:** Preserve durability, integrity checks, and strict limits.

---

## Standards (Read First)
Before making changes, read and follow:
- `Docs/standards/codebase_engineering_standards.md`
- `Docs/standards/codebase_formatting_standards.md`
- `Docs/standards/doc_formatting_standards.md`

## 0) TL;DR

- **Integrity first:** hashes must validate on load.
- **Fail closed:** corrupt or oversized records must reject.
- **No silent schema changes:** version upgrades require explicit migration.

---

## 1) In scope
- Run state persistence and schema registry storage.
- Size limits, path validation, and hash verification.
- Unit tests for corruption and edge cases.

## 2) Out of scope (design approval required)
- Changing schema versions or migration flow without review.
- Relaxing size limits or integrity checks.

## 3) Non-negotiables
- Deterministic serialization and hash verification.
- No silent data loss on errors.

## 4) Testing
```bash
cargo test -p decision-gate-store-sqlite
```

## 5) References
- Docs/standards/codebase_engineering_standards.md
- Docs/standards/codebase_formatting_standards.md
- Docs/standards/doc_formatting_standards.md
- Docs/security/threat_model.md
- decision-gate-core/AGENTS.md
