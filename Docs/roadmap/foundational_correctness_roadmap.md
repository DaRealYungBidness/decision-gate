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
- System-tests suite: `system-tests/tests/suites/golden_runpacks.rs`.
- CI matrix job: Linux + Windows comparisons (still required).

---

## Gate 2 — Metamorphic Determinism

**Goal:** Same outcomes/runpacks even when event ordering changes.

**Must pass**
- Evidence arrival reordered ⇒ same decisions and runpack root hash.
- Provider call order randomized ⇒ same decision + runpack hash.
- Concurrent triggers ⇒ deterministic outcomes with idempotency.

**Implementation**
- System-tests suite: `system-tests/tests/suites/metamorphic.rs`.
- Core unit tests: `decision-gate-core/tests/metamorphic_determinism.rs`.

---

## Gate 3 — Canonicalization Contract

**Goal:** Explicit, audited canonicalization rules across schemas and runpacks.

**Must pass**
- JSON canonicalization via RFC 8785 (JCS) is enforced for all hashes.
- NaN/Infinity rejected (explicit rule).
- Floating-point behavior documented and tested.
- Ordering rules for collections are deterministic and tested.

**Implementation**
- Contract docs in `Docs/generated/decision-gate/*`.
- Tests in `decision-gate-core/tests/hashing.rs`.

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
- `system-tests/tests/suites/precheck.rs`

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
- JSON provider: path traversal, symlink escape, invalid JSONPath, size limits.
- HTTP provider: scheme enforcement, allowlist, redirects, timeouts, size limits,
  TLS failure.
- Env provider: missing keys, allow/deny list, key/value size limits.
- Time provider: logical/Unix enforcement + RFC3339 parsing.
- MCP provider: malformed responses, text/empty results, flaky failures,
  namespace mismatch, signature-required failures, contract mismatch.

---

## Gate 6 — Runpack Integrity & Offline Verification

**Goal:** Runpacks are tamper-evident and safe to verify offline.

**Must pass**
- Path traversal + path length attacks blocked.
- Size limits enforced for all artifacts.
- Missing artifacts fail closed.
- Manifest hash mismatch fails closed.
- Backward compatibility: previous runpack versions still verify.

**Existing**
- `decision-gate-core/tests/runpack.rs`
- `system-tests/tests/suites/runpack.rs`

**Missing**
- Explicit backward compatibility test vectors.

---

## Gate 7 — Adversarial + Fuzzing + Log Safety

**Goal:** Structured fuzzing against scenario specs, evidence, providers, and
safe logging across failure paths.

**Must pass**
- ScenarioSpec fuzzing (schema and comparator edge cases).
- Evidence payload fuzzing (type confusion, unicode, oversized inputs).
- Provider response fuzzing (malformed, oversized, corrupt).
- Log leakage scanning for secrets across error paths and panics.

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
- Capacity targets defined (e.g., 10k runs, provider fan-out, large evidence).

**Note:** No enterprise HA/SLO targets here.

---

## Gate 9 — Packaging, Docs, and UX

**Goal:** OSS is usable, documented, and deterministic to operate.

**Must pass**
- Quick Start works end-to-end on Linux + Windows.
- Examples are runnable and deterministic.
- CLI runpack export/verify workflows are tested.
- Docs match runtime behavior (schemas, tooltips, provider contracts).
- Reproducible build guidance and version stamping are in place.

---

## Gate 10 — Agentic Flow Harness

**Goal:** End-to-end harness that simulates real agent orchestration.

**Must pass**
- Tool execution outside DG with MCP gating.
- JSON artifact generation, retries, disclosure packets.
- Final runpack export + verify.
- Canonical scenarios:
  - CI pipeline gate
  - Multi-step agent loop
  - Disclosure stage
  - Hallucinated evidence attempt
  - Namespace collision
  - Provider outage
  - Attack payload
  - Replay verification

---

