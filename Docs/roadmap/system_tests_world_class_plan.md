<!--
Docs/roadmap/system_tests_world_class_plan.md
============================================================================
Document: Decision Gate System-Tests World-Class Plan
Description: Detailed plan for a system-tests crate, registry, gaps, and tooling.
Purpose: Define scope, structure, and implementation steps before execution.
Dependencies:
  - Docs/roadmap/open_items.md
  - Docs/security/threat_model.md
  - Docs/standards/codebase_engineering_standards.md
  - Docs/standards/codebase_formatting_standards.md
============================================================================
-->

# Decision Gate System-Tests World-Class Plan

## Overview
This plan defines a world-class system-tests crate for Decision Gate. The intent is to
mirror the rigor and introspection of Asset-Core while tailoring categories, harnesses,
and artifacts to Decision Gate’s control-plane + MCP surface. This is a full end-to-end
system-test suite with explicit registry coverage, gap tracking, and automated docs.

## Goals (Aligned With Project Desires)
- Public-facing rigor: a repo inspection should signal strong engineering discipline.
- Determinism + auditability: every test emits artifacts that are verifiable and replayable.
- Single source of truth: system-test docs are auto-generated from registry + gaps.
- Fail-closed testing: no “best effort” assertions, no hidden fallbacks.
- LLM-friendly introspection: machine-readable manifests, standardized summaries, and logs.
- Zero legacy baggage: adopt the cleanest architecture now.

## Non-Goals (Phase 1)
- Performance benchmarks as release gates (defer to Phase 2).
- External/production providers by default (allow opt-in only).

## Crate Structure (Workspace Member)
Name: `system-tests` (top-level directory).

```
system-tests/
├─ AGENTS.md
├─ Cargo.toml
├─ README.md
├─ TEST_MATRIX.md
├─ test_registry.toml
├─ test_gaps.toml
├─ src/
│  ├─ lib.rs
│  └─ config/
│     ├─ env.rs
│     └─ mod.rs
└─ tests/
   ├─ smoke.rs
   ├─ functional.rs
   ├─ reliability.rs
   ├─ security.rs
   ├─ providers.rs
   ├─ runpack.rs
   ├─ mcp_transport.rs
   ├─ suites/
   │  ├─ smoke_*.rs
   │  ├─ functional_*.rs
   │  ├─ reliability_*.rs
   │  ├─ runpack_*.rs
   │  ├─ security_*.rs
   │  └─ providers_*.rs
   ├─ helpers/
   │  ├─ harness.rs
   │  ├─ mcp_client.rs
   │  ├─ readiness.rs
   │  ├─ artifacts.rs
   │  ├─ assertions.rs
   │  └─ config.rs
   └─ scenarios/
      ├─ smoke/
      ├─ functional/
      ├─ reliability/
      ├─ security/
      ├─ providers/
      ├─ runpack/
      └─ mcp_transport/
```

## Categories (Decision Gate Specific)
These are system-test “coverage” categories used in registry and docs.

- `smoke`: fast sanity across tool surface.
- `functional`: scenario lifecycle, branching, packets, comparators.
- `providers`: built-ins + federated MCP provider coverage.
- `mcp_transport`: stdio + loopback HTTP JSON-RPC behavior.
- `runpack`: export/verify, tamper detection, manifest correctness.
- `reliability`: idempotency, replay parity, run state stability.
- `security`: redaction, fail-closed policies, provider trust.
- `contract`: schema conformance (runtime payloads match contracts).
- `operations`: startup/stop config validation (optional in Phase 1).

## Hyperscaler Mapping (Public-Facing Signals)
- `correctness`: deterministic evaluation, schema conformance, idempotency.
- `crash_safety`: runpack verification + restart flows (future durable store).
- `concurrency`: parallel tool calls, multi-run isolation.
- `observability`: logs, summaries, runpack traceability.
- `api_contracts`: contract schemas, tool definitions.
- `operations`: transport boot + config validation.
- `security`: redaction, trust boundaries, provider validation.

## Harness and Contract Rules
1) No fail-open. No alternate paths. No “best effort” assertions.
2) No sleeps for correctness. Use readiness probes and explicit polling.
3) Use canonical types (from core/mcp) for all requests/responses.
4) Emit deterministic artifacts (canonical JSON).
5) All scenarios must be registry-listed and artifact-bearing.

