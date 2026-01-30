# AGENTS.md (decision-gate-providers)

> **Audience:** Agents and automation working on built-in providers.
> **Goal:** Preserve safe, deterministic evidence queries with strict validation.

---

## 0) TL;DR

- **Fail closed:** invalid inputs must reject.
- **Deterministic outputs:** same inputs yield same evidence/hash.
- **Size and path limits:** enforce hard caps on IO and payloads.

---

## 1) In scope
- Provider check parsing and validation.
- EvidenceResult formation (value, hash, metadata).
- Unit tests for each provider's error paths.

## 2) Out of scope (design approval required)
- Relaxing IO limits or security checks.
- Provider behaviors that depend on nondeterministic state without justification.

## 3) Non-negotiables
- No hidden network calls beyond declared providers.
- No path traversal or unsafe file access.
- Strict request size enforcement.

## 4) Testing
```bash
cargo test -p decision-gate-providers
```

## 5) References
- Docs/security/threat_model.md
- decision-gate-core/AGENTS.md
