<!--
Docs/roadmap/decision_gate_agentic_flow_harness_plan.md
============================================================================
Document: Decision Gate Agentic Flow Harness + Scenario Library Plan
Description: End-to-end harness design for exercising DG across SDKs, adapters,
             and live LLM workflows with deterministic + live modes.
Purpose: Define a world-class, invariance-aligned plan for agentic flows.
Dependencies:
  - Docs/roadmap/foundational_correctness_roadmap.md
  - Docs/roadmap/decision_gate_world_class_integration_readiness_plan.md
  - Docs/business/decision_gate_integration_landscape.md
  - Docs/security/threat_model.md
  - system-tests/TEST_MATRIX.md
============================================================================
Last Updated: 2026-01-29 (UTC)
============================================================================
-->

# Decision Gate Agentic Flow Harness + Scenario Library Plan

## Executive Intent

Decision Gate must demonstrate **ecosystem-wide integration** without
privileging any single framework. The agentic flow harness makes that true by
running the **same canonical scenarios** across **every projection** (SDKs,
framework adapters, raw MCP/HTTP) in **two modes**:

- **Deterministic mode:** fully reproducible, fail-closed, no external network
  dependencies.
- **Live mode:** real LLM + real network calls for product learning, ergonomics,
  and user empathy.

This plan defines the harness, the scenario library, and the release gates that
turn “integration surface” into “integration proof.”

---

## Alignment: Doctrine of Invariance

The Asset-Core Doctrine of Invariance demands that all projections are derived
from a single invariant. For Decision Gate, the invariant is **the canonical
scenario pack** plus the **contract-driven tool surface**. The harness ensures
that every projection (SDKs, adapters, raw MCP) is a functorial mapping of the
same underlying contract and scenarios.

Invariance constraints for this plan:

1. **Single scenario source:** one canonical scenario library feeds every
   projection.
2. **Commutativity:** every driver produces equivalent decisions + runpack
   hashes for identical inputs.
3. **No manual drift:** scenario metadata, expected outcomes, and validation
   rules are generated or enforced via registry, not copied by hand.

---

## Definitions

**Agentic Flow Harness**
A deterministic runner that orchestrates:
- a DG server
- a projection driver (SDK/adapter/raw MCP)
- a scenario pack (spec/run-config/trigger/fixtures)
- verification artifacts (runpack + tool transcript + summary)

**Scenario Pack**
A self-contained bundle of:
- scenario spec + run-config + trigger(s)
- evidence fixtures (JSON, env vars, stub HTTP payloads)
- expected decision + runpack hash
- driver compatibility + mode constraints

**Projection Driver**
A thin runner for a specific integration surface:
- Python SDK
- TypeScript SDK
- Framework adapters (LangChain/CrewAI/AutoGen/OpenAI Agents SDK)
- Raw MCP HTTP (baseline control)

---

## Inventory: Current Integration Surfaces (with code locations)

### Harnesses + Runners (existing)

| Area | Purpose | Location | Status |
| --- | --- | --- | --- |
| System-test registry runner | Registry-driven test execution, artifact capture | `scripts/test_runner.py` | Implemented |
| System-test suite harness | End-to-end tests against live MCP server | `system-tests/` | Implemented |
| Adapter test harness | Runs framework adapter examples in isolated venv | `scripts/adapter_tests.sh` | Implemented |
| Verification orchestrator | Unified local CI entrypoint | `scripts/verify_all.sh` | Implemented |
| Packaging dry-run | Build/install SDKs without publishing | `scripts/package_dry_run.sh` | Implemented |
| Golden runpack cross-OS | Cross-OS determinism gate | `.github/workflows/golden_runpack_cross_os.yml` | Implemented |

### SDKs + Generators

| Surface | Purpose | Location | Status |
| --- | --- | --- | --- |
| Python SDK | Generated MCP client + helpers | `sdks/python` | Implemented |
| TypeScript SDK | Generated MCP client + helpers | `sdks/typescript` | Implemented |
| SDK generator | Contract-driven SDK generation | `decision-gate-sdk-gen` | Implemented |

