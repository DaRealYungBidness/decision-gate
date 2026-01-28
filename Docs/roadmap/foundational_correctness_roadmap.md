<!--
Docs/roadmap/foundational_correctness_roadmap.md
============================================================================
Document: Decision Gate Foundational Correctness Roadmap (OSS Launch Gates)
Description: Gate-based, launch-blocking roadmap for deterministic, fail-closed,
             auditable OSS correctness across Linux/Windows (Mac optional).
Purpose: Provide an agent-executable plan to reach launch-grade correctness
         without enterprise dependencies.
Dependencies:
  - Docs/testing/test_infrastructure_guide.md
  - Docs/testing/decision_gate_test_coverage.md
  - Docs/security/threat_model.md
  - Docs/architecture/decision_gate_runpack_architecture.md
  - Docs/architecture/decision_gate_evidence_trust_anchor_architecture.md
  - system-tests/test_registry.toml
============================================================================
Last Updated: 2026-01-28 (UTC)
============================================================================
-->

# Decision Gate Foundational Correctness Roadmap (OSS Launch Gates)

## Purpose

This roadmap defines **launch-blocking, gate-based** correctness requirements
for Decision Gate OSS. The goal is to reach a world-class bar in determinism,
fail-closed security, auditability, and cross-OS reproducibility **before any
external launch**.

## Scope (OSS Only)

**In scope**
- Decision Gate OSS crates, MCP server, built-in providers, runpacks, contracts,
  CLI, system tests, examples, and docs.
- Cross-OS determinism (Linux + Windows required; macOS optional but desired).

**Out of scope**
- Enterprise/DG-E features (private repo).
- SaaS billing/hosting, enterprise storage/auth/quota platforms.

## Non-Negotiables

1. **Determinism** across OSes for identical inputs.
2. **Fail-closed** semantics everywhere.
3. **Auditability** via runpacks + canonical hashing.
4. **No enterprise contamination** in OSS crates.
5. **World-class adversarial posture** (nation-state + hostile auditors).

---

# Gate Model

Every gate is **launch-blocking**. "Done" means all gates pass.

## Gate 0 — Baseline & Decision Log

**Goal:** Freeze scope, define invariants, and lock the launch contract.

**Must pass**
- Roadmap and invariants finalized.
- Test runner categories and priorities validated.
- Explicit stop-conditions written for each gate.

**Artifacts**
- This document.
- Updated test registry where needed.

---

## Gate 1 — Golden Runpacks + Cross-OS Determinism

**Goal:** Deterministic runpack generation + verification across OSes.

**Must pass**
- Golden scenario set with frozen inputs.
- Runpacks committed to repo and verified bit-for-bit.
- Linux + Windows produce identical root hashes and manifests.
- No timestamp drift: `generated_at` pinned (logical or fixed).

**Implementation**
- Golden runpacks stored under:
  - `system-tests/tests/fixtures/runpacks/golden/<scenario>/`
- One canonical manifest hash per scenario.
- Include a verification report and `runpack_verify` transcript.
- Golden scenario set (OSS-only, no OS-specific evidence):
  - `golden_time_after_pass` (time provider, deterministic completion)
  - `golden_visibility_packet` (entry packet + visibility metadata)

**Tests**
- New system-test: `golden_runpack_cross_os`
- CI matrix job: Linux + Windows comparisons.

---

## Gate 2 — Metamorphic Determinism

**Goal:** Same outcomes/runpacks even when event ordering changes.

**Must pass**
- Evidence arrival reordered ⇒ same decisions and runpack root hash.
- Provider call order randomized ⇒ same decision + runpack hash.
- Concurrent triggers ⇒ deterministic outcomes with idempotency.

**Implementation**
- New system-tests in `system-tests/tests/suites/determinism_metamorphic.rs`.
- Determinism checks compare runpack `integrity.root_hash` and decisions.

---

## Gate 3 — Canonicalization Contract

**Goal:** Explicit, audited canonicalization rules across schemas and runpacks.

**Must pass**
- JSON canonicalization via RFC 8785 (JCS) is enforced for all hashes.
- NaN/Infinity rejected or normalized (explicit rule).
- Floating-point behavior documented and tested.
- Ordering rules for collections are deterministic and tested.

**Implementation**
- Contract docs in `Docs/generated/decision-gate/*`.
- Tests in `decision-gate-core/tests/hashing.rs` (new if missing).

---

## Gate 4 — Trust Lanes & Precheck Correctness

**Goal:** Asserted vs verified evidence is enforced, precheck is read-only.

**Must pass**
- Verified-only predicates reject asserted evidence.
- Precheck never mutates run state.
- Lane mismatch yields structured `Unknown`.
- Lane overrides (config → predicate → gate) compose deterministically.

**Existing**
- `decision-gate-core/tests/precheck.rs`
- `decision-gate-core/tests/trust_lane.rs`

**Coverage**
- `system-tests/tests/suites/precheck.rs` (precheck read-only system test)

---

## Gate 5 — Provider Robustness (Built-ins + MCP)

**Goal:** Provider failures are safe, explicit, and deterministic.

**Built-ins (JSON/HTTP/Env/Time)**
- JSON: symlink traversal, path traversal, size limits, JSONPath fuzz.
- HTTP: TLS failures, redirect loops, timeout behavior, size limits.
- Env: missing vars, allow/deny list, size limits.
- Time: timezone correctness, boundary timestamps, logical vs unix.

