# AGENTS.md (decision-gate-contract)

> **Audience:** Agents and automation working on Decision Gate contract artifacts.
> **Goal:** Keep schemas, examples, and tool contracts authoritative and in sync.

---

## Standards (Read First)
Before making changes, read and follow:
- `Docs/standards/codebase_engineering_standards.md`
- `Docs/standards/codebase_formatting_standards.md`
- `Docs/standards/doc_formatting_standards.md`

## 0) TL;DR

- **Single source of truth:** contracts drive SDKs and docs.
- **No manual drift:** regenerate artifacts after changes.
- **Compatibility first:** breaking schema changes require design review.

---

## 1) In scope
- JSON schema definitions and MCP tool contracts.
- Example payloads and validation fixtures.
- Generation/validation tooling.

## 2) Out of scope (design approval required)
- Changing tool semantics without core alignment.
- Silent breaking changes to schema fields.

## 3) Non-negotiables
- Artifacts must match source definitions.
- Schema validation tests must pass.

## 4) Testing
```bash
cargo test -p decision-gate-contract
cargo run -p decision-gate-contract -- check
```

## 5) References
- Docs/standards/codebase_engineering_standards.md
- Docs/standards/codebase_formatting_standards.md
- Docs/standards/doc_formatting_standards.md
- Docs/roadmap/trust_lanes_registry_plan.md
- Docs/security/threat_model.md