### Framework Adapters

| Framework | Adapter | Location | Status |
| --- | --- | --- | --- |
| LangChain | Tool wrapper | `adapters/langchain` | Implemented |
| CrewAI | Tool wrapper | `adapters/crewai` | Implemented |
| AutoGen | Function tool | `adapters/autogen` | Implemented |
| OpenAI Agents SDK | Tool wrapper | `adapters/openai_agents` | Implemented |

### Examples (SDK + Framework)

| Example Set | Purpose | Location | Status |
| --- | --- | --- | --- |
| Python SDK examples | Scenario lifecycle + CI + precheck | `examples/python` | Implemented |
| TypeScript SDK examples | Scenario lifecycle + CI + precheck | `examples/typescript` | Implemented |
| Framework adapter examples | Framework tool construction + DG calls | `examples/frameworks` | Implemented |
| LLM disclosure example | Packet dispatch to callback sink | `examples/llm-scenario` | Implemented |

### Built-in Providers (core dependency for scenarios)

| Provider | Location | Status |
| --- | --- | --- |
| time | `decision-gate-providers` | Implemented |
| env | `decision-gate-providers` | Implemented |
| json | `decision-gate-providers` | Implemented |
| http | `decision-gate-providers` | Implemented |

---

## What Is Missing (from a world-class harness perspective)

1. **Canonical scenario library** spanning real-world problems across providers.
2. **A unified agentic flow harness** that runs those scenarios via every
   projection (SDKs + adapters + raw MCP), with deterministic + live modes.
3. **Mode-specific execution controls** (no-network deterministic vs. live LLM).
4. **Scenario registry schema** to prevent drift and manual duplication.

---

## Scenario Library Design

### Scenario Registry (proposed)

Location (proposed): `system-tests/tests/fixtures/agentic/scenario_registry.toml`

Each entry defines:

- `scenario_id`
- `description`
- `providers` (e.g., `json`, `http`, `env`, `time`)
- `drivers` (sdk, adapters, raw MCP)
- `modes` (`deterministic`, `live`)
- `expected_decision` + `expected_runpack_hash`
- `artifacts` (tool transcript, runpack, summaries)
- `allow_network` (boolean + allowlist)
- `llm_required` (boolean + provider)

### Scenario Pack Layout (proposed)

```
system-tests/tests/fixtures/agentic/
  <scenario_id>/
    spec.json
    run_config.json
    trigger.json
    fixtures/
      evidence.json
      env.json
      http_stub.json
    expected/
      decision.json
      runpack_root_hash.txt
```

`expected/runpack_root_hash.txt` is authoritative for deterministic hashing.
When regenerating fixtures, update it via the harness update flag
(`UPDATE_AGENTIC_EXPECTED=1`).

### Canonical Scenario Set (initial)

These scenarios are intentionally **real-world** and collectively exercise
**every built-in provider at least once**. Each scenario is included because
it demonstrates a production-relevant workflow, surfaces integration ergonomics,
and validates deterministic evidence handling.

1. **CI Gate** (json + time + env)  
   **Why:** CI/CD gating is the most common real-world requirement check. This
   scenario proves DG can gate on structured test results (json), enforce time
   windows (time), and respect deployment environment flags (env).

2. **Artifact Integrity** (http + json)  
   **Why:** Production workflows often rely on external evidence. This scenario
   exercises HTTP evidence collection with bounded responses (http) and strict
   hash/shape validation (json), demonstrating secure supply-chain checks.

3. **Policy-Gated Disclosure** (json + time)  
   **Why:** Demonstrates packet disclosure with time-based gating. This is a
   core DG value: controlled disclosure only after predicates pass, with
   audit-grade artifacts.

4. **Attack Payload Rejection** (json + env)  
   **Why:** Ensures fail-closed behavior on malformed or adversarial evidence
   (json) while enforcing strict configuration modes via environment signals
   (env). This scenario validates security posture and error clarity.

5. **Policy-Gated Fetch** (http + json)  
   **Why:** A realistic “external fetch + verify” flow. Proves that untrusted
   inputs retrieved over HTTP are still deterministically validated before
   decisions advance.

