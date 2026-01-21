# AGENTS.md (ret-logic)

> **Audience:** Code-generation agents and automation working in `ret-logic/`.  
> **Goal:** Preserve the universal predicate algebra (RET: Requirement Evaluation Tree),
> high-performance evaluation paths, and strict correctness guarantees of the engine.

---

## 0) TL;DR (one screen)

- **Universal algebra only:** AND/OR/NOT/RequireGroup stay domain-agnostic.
- **Performance posture:** zero allocations in hot paths; prefer SoA readers.
- **Tri-state is opt-in:** boolean evaluation remains the default fast path.
- **No Decision Gate policy bleed:** evidence anchoring, runpack, and disclosure policy live in Decision Gate.
- **Style:** follow **Docs/standards/codebase_formatting_standards.md** headers and sections.
- **Engineering standards:** follow **Docs/standards/codebase_engineering_standards.md**.
- **Threat model:** consult **Docs/security/threat_model.md**; note `Threat Model Delta: none`
  if no update is needed.

---

## 1) What you may / may not change

### ✅ In scope

- Add or refine predicate evaluation traits and adapters.
- Extend tri-state evaluation logic and trace hooks.
- Add or tighten DSL parsing and structural validation.
- Add unit tests (dedicated files under `tests/` only).

### ⛔ Out of scope (require design approval)

- Embedding Decision Gate-specific policy (evidence anchoring, runpack artifacts, disclosure rules).
- Introducing allocations in hot-path evaluation.
- Relaxing lint/clippy severity or removing safety checks.

---

## 2) Non-negotiables

- **Zero-allocation hot paths** for runtime evaluation.
- **Domain boundary at predicates** only; keep algebra universal.
- **Strict lint/clippy** as defined in this crate's `Cargo.toml`.
- **Determinism:** evaluation results must be deterministic for identical inputs.

---

## 3) Testing requirements

- Unit tests must live in dedicated files under `tests/`.
- Cover tri-state logic tables and RequireGroup semantics.
- Include trace-hook coverage when adding trace features.

---

## 4) References

- Docs/standards/codebase_formatting_standards.md
- Docs/standards/codebase_engineering_standards.md
- Docs/security/threat_model.md
