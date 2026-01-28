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
**Status**: Foundational correctness gates are active and launch-blocking.
See `Docs/roadmap/foundational_correctness_roadmap.md` for the complete
gate checklist and cross-OS determinism requirements.

### 1) [P2] Scenario Examples for Hold/Unknown/Branch Outcomes
**What**: Add canonical scenarios that demonstrate unknown outcomes, hold
decisions, and branch routing for true/false/unknown.
**Why**: Scenario authors need precise, audited examples that show how
tri-state outcomes affect routing and hold behavior.
**Status**: Partial (only happy-path examples today).
**Where**: `Docs/generated/decision-gate/examples/`

### 2) [P2] Agent Progress vs Plan State Guidance
**What**: Clarify that Decision Gate evaluates evidence and run state, while
agent planning is external. Progress signals should be modeled as evidence or
submissions.
**Why**: Keeps Decision Gate deterministic and avoids embedding agent logic.
**Status**: Open (guidance).

### 3) [P2] Runpack Verification with Evidence Replay (Optional)
**What**: Optional CLI/MCP flow to re-query evidence and compare against
runpack anchors/hashes during verification.
**Why**: Provides an additional audit mode when evidence sources are stable.
**Status**: Open (not implemented).

### 4) [P2] ASC Integration Collateral Placeholders (Lead Example + Recipes)
**What**: Replace placeholders with a real lead example and validated
deployment guidance for DG+ASC integration.
**Why**: Integration docs are visible and referenced as canonical; placeholders
create ambiguity for adopters and could be mistaken as production-ready.
**Status**: Open (explicit TODOs remain).
**Where**:
- `Docs/integrations/assetcore/examples.md` (lead example narrative)
- `Docs/integrations/assetcore/deployment.md` (validated deployment patterns)
- `Docs/integrations/assetcore/README.md` (lead example link/summary)
**Notes**:
- The lead example must align with the integration contract (namespace authority,
  auth mapping, evidence anchors).
- Deployment notes should stay conceptual until hardened recipes exist.

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
precheck are present; remaining items are audit hardening and docs.

### K) Policy Engine Integration
**Status**: Implemented. Swappable policy engine selection with deterministic
static rules, deny/permit/error effects, and contract schema support.
**Refs**:
- `decision-gate-mcp/src/policy.rs`
- `decision-gate-mcp/src/config.rs`
- `decision-gate-contract/src/schemas.rs`
- `decision-gate-contract/src/tooltips.rs`

### L) Dev-Permissive Mode + Default Namespace Policy
**Status**: Implemented. `dev.permissive` provides explicit dev-only trust
relaxation with warnings and TTL checks. Default namespace id usage is allowlisted
via `namespace.default_tenants` and is never implicitly enabled. Dev-permissive
is disallowed when `namespace.authority.mode = "assetcore_http"`.

### M) Schema Registry RBAC/ACL + Audit Events
**Status**: Implemented. Registry access is enforced by
`schema_registry.acl` (builtin or custom), backed by `server.auth.principals`
role mappings. Registry allow/deny decisions emit audit events, and signing
metadata can be required for schema writes.

### N) Durable Runpack Storage Beyond Filesystem
**Status**: Implemented. OSS object-store sink/reader and configuration are in
place, with strict key validation and size limits. Enterprise S3 adapters
support optional Object Lock (WORM) retention and legal hold.

## Notes on Structural Readiness
Evidence, storage, and dispatch interfaces already exist in
`decision-gate-core/src/interfaces/mod.rs`, enabling policy enforcement and
durable backends without core rewrites.
