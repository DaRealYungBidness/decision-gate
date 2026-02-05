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
Last Updated: 2026-02-04 (UTC)
============================================================================
-->

# Decision Gate Foundational Correctness Roadmap (OSS Launch Gates)

## Purpose

This roadmap defines **launch-blocking, gate-based** correctness requirements
for Decision Gate OSS. The goal is to reach a high bar in determinism,
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
5. **High-bar adversarial posture** (nation-state + hostile auditors).

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
- Determinism replay suite: `system-tests/tests/suites/determinism.rs` (AssetCore fixture replay).
- Cross-OS CI matrix: `.github/workflows/golden_runpack_cross_os.yml` runs on Linux + Windows.

**Missing / gaps**

- Golden runpack fixtures include `verifier_report.json` but do not capture a committed
  `runpack_verify` transcript artifact alongside the golden runpacks.

---

## Gate 2 — Metamorphic Determinism

**Goal:** Same outcomes/runpacks even when event ordering changes.

**Must pass**

- Evidence arrival reordered ⇒ same decisions and runpack root hash.
- Provider call order randomized ⇒ same decision + runpack hash.
- Concurrent triggers ⇒ deterministic outcomes with idempotency.

**Implementation**

- System-tests suite: `system-tests/tests/suites/metamorphic.rs`.
- Core unit tests: `crates/decision-gate-core/tests/metamorphic_determinism.rs`.

**Existing**

- Core: canonical gate-eval evidence ordering (`crates/decision-gate-core/tests/metamorphic_determinism.rs`).
- System: concurrent runs yield identical runpack hashes (`system-tests/tests/suites/metamorphic.rs`).
- Related reliability coverage: trigger/submission idempotency (`system-tests/tests/suites/reliability.rs`).
- Core: deterministic condition evaluation shuffles preserve runpack hash and evidence order
  (`crates/decision-gate-core/tests/metamorphic_determinism.rs`).
- System: evidence ordering is canonical across multiple trigger evaluations
  (`system-tests/tests/suites/metamorphic.rs`).
- System: concurrent trigger ordering produces deterministic records
  (`system-tests/tests/suites/metamorphic.rs`).

**Missing**

- None.

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
- Tests in `crates/decision-gate-core/tests/hashing.rs`.

---

## Gate 4 — Trust Lanes & Precheck Correctness

**Goal:** Asserted vs verified evidence is enforced, precheck is read-only.

**Must pass**

- Verified-only conditions reject asserted evidence.
- Precheck never mutates run state.
- Lane mismatch yields structured `Unknown`.
- Lane overrides (config → condition → gate) compose deterministically.

**Existing**

- `crates/decision-gate-core/tests/precheck.rs`
- `crates/decision-gate-core/tests/trust_lane.rs`
- `system-tests/tests/suites/precheck.rs`
- `system-tests/tests/suites/validation.rs`

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

- JSON provider: unit tests for path traversal, size limits, invalid JSON/YAML, JSONPath fuzz
  (`crates/decision-gate-providers/tests/json_provider.rs`, `crates/decision-gate-providers/tests/proptest_json.rs`)
  + system-test symlink escape coverage.
- HTTP provider: HTTPS enforcement, allowlist/SSRF prevention, redirect handling/loops, timeouts,
  response size limits, TLS failure, slow-loris, truncation, and body hash check tests.
- Env provider: missing keys, allow/deny list, key/value size limits.
- Time provider: logical/Unix enforcement + RFC3339 parsing/invalid inputs + timezone offsets +
  epoch boundary cases.
- MCP provider: malformed JSON-RPC, text/empty results, flaky responses, namespace mismatch,
  missing/unauthorized/invalid signatures, contract mismatch, timeout enforcement.
- MCP provider: evidence hash mismatch vs signed payload.
- Provider templates: tools/list + tools/call + header/body size limits + fail-closed framing.
- Provider discovery: HTTP + stdio discovery tools, denylist enforcement, response size limits.

