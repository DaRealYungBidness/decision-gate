# AGENTS.md (decision-gate-core)

> **Audience:** Code-generation agents, copilots, and automation working on Decision Gate core.  
> **Goal:** Preserve Decision Gate's deterministic gate evaluation, controlled disclosure,
> and audit-grade artifact generation with Zero Trust defaults.

---

## Standards (Read First)
Before making changes, read and follow:
- `Docs/standards/codebase_engineering_standards.md`
- `Docs/standards/codebase_formatting_standards.md`
- `Docs/standards/doc_formatting_standards.md`

## 0) TL;DR (one screen)

- **Decision Gate is not an agent framework:** it ingests triggers, evaluates gates, and
  dispatches disclosures; it does not run conversations.
- **Evidence-based gating:** decisions must be decidable from recorded evidence.
- **Fail closed:** missing or invalid evidence yields hold/deny behavior.
- **Runpack-ready:** outputs must remain offline verifiable and deterministic.
- **Requirements crate boundary:** gate algebra lives in `ret-logic/`;
  Decision Gate owns evidence anchoring, decision records, and disclosure policy.
- **Trust lanes:** asserted vs verified evidence is enforced at gate/condition level.
- **Precheck:** read-only evaluation path must not mutate run state.
- **Docs/tooltips alignment:** after behavior or schema changes, update contract
  tooltips and regenerate `Docs/generated/decision-gate`.
- **Threat model:** consult **Docs/security/threat_model.md**; note
  `Threat Model Delta: none` when applicable.

---

## 1) What you may / may not change

### ✅ In scope

- Gate evaluation logic and decision logging.
- Evidence anchoring and safe-summary behavior.
- Trust-lane enforcement and precheck evaluation behavior.
- Artifact/manifest interfaces and deterministic hashing.
- Unit tests for all behavior (in dedicated files under `tests/`).

### ⛔ Out of scope (require design approval)

- Relaxing security posture or fail-closed semantics.
- Introducing AssetCore dependencies in Decision Gate core.
- Adding ad-hoc disclosure logic outside the control plane.

---

## 2) Non-negotiables

- Determinism and replayability of decisions.
- Strict Zero Trust posture (assume nation-state adversaries).
- No hidden mutable globals.
- No inline tests inside library modules.
 - Precheck must never write run state or issue disclosures.

---

## 3) Testing requirements

- Unit tests must live in dedicated files under `tests/`.
- Add determinism/idempotence tests for any evaluation changes.
- Include negative tests for malformed triggers or invalid evidence.

---

## 4) References

- Docs/standards/codebase_formatting_standards.md
- Docs/standards/codebase_engineering_standards.md
- Docs/standards/doc_formatting_standards.md
- Docs/security/threat_model.md
