<!--
Docs/roadmap/decision_gate_portable_verification_level1.md
============================================================================
Document: Decision Gate Portable Verification (Level 1)
Description: Roadmap for offline evidence signature verification in runpack
             verification flows.
Purpose: Define the scope, requirements, and implementation plan for Level 1
         portable verification in OSS.
Dependencies:
  - decision-gate-core/src/core/evidence.rs
  - decision-gate-core/src/runtime/runpack.rs
  - decision-gate-mcp/src/evidence.rs
  - Docs/architecture/decision_gate_runpack_architecture.md
  - Docs/security/threat_model.md
============================================================================
Last Updated: 2026-02-04 (UTC)
============================================================================
-->

# Decision Gate Portable Verification (Level 1)

## Purpose

Define Level 1 portable verification: **offline authenticity checks for evidence
snapshots** stored in runpacks. This roadmap focuses on verifying evidence
signatures offline without contacting providers.

Level 1 is an **auditability upgrade**, not a launch blocker.

---

## Scope

**In scope**

- Offline signature verification for evidence records stored in runpacks.
- Explicit trust configuration for public keys used during verification.
- Fail-closed verification behavior with structured errors.
- OSS implementation (no enterprise dependencies).

**Out of scope**

- Evidence fetch or snapshotting beyond what is already stored in runpacks.
- Running a hosted key registry or PKI.
- Encryption or secure storage of private keys.
- Online re-evaluation against live providers.

---

## Current State (Gap)

- Evidence signatures exist in the data model (`EvidenceSignature` in
  `EvidenceResult`).
- MCP can enforce signatures at ingest time (`TrustPolicy::RequireSignature`).
- Runpack verification **does not** validate evidence signatures offline.

References:

- `decision-gate-core/src/core/evidence.rs`
- `decision-gate-mcp/src/evidence.rs`
- `decision-gate-core/src/runtime/runpack.rs`

---

## Level 1 Definition (Target)

Level 1 portable verification means:

- A verifier can prove, offline, that **evidence snapshots in the runpack were
  signed by authorized provider keys**.
- Verification does **not** require contacting external providers.

This does **not** guarantee external system immutability. It only proves that
the evidence snapshot was attested by a trusted key.

---

## Requirements

1. **Trusted key inputs**
   - Verification must accept a set of trusted public keys indexed by `key_id`.
   - Keys are provided by configuration or tooling, not embedded in runpacks.

2. **Signature semantics**
   - Signature verification must use the same canonical hash semantics as MCP
     ingest (`EvidenceSignature` over the canonical JSON of the evidence hash).
   - Signature scheme support must be explicit and minimal (ed25519 only, for now).

3. **Fail-closed behavior**
   - If a signature is missing and a policy requires signatures, verification
     must fail for that evidence record.
   - Invalid signatures or untrusted keys must fail the verification report.

4. **Determinism**
   - Verification results must be deterministic for identical inputs, including
     trusted key sets.

5. **Minimal integration surface**
   - Core verifier remains deterministic and policy-driven.
   - MCP/CLI provides key configuration and optional policy flags.

---

## Implementation Plan (OSS)

1. **Key configuration contract**
   - Define a runpack verification configuration structure containing:
     - trusted public keys (`key_id` → public key bytes)
     - signature policy: `required` or `audit`
   - Store configuration outside runpacks (local config or CLI flags).

2. **Verifier extension**
   - Extend `RunpackVerifier` to accept an optional signature verification hook.
   - Verification reads `gate_evals.json`, inspects `EvidenceResult.signature`,
     and validates against the trusted keys.
   - Collect structured errors into the verification report.

3. **MCP / CLI integration**
   - Provide a configuration path for trusted keys.
   - Wire the configuration into `runpack_verify` flows.

4. **Documentation**
   - Update runpack architecture doc with Level 1 verification semantics.
   - Update threat model with signature verification boundary and key-trust assumptions.

---

## Test Plan

**Unit tests**

- Valid signature with trusted key passes.
- Missing signature with required policy fails.
- Signature mismatch fails.
- Untrusted `key_id` fails.

**System tests**

- Runpack verification with signed evidence passes with correct key set.
- Runpack verification fails with wrong or missing keys.

---

## Non-Goals (Explicit)

- No evidence re-fetch or external DB replay.
- No bundled key registry or online revocation checks.
- No changes to evidence providers required beyond existing signature support.

---

## Threat Model Delta

**Expected change:** Add explicit trust boundary for offline signature
verification and key-trust configuration. Update
`Docs/security/threat_model.md` when implementation begins.

