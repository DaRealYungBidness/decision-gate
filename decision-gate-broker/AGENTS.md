# AGENTS.md (decision-gate-broker)

> **Audience:** Agents and automation working on broker sources/sinks.
> **Goal:** Provide safe, deterministic payload resolution and dispatch.

---

## Standards (Read First)
Before making changes, read and follow:
- `Docs/standards/codebase_engineering_standards.md`
- `Docs/standards/codebase_formatting_standards.md`
- `Docs/standards/doc_formatting_standards.md`

## 0) TL;DR

- **Fail closed:** reject unsafe paths and oversized payloads.
- **No side effects on error:** do not partially dispatch.
- **Determinism:** same inputs yield same outputs.

---

## 1) In scope
- Source and sink implementations.
- Payload size and path validation.
- Composite broker wiring and error mapping.

## 2) Out of scope (design approval required)
- Changing core disclosure semantics.
- Relaxing size or security constraints.

## 3) Non-negotiables
- Strict input validation.
- No path traversal or unsafe file IO.

## 4) Testing
```bash
cargo test -p decision-gate-broker
```

## 5) References
- Docs/standards/codebase_engineering_standards.md
- Docs/standards/codebase_formatting_standards.md
- Docs/standards/doc_formatting_standards.md
- decision-gate-core/AGENTS.md
- Docs/security/threat_model.md
