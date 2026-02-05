# AGENTS.md (decision-gate-config)

> **Audience:** Agents and automation working on Decision Gate configuration.
> **Goal:** Single-source-of-truth config model, deterministic schema/docs, and
> fail-closed validation with Zero Trust defaults.

---

## Standards (Read First)
Before making changes, read and follow:
- `Docs/standards/codebase_engineering_standards.md`
- `Docs/standards/codebase_formatting_standards.md`
- `Docs/standards/doc_formatting_standards.md`

## 0) TL;DR

- **Single source of truth:** config types + validation live here.
- **Deterministic artifacts:** schema, docs, and examples are generated here.
- **Fail closed:** invalid configs must reject safely.
- **No drift:** generators must stay aligned with runtime validation.
- **OSS boundary:** no enterprise deps or runtime-only behavior changes.

---

## 1) In scope

- Config structs, defaults, and validation.
- Schema and documentation generators.
- Canonical config examples and fixtures.
- Drift-prevention tests for config artifacts.

## 2) Out of scope (design approval required)

- Changing runtime semantics outside config validation.
- Introducing enterprise-specific fields or dependencies.
- Relaxing security defaults or fail-closed behavior.

## 3) Non-negotiables

- Deterministic output ordering and formatting.
- Every config field must be documented and schema-covered.
- Examples must validate against the schema.
- Runtime validation must remain stricter or equal to schema constraints.

## 4) Testing

- Unit tests for all validation constraints.
- Tests that generated docs/schema/example match committed outputs.
- Tests that example TOML validates against JSON schema.

## 5) References

- Docs/configuration/decision-gate.toml.md
- Docs/roadmap/decision_gate_config_single_source_plan.md
- Docs/security/threat_model.md
- Docs/standards/codebase_formatting_standards.md
- Docs/standards/codebase_engineering_standards.md
- Docs/standards/doc_formatting_standards.md
