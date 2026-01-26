<!--
Docs/roadmap/open_items.md
============================================================================
Document: Decision Gate Open Items
Description: Open roadmap items and release readiness gaps.
Purpose: Track remaining work after MCP core, trust lanes, and strict validation.
Dependencies:
  - Docs/security/threat_model.md
  - Docs/roadmap/trust_lanes_registry_plan.md
  - Docs/architecture/comparator_validation_architecture.md
============================================================================
-->

# Decision Gate Open Items

## Overview
This document tracks remaining release-readiness gaps after MCP core, trust
lanes, schema registry, strict validation, runpack tooling, and system tests
are in place. Priority legend: P0 = release blocker, P1 = production readiness,
P2 = docs/guidance.

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

### P0) Release Blockers
**Status**: None currently open. Security audit is clean and transport hardening
is complete.

### 1) [P1] Policy Engine Integration
**What**: Replace `PermitAll` / `DenyAll` with real policy adapters.
**Why**: Dispatch authorization is critical to disclosure control.
**Status**: Open.
**Where**:
- `decision-gate-core/src/interfaces/mod.rs` (PolicyDecider trait)
- `decision-gate-mcp/src/tools.rs` (DispatchPolicy implementation)
**How**: Add policy backends and include their schemas/config in the contract
bundle.

### 2) [P1] Dev-Permissive Mode + Default Namespace Policy
**What**: Add an explicit dev-permissive toggle (asserted evidence allowed)
and define default namespace behavior for non-Asset-Core deployments.
**Why**: Trust lanes and namespace isolation need an explicit opt-in for
single-tenant/dev mode with warnings.
**Status**: Open.
**Where**:
- `decision-gate-mcp/src/config.rs` (trust config surface)
- `decision-gate-mcp/src/server.rs` (startup warnings)
**How**: Add config flags, enforce defaults, and emit warnings when enabled.

### 3) [P1] Schema Registry RBAC/ACL + Audit Events
**What**: Enforce per-tenant/role ACLs for schema registry operations and
emit registry-specific audit events.
**Why**: Registry writes are a trust boundary and need explicit access control.
**Status**: Open/Partial (tool allowlists exist, but no per-tenant ACL).
**Where**:
- `decision-gate-mcp/src/tools.rs` (schemas_register/list/get)
- `decision-gate-mcp/src/auth.rs` (auth policy)
- `decision-gate-mcp/src/audit.rs` (audit sink)

### 4) [P1] Precheck Hash-Only Audit Logging
**What**: Emit hash-only audit records for precheck requests/responses by
default (no raw payload).
**Why**: Precheck is read-only but still handles asserted data; audit must be
privacy-preserving by default.
**Status**: Open.
**Where**:
- `decision-gate-mcp/src/audit.rs`
- `decision-gate-mcp/src/tools.rs` (precheck handler)

### 5) [P1] Durable Runpack Storage Beyond Filesystem
**What**: Add production-grade `ArtifactSink` and `ArtifactReader` backends
for object storage or WORM storage.
**Why**: Filesystem runpacks are implemented, but cloud-native durability
requires blob store adapters.
**Status**: Partial (file-backed sink/reader implemented).
**Where**:
- `decision-gate-mcp/src/runpack.rs` (file-backed sink/reader)
- `decision-gate-core/src/interfaces/mod.rs` (ArtifactSink/Reader traits)
**How**: Implement object store adapters with strict path validation and
typed errors.

### 6) [P2] Scenario Examples for Hold/Unknown/Branch Outcomes
**What**: Add canonical scenarios that demonstrate unknown outcomes, hold
decisions, and branch routing for true/false/unknown.
**Why**: Scenario authors need precise, audited examples that show how
tri-state outcomes affect routing and hold behavior.
**Status**: Partial (only happy-path examples today).
**Where**: `Docs/generated/decision-gate/examples/`

### 7) [P2] Run Lifecycle Guide
**What**: Create a single guide that maps tool calls to run state transitions
and runpack artifacts.
**Why**: Integrators need a mental model that ties `scenario_define` →
`scenario_start` → `scenario_next`/`scenario_trigger` → `runpack_export` to
state mutations and artifacts.
**Status**: Missing.
**Where**: `Docs/guides/run_lifecycle.md` (new).

### 8) [P2] Agent Progress vs Plan State Guidance
**What**: Clarify that Decision Gate evaluates evidence and run state, while
agent planning is external. Progress signals should be modeled as evidence or
submissions.
**Why**: Keeps Decision Gate deterministic and avoids embedding agent logic.
**Status**: Open (guidance).

### 9) [P2] Runpack Verification with Evidence Replay (Optional)
**What**: Optional CLI/MCP flow to re-query evidence and compare against
runpack anchors/hashes during verification.
**Why**: Provides an additional audit mode when evidence sources are stable.
**Status**: Open (not implemented).

## Completed Items (Reference)

### A) Canonical Contract and Generated Docs Bundle
**Status**: Implemented. Contract artifacts are generated under
`Docs/generated/decision-gate/`.

### B) System Tests Crate (End-to-End)
**Status**: Implemented. System-tests crate, registry, scripts, and coverage
docs are in place. See `system-tests/` and `Docs/testing/`.

### C) Authoring Formats (ScenarioSpec and Requirements)
**Status**: Implemented. JSON is canonical; RON is accepted as input and
normalized to canonical JSON (RFC 8785). Examples are generated in
`Docs/generated/decision-gate/examples/`.

### D) MCP Tool Surface: Docs, Schemas, and Enums
**Status**: Implemented. Tool schemas, examples, and tooltips are generated
from the contract bundle and aligned with runtime behavior.

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

### H) Inbound AuthN/AuthZ + Transport Hardening
**Status**: Implemented. MCP tool calls enforce authn/authz with local-only
defaults, bearer token or mTLS subject allowlists, per-tool authorization, rate
limits, TLS/mTLS, and audit logging.

### I) Strict Comparator Validation (Default-On)
**Status**: Implemented. See
`Docs/architecture/comparator_validation_architecture.md`.

### J) Trust Lanes, Schema Registry, Discovery Tools, Precheck
**Status**: Implemented. Trust lanes, registry storage, discovery tools, and
precheck are present; remaining items are policy/audit hardening and docs.

## Notes on Structural Readiness
Evidence, storage, and dispatch interfaces already exist in
`decision-gate-core/src/interfaces/mod.rs`, enabling policy enforcement and
durable backends without core rewrites.