## Standard Artifacts Per Test
- `runner.stdout.log`, `runner.stderr.log` (from orchestrator).
- `mcp.log` (server logs).
- `provider.log` (federated provider logs when applicable).
- `summary.json` + `summary.md` (scenario summary).
- `tool_transcript.json` (MCP request/response transcript).
- `runpack/` with `index.json` and logs (for runpack tests).

## Registry and Gaps (Governance)
### Registry (`system-tests/test_registry.toml`)
Each test entry includes:
- `name`, `category`, `priority` (P0/P1/P2)
- `hyperscaler_mapping` (list)
- `run_command` (exact)
- `files`, `description`, `artifacts`, `estimated_runtime_sec`
- `requires_serial` for shared resource tests

### Gaps (`system-tests/test_gaps.toml`)
Each gap includes:
- `id`, `title`, `category`, `priority`, `status`
- `acceptance_criteria` (explicitly measurable)
- `files_to_modify`, `dependencies`
- LLM task generation fields (optional)

## Orchestrator Scripts
### `scripts/test_runner.py`
Clone the Asset-Core flow with Decision Gate names:
- Reads registry.
+- Supports `--category`, `--priority`, `--hyperscaler`, `--quick`, `--analyze`.
- Creates `.tmp/test_run_<timestamp>` run root with per-test subdirs.
- Captures stdout/stderr, emits `manifest.json`.
- `DECISION_GATE_SYSTEM_TEST_RUN_ROOT` passed to tests.
- Optional isolated `CARGO_TARGET_DIR` when parallel runs.

### `scripts/coverage_report.py`
Generate:
- `Docs/testing/decision_gate_test_coverage.md`
- `Docs/testing/test_infrastructure_guide.md`
Same structure as Asset-Core, tuned categories/hyperscaler mapping.

### `scripts/gap_tracker.py`
Mirror Asset-Core gap tooling: list/show/close/generate-task.

## System-Test Suites (Phase 1 Battery)
### P0 (Must Pass)
- `smoke_define_start_next_status`: full MCP lifecycle.
- `runpack_export_verify_happy_path`: runpack round-trip.
- `schema_conformance_all_tools`: runtime outputs validate schemas.
- `evidence_redaction_default`: policy enforcement.
- `idempotent_trigger`: same trigger_id yields same decision.
- `provider_time_after`: deterministic time predicate.

### P1 (High Value)
- `http_transport_end_to_end`: loopback HTTP JSON-RPC.
- `federated_provider_echo`: external MCP provider integration.
- `packet_disclosure_visibility`: visibility policies asserted.
- `runpack_tamper_detection`: corrupt artifact check.

### P2 (Later)
- Performance smoke (non-gated).
- External provider integration (opt-in secrets).

## Implementation Phases
### Phase 1: Foundations
1) Add new crate with layout + AGENTS.md + README.md.
2) Add registry + gaps TOML with initial tests.
3) Implement orchestrator scripts and coverage docs generator.
4) Add helper harness for MCP server and simple client wrapper.

### Phase 2: Core Battery
1) Implement P0 tests.
2) Implement P1 tests.
3) Add scenario summaries + transcript artifacts.
4) Register tests in registry with run commands and artifacts.

### Phase 3: Docs + CI
1) Generate Docs/testing outputs from coverage_report.py.
2) Document nextest profiles (optional).
3) Wire a lightweight CI target to run smoke or selected P0 tests.

## Acceptance Criteria (System-Tests Ready)
- All P0 + P1 tests pass on Windows and Linux.
- Registry + gaps + coverage docs are in sync.
- Orchestrator produces manifests and artifact aggregation.
- README + AGENTS define strict test contract rules.
- No manual docs drift (coverage docs are generated).

## Risks and Mitigations
- Transport flakiness: use deterministic readiness probes and strict timeouts.
- External dependencies: make external providers opt-in (env var).
- Windows path differences: normalize paths in artifacts and manifests.

## Security Posture Alignment
System tests must consult `Docs/security/threat_model.md`. Any new trust boundary
introduced by the test harness must be documented or explicitly marked as test-only.

## Next Steps (Execution)
Implement Phase 1 + Phase 2 in a single cohesive pass, then generate the
coverage docs and validate test execution locally.