**External MCP Providers**
- Slow provider, flaky provider, malformed schema, wrong namespace, bad hashes.

**Must pass**
- All failure modes return explicit, structured errors.
- Run state remains stable.
- Errors never silently downgrade to success.

**Coverage (so far)**
- JSON provider path traversal, symlink escape, invalid JSONPath, size limits.
- HTTP provider scheme enforcement, allowlist, redirects, timeouts, size limits, TLS failure.
- Env provider missing keys, allow/deny list, key/value size limits.
- Time provider logical/Unix enforcement + RFC3339 parsing.
- MCP provider malformed responses, text/empty results, flaky failures, namespace mismatch,
  signature-required failures, contract mismatch.

---

## Gate 6 — Runpack Integrity & Offline Verification

**Goal:** Runpacks are tamper-evident and safe to verify offline.

**Must pass**
- Path traversal + path length attacks blocked.
- Size limits enforced for all artifacts.
- Missing artifacts fail closed.
- Manifest hash mismatch fails closed.

**Existing**
- `decision-gate-core/tests/runpack.rs`
- `system-tests/tests/suites/runpack.rs`

**Missing**
- Cross-OS runpack verification gate (Gate 1).

---

## Gate 7 — Adversarial + Fuzzing

**Goal:** Structured fuzzing against scenario specs, evidence, and providers.

**Must pass**
- ScenarioSpec fuzzing (schema and comparator edge cases).
- Evidence payload fuzzing (type confusion, unicode).
- Provider response fuzzing (malformed, oversized, corrupt).

**Execution tiers**
- **PR tier:** bounded fuzz (short time budget).
- **Nightly tier:** long-running fuzz suite.

---

## Gate 8 — Performance & Scaling (OSS)

**Goal:** Bound resource use and avoid pathological regressions.

**Must pass**
- Performance smoke test runs (non-gated).
- Memory caps respected in runpack verification and stores.
- High-volume scenarios do not exceed configured limits.

**Note:** No enterprise HA/SLO targets here.

---

## Gate 9 — Packaging, Docs, and UX

**Goal:** OSS is usable, documented, and deterministic to operate.

**Must pass**
- Quick Start works end-to-end on Linux + Windows.
- Examples are runnable and deterministic.
- CLI runpack export/verify workflows are tested.
- Docs match runtime behavior (schemas, tooltips, provider contracts).

---

# Test Matrix (Mapping to Gates)

**Legend:** ✅ existing | 🟡 partial | ❌ missing

| Gate | Requirement | Status | Evidence |
| --- | --- | --- | --- |
| 1 | Golden runpack suite committed | ❌ | New fixtures + tests |
| 1 | Cross-OS determinism | ❌ | CI matrix (Linux/Windows) |
| 2 | Metamorphic determinism | ❌ | New system-tests |
| 3 | Canonicalization contract tests | 🟡 | Hashing present, edge cases missing |
| 4 | Trust lanes enforced | ✅ | core tests + docs |
| 5 | Provider hardening (built-ins) | 🟡 | Many tests, missing TLS/redirect/symlink/zone |
| 5 | External MCP adversarial harness | 🟡 | timeouts covered, others missing |
| 6 | Runpack integrity | ✅ | runpack + IO tests |
| 7 | Fuzzing | 🟡 | anchors + comparator only |
| 8 | Performance smoke | ✅ | ignored test |
| 9 | Docs + CLI usability | 🟡 | tests exist, examples/quickstart verification missing |

---

# Execution Plan (Agent-Executable)

## Phase A — Determinism & Runpacks
1. Define golden scenario set and freeze inputs.
2. Export runpacks on Linux + Windows.
3. Commit runpacks and add cross-OS comparison test.
4. Add CI job enforcing identical `root_hash` across OSes.

**Stop condition:** any mismatch in runpack root hash or manifest.

## Phase B — Metamorphic Determinism
1. Add tests with reordered evidence and provider calls.
2. Add concurrency tests with deterministic outcomes.

**Stop condition:** decision or runpack hash mismatch.

## Phase C — Provider Hardening
1. JSON provider symlink + traversal test.
2. HTTP provider TLS/redirect failure tests.
3. Time provider boundary + timezone tests.
4. External MCP adversarial harness.

**Stop condition:** any fail-open behavior or missing error metadata.

## Phase D — Fuzzing
1. Add fuzz harnesses for ScenarioSpec, Evidence payloads, Provider responses.
2. Wire into PR (short) + nightly (long) runners.

**Stop condition:** untriaged crash or nondeterministic outcome.

## Phase E — Docs + Examples
1. Validate Quick Start on Linux + Windows.
2. Regenerate schemas/tooltips/contracts as needed.
3. Ensure examples run with deterministic outputs.

**Stop condition:** doc example fails or drift from runtime behavior.

---

# CI/Validation Requirements

**Required OS:** Linux + Windows  
**Optional OS:** macOS (best-effort)

**Required jobs**
- `P0` system-tests
- `runpack` suite
- `determinism` suite
- `contract` suite
- cross-OS golden runpack comparison

---

# OSS Boundary Reminder

This roadmap is OSS-only. Any enterprise/DG-E platform work is explicitly
out-of-scope and must live in the private monorepo.