# Test Matrix (Mapping to Gates)

**Legend:** ✅ existing | 🟡 partial | ❌ missing

| Gate | Requirement | Status | Evidence |
| --- | --- | --- | --- |
| 1 | Golden runpack suite committed | ✅ | Fixtures + `golden_runpacks` suite |
| 1 | Cross-OS determinism | ❌ | CI matrix (Linux/Windows) |
| 2 | Metamorphic determinism | 🟡 | Core + system tests exist |
| 3 | Canonicalization contract tests | ✅ | `decision-gate-core/tests/hashing.rs` |
| 4 | Trust lanes enforced | ✅ | Core + system tests |
| 5 | Provider hardening (built-ins) | 🟡 | Many tests, missing chaos matrix |
| 5 | External MCP adversarial harness | 🟡 | Timeout/flaky covered, more missing |
| 6 | Runpack integrity | ✅ | Runpack + IO tests |
| 6 | Runpack backward compatibility | ❌ | Add legacy vectors |
| 7 | Fuzzing breadth | 🟡 | Anchor/comparator only |
| 7 | Log leakage scanning | ❌ | Add log scrubbing tests |
| 8 | Performance smoke | ✅ | Ignored test |
| 8 | Capacity targets | ❌ | Add thresholds + gated bench |
| 9 | Docs + CLI usability | 🟡 | Examples/Quick Start verification missing |
| 10 | Agentic flow harness | ❌ | Harness + scenarios |

---

# Execution Plan (Agent-Executable)

## Phase A — Determinism & Runpacks
1. Define golden scenario set and freeze inputs.
2. Export runpacks on Linux + Windows.
3. Commit runpacks and add cross-OS comparison test.
4. Add CI job enforcing identical `root_hash` across OSes.

**Stop condition:** any mismatch in runpack root hash or manifest.

## Phase B — Metamorphic Determinism
1. Expand reordering, provider shuffle, and concurrency cases.
2. Compare decisions + runpack hashes.

**Stop condition:** decision or runpack hash mismatch.

## Phase C — Provider Hardening + Chaos
1. Add chaos/fault injection matrix (TLS oddities, redirect loops, slow-loris,
   mid-stream truncation).
2. Extend JSONPath fuzzing for JSON provider.
3. Extend MCP provider schema/namespace/hash mismatches.

**Stop condition:** any fail-open behavior or missing error metadata.

## Phase D — Runpack Compatibility + Durability
1. Add legacy runpack vectors and verify compatibility.
2. Add sqlite durability tests (crash/partial write/rollback).

**Stop condition:** verification regression or data loss scenario.

## Phase E — Fuzzing + Log Safety
1. Add fuzz harnesses for ScenarioSpec, Evidence payloads, Provider responses.
2. Wire PR (short) + nightly (long) runners.
3. Add log leakage scanning for secrets in error paths.

**Stop condition:** untriaged crash or nondeterministic outcome.

## Phase F — Performance + Scaling
1. Define capacity targets and thresholds.
2. Add gated benchmarks for load, fan-out, large evidence payloads.

**Stop condition:** regression against defined limits.

## Phase G — Docs + Release Hardening
1. Validate Quick Start on Linux + Windows.
2. Regenerate schemas/tooltips/contracts as needed.
3. Ensure examples run with deterministic outputs.
4. Add reproducible build notes + version stamping.

**Stop condition:** doc example fails or drift from runtime behavior.

## Phase H — Agentic Harness
1. Build harness simulating agent orchestration.
2. Implement canonical scenarios and replay verification.

**Stop condition:** harness scenario failure or nondeterministic runpack.

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
- fuzz PR tier (short budget)

**Nightly jobs**
- fuzz long tier
- chaos provider matrix
- capacity benchmarks

---

# OSS Boundary Reminder

This roadmap is OSS-only. Any enterprise/DG-E platform work is explicitly
out-of-scope and must live in the private monorepo.
