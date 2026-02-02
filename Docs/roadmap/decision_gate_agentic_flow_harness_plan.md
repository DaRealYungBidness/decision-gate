<!--
Docs/roadmap/decision_gate_agentic_flow_harness_plan.md
============================================================================
Document: Decision Gate Agentic Flow Harness (Status + Live-Mode Plan)
Description: Clear status of what's implemented vs missing, plus the
             remaining live-mode requirements.
Purpose: Make launch cleanup obvious while preserving live-mode intent.
Dependencies:
  - Docs/roadmap/foundational_correctness_roadmap.md
  - Docs/roadmap/decision_gate_world_class_integration_readiness_plan.md
  - Docs/business/decision_gate_integration_landscape.md
  - Docs/security/threat_model.md
  - system-tests/TEST_MATRIX.md
============================================================================
Last Updated: 2026-02-02 (UTC)
============================================================================
-->

# Decision Gate Agentic Flow Harness (Status + Live-Mode Plan)

This doc is status-first. It answers:

1. What is implemented today?
2. What is explicitly not implemented?
3. Where are the entrypoints and files?

---

## Status Summary (as of 2026-02-02)

**Implemented (deterministic harness is real and enforced):**

- Canonical scenario registry + packs exist and are used by the harness.
- Deterministic harness runs scenarios across raw MCP + SDKs + adapters.
- Runpack root-hash determinism is enforced per scenario.
- Harness scripts + bootstrap exist and are wired to system-tests.
- Example packs are mirrored for onboarding.

**Not implemented (live-mode is still a plan):**

- No live-mode runner (no LLM calls, no allowlist, no live transcripts).
- Registry schema does not include live-mode controls (allowlists, llm flags).
- No live-mode CI wiring or nightly job.

If live-mode is still planned, keep this doc and use the checklist below.
If live-mode is being dropped, delete this doc after updating other roadmaps
that reference it.

---

## Implemented (Ground Truth, Current Files)

**Harness + entrypoints**

- `system-tests/tests/suites/agentic_harness.rs`
- `system-tests/tests/agentic.rs`
- `system-tests/test_registry.toml` (`agentic_flow_harness_deterministic`)
- `scripts/agentic_harness.sh` (deterministic runner)

**Scenario registry + packs + drivers**

- `system-tests/tests/fixtures/agentic/scenario_registry.toml`
- `system-tests/tests/fixtures/agentic/*`
- `system-tests/tests/fixtures/agentic/drivers/`

**Scripts + bootstrap + deps**

- `scripts/agentic_harness_bootstrap.sh`
- `system-tests/requirements-agentic.txt`

**Mirrored examples**

- `examples/agentic/`

---

## What Is In Scope Today (Deterministic Harness)

**Scenario registry**

- Registry defines scenarios, drivers, and modes. Today it contains **deterministic only**.
- Scenarios are asserted for expected status/outcome and deterministic runpack hashes.

**Drivers (all deterministic, tool-only)**

- `raw_mcp` baseline driver (direct MCP calls).
- `python_sdk` and `typescript_sdk` drivers.
- Adapter drivers: `langchain`, `crewai`, `autogen`, `openai_agents`.
- These drivers invoke MCP tools only; they do **not** make live LLM calls.

**Determinism enforcement**

- Each scenario has an expected runpack root hash in `expected/runpack_root_hash.txt`.
- Baseline is the raw MCP run; other drivers must match the same root hash.
- `UPDATE_AGENTIC_EXPECTED=1` refreshes expected hashes.

**Artifacts**

- System test artifacts: `summary.json`, `summary.md`, `agentic_results.json`.
- Per-driver stdout/stderr logs and runpack directories are emitted under the system test run root.

---

## Operational Notes (Deterministic Harness)

**How to run**

- `scripts/agentic_harness.sh --mode=deterministic`
- System test entrypoint:
  `cargo test -p system-tests --features system-tests --test agentic -- --exact agentic_harness::agentic_flow_harness_deterministic`

**Filters and env controls**

- `DECISION_GATE_AGENTIC_SCENARIOS` (comma-separated scenario ids)
- `DECISION_GATE_AGENTIC_DRIVERS` (comma-separated driver ids)
- `UPDATE_AGENTIC_EXPECTED=1` (refresh expected runpack hashes)
- `DECISION_GATE_SYSTEM_TEST_RUN_ROOT` (override artifacts root)

**Driver availability**

- Python/Node adapters are optional; if deps/runtime are missing, the driver is **skipped**.
- Skipped drivers do **not** fail the harness, but they reduce coverage.
- The `agentic` test category is `quick = false` (not part of quick CI).

---

## Not Implemented (Live-Mode Requirements)

These are the concrete gaps for live-mode.

**Runner + execution**

- Add a live-mode runner (LLM calls, allowlisted network access).
- Allow both deterministic and live in `scripts/agentic_harness.sh`.

**Registry schema (live controls)**
Add or enforce fields such as:

- `allow_network` + allowlist constraints
- `llm_required` + provider selection
- live-mode artifacts/transcripts

**Scenario packs (live variants)**

- Create live-mode variants where applicable.
- Ensure deterministic packs remain hermetic and unchanged.

**Artifacts + policy**

- Always capture transcripts and runpacks in live mode.
- Live-mode failures are report-only and never block deterministic gates.

**CI wiring**

- Add an opt-in or nightly job for live-mode.
- Keep deterministic gating unchanged.

---

## Canonical Scenario Set (Implemented)

These scenarios exist as deterministic packs and are in the registry:

- CI Gate (json + time + env)
- Artifact Integrity (http + json)
- Policy-Gated Disclosure (json + time)
- Attack Payload Rejection (json + env)
- Policy-Gated Fetch (http + json + env)
- Namespace / Policy Collision (env + time + policy mode)

---

## Next Actions (if Live-Mode is Still Planned)

1. Extend registry schema with live-mode fields and update parser.
2. Add live-mode runner + allowlist enforcement + transcript capture.
3. Create live-mode variants for each scenario (when applicable).
4. Add live-mode CI job (nightly or manual trigger).
5. Keep deterministic harness unchanged as the correctness gate.