**Missing / gaps**

- None.

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

- `crates/decision-gate-core/tests/runpack.rs`
- `crates/decision-gate-mcp/tests/runpack_io.rs`
- `crates/decision-gate-cli/tests/runpack_commands.rs`
- `system-tests/tests/suites/runpack.rs` (export/verify + object store)
- `system-tests/tests/suites/sqlite_registry_runpack.rs`
- Back-compat fixture verification against committed `v1` runpack artifacts
  (`system-tests/tests/fixtures/runpacks/backcompat/v1`, `system-tests/tests/suites/runpack.rs`).
- Verifier rejects unsupported `manifest_version` values (fail-closed).

**Missing**

- None.

---

## Gate 7 — Adversarial + Fuzzing + Log Safety

**Goal:** Structured fuzzing against scenario specs, evidence, providers, and
safe logging across failure paths.

**Must pass**

- ScenarioSpec fuzzing (schema and comparator edge cases).
- Evidence payload fuzzing (type confusion, unicode, oversized inputs).
- Provider response fuzzing (malformed, oversized, corrupt).
- Log leakage scanning for secrets across error paths and panics.

**Current coverage**

- Anchor fuzz cases for malformed/oversize anchors (`system-tests/tests/suites/anchor_fuzz.rs`).
- Schema registry cursor/schema validation fuzz cases (`system-tests/tests/suites/schema_registry_fuzz.rs`).
- Audit log redaction + hash-only precheck audit events (partial log safety).
- Log leakage scanning for secrets across error paths/panics
  (`system-tests/tests/suites/log_leak_scan.rs`).

**Execution tiers**

- **PR tier:** bounded fuzz (short time budget).
- **Nightly tier:** long-running fuzz suite.

**Missing / gaps**

- Provider response fuzzing harnesses for malformed/oversized/corrupt provider outputs.
- Nightly fuzz runners are not wired into CI.

---

## Gate 8 — Performance & Scaling (OSS)

**Goal:** Bound resource use and avoid pathological regressions.

**Must pass**

- Performance smoke test runs (non-gated).
- Memory caps respected in runpack verification and stores.
- High-volume scenarios do not exceed configured limits.
- Capacity targets defined (e.g., 10k runs, provider fan-out, large evidence).

**Note:** No enterprise HA/SLO targets here.

**Current coverage**

- Performance smoke test (ignored) in `system-tests/tests/suites/performance.rs`.
- Stress tests for registry concurrency, paging stability, and precheck storms
  (`system-tests/tests/suites/stress.rs`).

**Missing / gaps**

- Capacity targets and thresholds are not encoded in tests or CI (stress tests exist,
  but no pass/fail limits are defined).

---

## Gate 9 — Packaging, Docs, and UX

**Goal:** OSS is usable, documented, and deterministic to operate.

**Must pass**

- Quick Start works end-to-end on Linux + Windows.
- Examples are runnable and deterministic.
- CLI runpack export/verify workflows are tested.
- Docs match runtime behavior (schemas, tooltips, provider contracts).
- Reproducible build guidance and version stamping are in place.

**Coverage (so far)**

- CLI end-to-end workflows (serve + runpack export/verify + authoring) in
  `system-tests/tests/suites/cli_workflows.rs`.
- SDK examples runnable (Python/TypeScript, including agent-loop + precheck) in
  `system-tests/tests/suites/sdk_examples.rs`.
- SDK client lifecycle + auth tests in `system-tests/tests/suites/sdk_client.rs`.
- SDK generator CLI parity in `system-tests/tests/suites/sdk_gen_cli.rs`.
- Contract/schema conformance in `system-tests/tests/suites/contract.rs`.
- Provider discovery workflows in `system-tests/tests/suites/provider_discovery.rs`.

**Missing / gaps**

- Quick Start validation on Windows (manual today).
- Reproducible build guidance + version stamping (see `Docs/roadmap/README.md`).

---

## Gate 10 — Agentic Flow Harness

