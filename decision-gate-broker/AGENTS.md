# AGENTS.md (decision-gate-broker)

> **Audience:** Agents and automation working on broker sources/sinks.
> **Goal:** Provide safe, deterministic payload resolution and dispatch.

---

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
- decision-gate-core/AGENTS.md
- Docs/security/threat_model.md
