<!--
Docs/roadmap/decision_gate_publish_readiness_checklist.md
============================================================================
Document: Decision Gate Publish Readiness Checklist
Description: Manual sign-off checklist before any public SDK or adapter release.
Purpose: Enforce world-class quality, security, and determinism before publish.
Dependencies:
  - Docs/roadmap/decision_gate_world_class_integration_readiness_plan.md
  - Docs/roadmap/decision_gate_sdk_invariance_plan.md
  - Docs/business/decision_gate_integration_landscape.md
============================================================================
-->

# Decision Gate Publish Readiness Checklist

## Purpose

This checklist defines the **manual gates** required before any SDK or adapter
is published to a public registry. Passing automated tests is necessary but not
sufficient. We only ship when every item below is explicitly verified and
signed off.

**Rule:** Publishing is a separate, deliberate act. This checklist governs
readiness only.

---

## Scope

Applies to:
- Python + TypeScript client SDKs
- Example suites
- Local CI-style verification scripts
- Docs and generated artifacts

Not in scope:
- Enterprise-only features
- Marketplace marketing activities

---

## Mandatory Gates (No Exceptions)

### A. Contract and Generation Invariants
- [ ] `decision-gate-contract` and `decision-gate-sdk-gen` outputs are in sync.
- [ ] `cargo run -p decision-gate-contract -- check` passes.
- [ ] `cargo run -p decision-gate-sdk-gen -- check` passes.
- [ ] Generated SDKs are **not** manually edited.
- [ ] OpenAPI output is updated and deterministic.

### B. SDK Quality and Ergonomics
- [ ] All tools have complete docstrings/JSDoc with field-level constraints.
- [ ] Examples in docs are short, correct, and explain intent (not just steps).
- [ ] Optional runtime validation helpers exist and are documented.
- [ ] Public exports are explicit (`__all__` in Python, named exports in TS).

### C. Tests and Example Trustworthiness
- [ ] Unit tests pass (`cargo test --workspace --exclude system-tests`).
- [ ] System tests pass (P0 and P1 as defined in `system-tests/test_registry.toml`).
- [ ] All examples are runnable and validated via system tests.
- [ ] Example outputs validate against generated schemas.
- [ ] Adapter examples pass under `scripts/adapter_tests.sh` (when deps installed).

### D. Packaging Dry-Run (No Publish)
- [ ] `scripts/package_dry_run.sh --python` passes (build + install + import).
- [ ] `scripts/package_dry_run.sh --typescript` passes (tsc + pack + import).
- [ ] `scripts/verify_all.sh --package-dry-run` passes end-to-end.

### E. Security Posture
- [ ] Non-loopback binds require explicit opt-in with strong warnings.
- [ ] TLS or explicit upstream termination + non-local auth are required for non-loopback operation.
- [ ] Example configs are safe-by-default.
- [ ] No insecure defaults introduced in SDKs or examples.

### F. Documentation Accuracy
- [ ] `Docs/business/decision_gate_integration_landscape.md` matches reality.
- [ ] Roadmap docs reflect the current state of completion.
- [ ] Generated artifacts referenced in docs exist on disk.

---

## Release Readiness Extras (Strongly Recommended)

- [ ] Version numbers are updated consistently across SDKs.
- [ ] A short release note is drafted (what changed, why, risks).
- [ ] A manual read-through of generated SDKs is completed.
- [ ] License headers and metadata are correct.

---

## Sign-Off

**Reviewer:** ____________________________________

**Date:** ________________________________________

**Notes:** _______________________________________