**Goal:** End-to-end harness that simulates real agent orchestration.

**Must pass**

- Deterministic agentic harness runs a canonical scenario library across
  **every projection** (raw MCP + Python SDK + TypeScript SDK + all adapters).
- Runpack hash invariance across projections and OS for deterministic mode.
- Scenario packs are canonical, registry-driven, and mirrored into examples.
- Live-mode harness exists as a **report-only** integration reality check:
  - pluggable LLM providers
  - allowlisted network access
  - full transcript capture
  - never gates correctness
- Canonical scenarios (V1 minimum, provider-complete):
  - CI pipeline gate (json + time + env)
  - Artifact integrity (http + json)
  - Policy-gated disclosure (json + time)
  - Attack payload rejection (json + env)
  - Policy-gated fetch (http + json + env)
  - Namespace/policy collision (env + time)

**Current coverage**

- Agent loop examples in `examples/agent-loop/` and SDK examples
  (`examples/python/agent_loop.py`, `examples/typescript/agent_loop.ts`).
- SDK example suite exercises agent loop flows; deterministic harness covers multi-projection parity.
- Deterministic agentic harness implemented in
  `system-tests/tests/suites/agentic_harness.rs`, backed by a registry in
  `system-tests/tests/fixtures/agentic/scenario_registry.toml`, canonical packs
  under `system-tests/tests/fixtures/agentic/`, and mirrored packs in
  `examples/agentic/`.
- Deterministic mode is hermetic (stub HTTP + deterministic ports + fail-closed
  run roots), with cross-driver runpack hash validation.

**Missing / gaps**

- Live-mode harness (LLM provider swap, allowlisted network, transcripts).
- Registry coverage for live-mode runs and report-only results.
- Cross-OS parity for agentic harness in CI.

---

# Test Matrix (Mapping to Gates)

**Legend:** ✅ existing | 🟡 partial | ❌ missing

| Gate | Requirement                      | Status | Evidence                                |
| ---- | -------------------------------- | ------ | --------------------------------------- |
| 1    | Golden runpack suite committed   | ✅     | Fixtures + `golden_runpacks` suite      |
| 1    | Cross-OS determinism             | ✅     | `golden_runpack_cross_os` workflow + fixtures |
| 1    | Runpack verify transcript committed | ❌   | Golden fixtures include `verifier_report.json` only |
| 2    | Metamorphic determinism          | ✅     | Core evidence order + deterministic shuffle + system trigger ordering |
| 3    | Canonicalization contract tests  | ✅     | `crates/decision-gate-core/tests/hashing.rs`   |
| 4    | Trust lanes enforced             | ✅     | Core + system tests                     |
| 5    | Provider hardening (built-ins)   | ✅     | Unit + system tests + chaos matrix      |
| 5    | External MCP adversarial harness | ✅     | MCP provider + templates + timeouts + hash tamper |
| 6    | Runpack integrity                | ✅     | Core + MCP IO + CLI + system tests      |
| 6    | Runpack backward compatibility   | ❌     | Add legacy vectors                      |
| 7    | Fuzzing breadth                  | 🟡     | Spec/evidence/comparator/JSONPath/anchor/schema coverage; provider response fuzz missing |
| 7    | Log leakage scanning             | ✅     | `log_leak_scan` suite + audit checks    |
| 8    | Performance smoke                | ✅     | Ignored test                            |
| 8    | Capacity targets                 | ❌     | Stress tests only; no thresholds        |
| 9    | Docs + CLI usability             | 🟡     | CLI + SDK + contract suites; docs gaps  |
| 10   | Agentic flow harness             | 🟡     | Deterministic harness + registry + packs; live-mode missing |

---

# Execution Plan (Agent-Executable)

## Phase A — Determinism & Runpacks

**Current status:** golden fixtures + `golden_runpack_cross_os` suite exist; determinism replay
suite present; cross-OS CI enforcement is wired via `.github/workflows/golden_runpack_cross_os.yml`.

