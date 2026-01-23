<!--
Docs/roadmap/open_items.md
============================================================================
Document: Decision Gate Open Items
Description: Open roadmap items and release readiness gaps.
Purpose: Track remaining work after MCP core implementation.
Dependencies:
  - Docs/roadmap/decision_gate_mcp_roadmap.md
  - Docs/security/threat_model.md
============================================================================
-->

# Decision Gate Open Items

## Overview
This document tracks remaining roadmap items now that the MCP foundation,
provider federation, and CLI scaffolding are complete. The focus is on
release-readiness gaps, invariance alignment, and system-level validation.
Priority legend: P0 = release blocker, P1 = production readiness, P2 = docs/guidance.

## Decision Summary (Current Defaults)
These defaults anchor the roadmap and should be treated as authoritative until
explicitly revised.

1. **Canonical Contract Location**
   - A dedicated crate (`decision-gate-contract`) owns contract generation and
     all derived docs artifacts.
2. **Generated Docs and Artifacts**
   - Contract outputs are committed in this repo under
     `Docs/generated/decision-gate/`.
3. **Authoring Formats**
   - Canonical ScenarioSpec format is JSON.
   - RON is allowed as an authoring input and converted to JSON.
   - YAML is not supported unless explicitly added later.

## Open Items (Current)

### 1) [P0] Inbound AuthN/AuthZ for MCP Tool Calls
**What**: Add explicit auth interfaces for MCP tool calls (token/mTLS and
per-tool authorization) with audit logging.
**Why**: The current local-only posture is not production-safe, and tool calls
are the highest-risk boundary.
**Status**: Open.
**How**: Introduce a `ToolAuthz` trait in `decision-gate-mcp`, enforce it in
`server.rs`/`tools.rs`, and include a default local-only policy with warnings.

### 2) [P1] Durable Runpack Storage
**What**: Add production-grade `ArtifactSink` and `ArtifactReader` backends.
**Why**: Runpacks are the audit trail; durable storage is required for real use.
**Status**: Open.
**How**: Implement object store or secured filesystem adapters with strict path
validation and explicit error typing.

### 3) [P0] Transport Hardening and Operational Telemetry
**What**: Add rate limiting, structured error responses, TLS/mTLS, and audit logs.
**Why**: This is required for hyperscaler/DoD-grade deployments.
**Status**: Open.
**How**: Harden JSON-RPC handlers and introduce structured audit logging with
redaction policies for evidence output.

### 4) [P1] Policy Engine Integration
**What**: Replace `PermitAll` with real policy adapters.
**Why**: Dispatch authorization is critical to disclosure control.
**Status**: Open.
**How**: Add policy backends and include their schemas in the contract bundle.

### 5) [P2] Agent Progress vs Plan State
**What**: Clarify that Decision Gate evaluates evidence and run state, while
agent planning is external. Progress signals should be modeled as evidence or
submissions.
**Why**: This keeps Decision Gate deterministic and avoids embedding agent logic.
**Status**: Open (guidance).
**How**: Provide a default pattern: agents emit progress as `scenario_submit`
payloads or evidence predicates. If plan artifacts are desired, store them as
explicit packet payloads or submissions, not core run state.

### 6) [P2] Scenario Examples for Hold/Unknown/Branch Outcomes
**What**: Add canonical scenarios that demonstrate unknown outcomes, hold
decisions, and branch routing for true/false/unknown.
**Why**: Scenario authors need precise, audited examples that show how tri-state
outcomes affect routing and hold behavior.
**Status**: Partial (only happy-path examples today).
**How**: Extend `decision-gate-contract` examples to include a branch scenario
and a hold/unknown scenario, emit them into
`Docs/generated/decision-gate/examples/`, and reference them in guides.

### 7) [P2] Run Lifecycle Guide
**What**: Create a single guide that maps tool calls to run state transitions
and runpack artifacts.
**Why**: Integrators need a mental model that ties `scenario_define` →
`scenario_start` → `scenario_next`/`scenario_trigger` → `runpack_export` to state
mutations and artifacts.
**Status**: Missing.
**How**: Add a `Docs/guides/run_lifecycle.md` with a step-by-step timeline,
inputs/outputs, and references to tooling examples and runpack artifacts.

### 8) [P0] Security Findings (Mirrored from Docs/security/audit.md)
**What**: Address open security findings documented in the audit log.
**Why**: These are release-readiness blockers and should be tracked with the
same rigor as other open items.
**Status**: Closed.
**How**: Track each item to closure with tests that assert fail-closed behavior.
Current open findings: None.

## Completed Items (Reference)

### A) Canonical Contract and Generated Docs Bundle
**Status**: Implemented. Contract artifacts are generated under
`Docs/generated/decision-gate/`. Web sync script is handled in an external repo.

### B) System Tests Crate (End-to-End)
**Status**: Implemented. System-tests crate, registry/gaps, scripts, and coverage
docs are in place with P0/P1 coverage. See `system-tests/` and `Docs/testing/`.

### C) Authoring Formats (ScenarioSpec and Requirements)
**Status**: Implemented. JSON is canonical; RON is accepted as input and
normalized to canonical JSON (RFC 8785). Examples are generated in
`Docs/generated/decision-gate/examples/`.

### D) MCP Tool Surface: Docs, Schemas, and Enums
**Status**: Implemented. Tool schemas, examples, and tooltips are generated from
the contract bundle and aligned with runtime behavior.

### E) Provider Capability Metadata and Validation
**Status**: Implemented. Capability registry validation is enforced for
ScenarioSpec and evidence queries. Provider docs are generated from the same
contract metadata.

### F) Durable Run State Store
**Status**: Implemented. SQLite WAL-backed store with snapshots, integrity
checks, typed errors, and retention is available and configurable.

### G) Timeout Policy Enforcement and Documentation
**Status**: Implemented. Timeout policies are enforced by tick triggers and
documented in tooltips and generated contract docs.

## Notes on Structural Readiness
Evidence, storage, and dispatch interfaces already exist in
`decision-gate-core/src/interfaces/mod.rs`, enabling durable backends and
policy enforcement without core rewrites. Remaining gaps are inbound MCP
auth, durable runpack storage, transport hardening/telemetry, policy engine
integration, scenario examples, and run lifecycle guidance.