6. **Namespace / Policy Collision** (env + time + policy)  
   **Why:** Multi-tenant correctness is non-negotiable. This scenario proves
   namespace and policy constraints are enforced even when time-based predicates
   would otherwise pass.

Each scenario must have:
- deterministic fixtures
- live-mode variant (if applicable)
- explicit expected decision + runpack hash

---

## Harness Architecture (proposed)

### Runner Topology

```
Scenario Registry
  -> Harness Orchestrator
     -> DG server (local)
     -> Projection drivers (SDKs, adapters, raw MCP)
     -> Evidence environment (fixtures or live endpoints)
     -> Verification (decision + runpack hash + tool transcript)
```

### Driver Interface (proposed)

Each driver must implement:

- `setup()` (env, dependencies, framework init)
- `execute_scenario(scenario_pack)`
- `collect_artifacts()`
- `teardown()`

Driver implementations live in `system-tests/tests/fixtures/agentic/drivers/`
with language-specific helpers under `system-tests/tests/fixtures/agentic/`.

### Parallelism

Harness should support **parallel independent runs**, with:
- port reservation (same allocator used in system-tests)
- isolated run roots
- deterministic per-run temp directories

---

## Deterministic vs Live Mode

### Deterministic Mode (PR / CI)

- No external network access
- Stub HTTP server with fixture payloads
- Logical time only
- LLM calls forbidden
- Strict artifact + hash assertions

### Live Mode (Nightly / Opt-in)

- Real LLM calls (configurable provider + API key)
- External HTTP allowlist required
- Full transcript capture for analysis
- Non-gating failures allowed (report-only)

**Provider swap requirement:** live-mode must treat the LLM as a pluggable
driver. The harness must support multiple model backends (frontier labs,
self-hosted, OSS, or custom) without changing scenario definitions. The LLM
integration is a runtime concern, not a scenario concern.

---

## Live-Mode: Purpose, Boundaries, and Success Criteria

### What Live-Mode Is For

Live-mode exists to validate **integration reality**, not correctness. It answers
questions that deterministic tests cannot:

- Do frameworks actually call DG when prompted or instructed?
- Are tool descriptions and prompts sufficient for correct tool invocation?
- Are auth, rate limits, and errors legible to agent loops?
- Do SDKs/adapters behave under real LLM tool usage?
- Does the flow feel natural to a developer using the framework?

### What Live-Mode Is NOT For

Live-mode is **not** a correctness gate and **must not** be used to assert:

- Deterministic outcomes
- Behavioral correctness of LLMs
- Security guarantees or fail-closed logic
- Release readiness

### Live-Mode Success Criteria (Minimal + Mechanical)

Live-mode should be judged only on mechanical integration signals:

1. **DG tool calls occur end-to-end** through each driver.
2. **No contract/schema errors** on tool invocation.
3. **Auth/config flows** behave as documented.
4. **Runpack export** succeeds and is readable.
5. **Transcripts + logs** are human-auditable and actionable.

If these are satisfied, live-mode has done its job. Anything beyond that is
exploratory and non-gating.

### Live-Mode Failure Policy

- Live-mode failures are **report-only** and never block deterministic gates.
- Failures must still emit full transcripts and runpack artifacts.
- Failures must surface actionable error context (no silent retries).

---

## Execution Integration

### System-test Integration

- Add a new **agentic harness suite** under `system-tests/tests/suites/`
- Register in `system-tests/test_registry.toml`
- Extend `scripts/test_runner.py` to support scenario registry execution

**Implemented locations:**
- Suite: `system-tests/tests/suites/agentic_harness.rs`
- Entry point: `system-tests/tests/agentic.rs`
- Registry: `system-tests/tests/fixtures/agentic/scenario_registry.toml`
- Scenario packs: `system-tests/tests/fixtures/agentic/`
- Mirrored packs: `examples/agentic/`

### Optional Local Entry Point