1. Maintain golden fixtures when contract/runtime changes (`UPDATE_GOLDEN_RUNPACKS=1`).
2. Keep cross-OS workflow green and investigate any manifest/hash drift.

**Stop condition:** any mismatch in runpack root hash or manifest.

## Phase B — Metamorphic Determinism

**Current status:** evidence ordering canonical in core + concurrent runpack hash check;
deterministic shuffle coverage and multi-trigger ordering cases implemented.

1. Maintain reorder/shuffle and concurrency cases as providers and evaluation logic evolve.
2. Compare decisions + runpack hashes.

**Stop condition:** decision or runpack hash mismatch.

## Phase C — Provider Hardening + Chaos

**Current status:** broad unit + system coverage for built-ins and MCP providers; chaos matrix
and JSONPath fuzzing are implemented (TLS oddities, redirect loops, slow-loris,
mid-stream truncation).

1. Maintain chaos/fault injection matrix coverage as providers evolve.
2. Maintain JSONPath fuzzing corpus for the JSON provider.
3. Extend MCP provider schema/namespace/hash mismatches as new cases appear
   (signature/hash mismatch covered).

**Stop condition:** any fail-open behavior or missing error metadata.

## Phase D — Runpack Compatibility + Durability

**Current status:** runpack IO + sqlite persistence tests exist; crash/rollback durability
tests are implemented; legacy vectors are still missing.

1. Add legacy runpack vectors and verify compatibility.

**Stop condition:** verification regression or data loss scenario.

## Phase E — Fuzzing + Log Safety

**Current status:** deterministic fuzz coverage exists for ScenarioSpec, Evidence payloads,
comparators, JSONPath, anchors, and schema registry; log-leak scanner is implemented.
Long-running fuzz harnesses and nightly runners are still missing.

1. Add long-running fuzz harnesses and wire nightly runners (optional).
2. Expand fuzz corpora as new edge cases appear.

**Stop condition:** untriaged crash or nondeterministic outcome.

## Phase F — Performance + Scaling

**Current status:** performance smoke + stress tests exist; no capacity targets or gated benchmarks.

1. Define capacity targets and thresholds.
2. Add gated benchmarks for load, fan-out, large evidence payloads.

**Stop condition:** regression against defined limits.

## Phase G — Docs + Release Hardening

**Current status:** CLI workflows + SDK examples/tests + contract suite exist; Windows Quick Start
and reproducible build notes pending.

1. Validate Quick Start on Linux + Windows.
2. Regenerate schemas/tooltips/contracts as needed.
3. Ensure examples run with deterministic outputs.
4. Add reproducible build notes + version stamping.

**Stop condition:** doc example fails or drift from runtime behavior.

## Phase H — Agentic Harness

**Current status:** deterministic agentic harness + canonical scenarios are implemented and
mirrored into examples; live-mode harness and cross-OS CI parity are still missing.

1. Implement live-mode harness (LLM provider swap, allowlisted network, transcripts).
2. Add cross-OS agentic harness parity in CI (Linux + Windows).

**Stop condition:** harness scenario failure or nondeterministic runpack.

---

# CI/Validation Requirements

**Required OS:** Linux + Windows  
**Optional OS:** macOS (best-effort)

**Required jobs**

- All `P0` entries in `system-tests/test_registry.toml`
  (smoke, runpack, contract, security, operations, reliability, providers).
- `reliability` suite (includes determinism + metamorphic + persistence).
- `runpack` suite (includes golden runpack verification).
- `contract` suite.
- `providers` suite.
- Cross-OS `golden_runpack_cross_os` on Linux + Windows.
- Anchor/schema registry fuzz (short budget).

**Nightly jobs**

- fuzz long tier (once harnesses exist)
- chaos provider matrix
- capacity benchmarks

---

# OSS Boundary Reminder

This roadmap is OSS-only. Any enterprise/DG-E platform work is explicitly
out-of-scope and must live in the private monorepo.
