<!--
Docs/verification/decision_gate_release_checklist.md
============================================================================
Document: Decision Gate Release Checklist (Repeatable)
Description: Repeatable checklist for tagging a Decision Gate release.
Purpose: Provide a lightweight, world-class release discipline for solo
         maintainers without slowing iteration.
Dependencies:
  - CHANGELOG.md
  - Cargo.toml
  - Docs/roadmap/foundational_correctness_roadmap.md
  - Docs/security/threat_model.md
============================================================================
Last Updated: 2026-02-04 (UTC)
============================================================================
-->

# Decision Gate Release Checklist (Repeatable)

Use this checklist **every time** you cut a release tag. This is not a
"merge-to-main" gate. It is a human sign-off step for published releases.

---

## 1) Version + Changelog

- [ ] Decide the release version and scope (patch/minor).
- [ ] Update `Cargo.toml` workspace version and any explicitly versioned crates.
- [ ] Update `CHANGELOG.md` with the release date and highlights.
- [ ] Ensure any breaking changes are called out explicitly.

---

## 2) Correctness Gates

- [ ] Confirm Gate status in `Docs/roadmap/foundational_correctness_roadmap.md` is accurate.
- [ ] Run core tests: `cargo test --workspace --exclude system-tests`.
- [ ] Run system tests relevant to the release scope (P0/P1 as applicable).

---

## 3) Determinism + Runpacks

- [ ] Re-run deterministic scenarios that produce runpacks, if affected.
- [ ] Verify runpack root hashes match expected artifacts where applicable.

---

## 4) Contracts + Tooling

- [ ] `cargo run -p decision-gate-contract -- check` passes.
- [ ] `cargo run -p decision-gate-sdk-gen -- check` passes if SDKs are in scope.

---

## 5) Documentation + Hygiene

- [ ] `npm run docs:lint` passes.
- [ ] `npm run docs:linkify:check` passes.
- [ ] README and top-level docs reflect current behavior and defaults.

---

## 6) Security Posture

- [ ] Threat model delta reviewed; update `Docs/security/threat_model.md` if needed.
- [ ] No insecure defaults introduced (binds, raw evidence, permissive policies).

---

## 7) Tag + Release

- [ ] Create a signed tag for the release version.
- [ ] Draft release notes (short summary, known risks, next steps).
- [ ] Publish the release artifacts.

---

## Sign-Off

**Release:** ___________________________________

**Version:** ___________________________________

**Date:** ______________________________________

**Notes:** _____________________________________