- `scripts/agentic_harness.sh` (new) to run deterministic or live modes
- `scripts/agentic_harness_bootstrap.sh` (new) to install Python adapter deps
- `system-tests/requirements-agentic.txt` (new) pins framework driver deps
- Wired into `scripts/verify_all.sh --agentic-harness=...`

---

## Network Stance (World-Class Requirements)

**Deterministic mode:** no external network access. All HTTP evidence must be
served from local fixtures or stub servers. This guarantees reproducibility,
cross-OS determinism, and audit-grade invariance. Stub servers must use
deterministic ports (or stable host placeholders) so runpack hashes do not drift
between runs.

**Live mode:** external network access is allowed **only** under explicit
allowlists (hostname + size limits + timeouts). This provides realistic
integration feedback without compromising safety, determinism, or auditability.

This stance is mandatory for a world-class launch: deterministic correctness
must remain hermetic, while live-mode remains controlled and non-gating.

---

## Scenario Pack Visibility (Docs + Examples)

Scenario packs are the authoritative fixtures for both testing and onboarding.
They must live in the system-tests fixtures **and** be mirrored in examples so
developers can discover them without digging into system-tests.

Proposed layout:

- Source of truth: `system-tests/tests/fixtures/agentic/`
- Mirrored examples: `examples/agentic/` (clearly labeled as derived)
- Documentation must note that examples are mirrored from system-tests

---

## World-Class Quality Bar (Non-Negotiable)

1. **Every deterministic scenario passes across every driver**  
   Raw MCP, Python SDK, TypeScript SDK, and each framework adapter.

2. **Runpack hash invariance across projections and OS**  
   Equivalent inputs produce identical root hashes and manifests.

3. **Zero drift from contract**  
   All SDKs, adapters, and examples must be contract-derived, with generator
   checks enforced in CI.

4. **Fail-closed by default**  
   Any error, mismatch, or missing data produces explicit failure, never
   silent fallback.

5. **Live-mode is non-gating but fully auditable**  
   Transcripts + runpacks are always captured even on failure.

This bar exists to support a “Day 1 adoptable, best-in-class” launch posture
and to minimize long-term maintenance by eliminating ambiguity and drift.
---

## Security + Risk Controls

- Explicit network allowlists in live mode
- Secrets never logged (redaction enforced)
- Rate limits for live LLM calls
- Per-run evidence size limits
- Structured failure logging for diagnosis

---

## Milestones + Acceptance Criteria

### Phase 0 — Design Freeze
- Scenario registry schema defined and reviewed
- Harness interface defined (drivers + orchestrator)

### Phase 1 — Deterministic Harness Skeleton
- Harness runs a single scenario via raw MCP + Python SDK
- Emits deterministic artifacts + runpack hash
- Integrated into `system-tests/test_registry.toml`

### Phase 2 — Scenario Library v1
- 3–5 canonical scenarios implemented
- Each has deterministic fixtures + expected decisions + hashes

### Phase 3 — Multi-Projection Coverage
- Python SDK + TypeScript SDK drivers
- Framework adapter drivers (LangChain/CrewAI/AutoGen/OpenAI Agents)
- Baseline raw MCP driver

### Phase 4 — Live LLM Mode
- Separate config and allowlist
- Opt-in runner (nightly)
- Artifact + transcript capture

### Phase 5 — Publish Readiness
- All deterministic scenarios pass across drivers
- Cross-OS runpack parity confirmed
- Docs updated + examples align with scenarios

---

## Open Questions (to resolve before implementation)

1. Which LLM providers are allowed in live mode (OpenAI, Anthropic, etc.)?
2. Which scenarios should be **mandatory** for all drivers vs. subset?
3. What is the minimal acceptable set of canonical scenarios for V1?
4. How strict should live-mode pass criteria be (report-only vs. gating)?
5. Should scenario packs live under `system-tests` or `examples` (or both)?

---

## Proposed Doc Updates (once implementation starts)

- Update `Docs/roadmap/foundational_correctness_roadmap.md` (Gate 10)
- Update `Docs/roadmap/decision_gate_world_class_integration_readiness_plan.md`
- Add harness references to `Docs/business/decision_gate_integration_landscape.md`
