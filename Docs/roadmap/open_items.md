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
  - Docs/roadmap/foundational_correctness_roadmap.md
============================================================================
-->

# Decision Gate Open Items

## Overview
This document tracks remaining release-readiness gaps after MCP core, trust
lanes, schema registry, strict validation, runpack tooling, and system tests
are in place. Priority legend: P0 = launch blocker, P1 = production readiness,
P2 = docs/guidance. The authoritative gate checklist lives in
`Docs/roadmap/foundational_correctness_roadmap.md`.

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
4. **Cross-OS Determinism**
   - Linux + Windows are required targets; macOS is best-effort.

## Open Items (Current)

### P0) Launch Blockers (World-Class Bar)
**Status**: Launch-blocking until all gates pass.

- Cross-OS determinism CI for golden runpacks (Linux + Windows byte-for-byte).
- Metamorphic determinism suite wired to CI with concurrency coverage.
- Fuzzing expansion across ScenarioSpec, Evidence payloads, JSONPath, and
  comparator edge cases.
- Chaos provider matrix (TLS oddities, redirect loops, slow-loris, truncation).
- Runpack backward compatibility verification (legacy vectors).
- SQLite durability tests (crash/partial write/rollback recovery).
- Log leakage scanning for secret exposure across error paths/panics.
- Performance/scaling targets with at least one gated benchmark.
- Agentic flow harness with canonical scenarios + replay verification.

### P1) Production Readiness
- Reproducible build guidance + version stamping for CLI.
- Quick Start validation on Linux + Windows.
- Contract/tooling regeneration checks (schemas, tooltips, examples).
- Capacity/limits documentation (max payloads, runpack sizes).
- Governance/OSS policy docs (CLA, trademark, contribution model).

### P2) Docs, Examples, and Optional Flows

1) Scenario Examples for Hold/Unknown/Branch Outcomes
**What**: Add canonical scenarios that demonstrate unknown outcomes, hold
Decisions, and branch routing for true/false/unknown.
**Why**: Scenario authors need precise, audited examples that show how
tri-state outcomes affect routing and hold behavior.
**Status**: Partial (only happy-path examples today).
**Where**: `Docs/generated/decision-gate/examples/`

2) Agent Progress vs Plan State Guidance
**What**: Clarify that Decision Gate evaluates evidence and run state, while
agent planning is external. Progress signals should be modeled as evidence or
submissions.
**Why**: Keeps Decision Gate deterministic and avoids embedding agent logic.
**Status**: Open (guidance).

3) Runpack Verification with Evidence Replay (Optional)
**What**: Optional CLI/MCP flow to re-query evidence and compare against
runpack anchors/hashes during verification.
**Why**: Provides an additional audit mode when evidence sources are stable.
**Status**: Open (not implemented).

4) Public Integration Docs Hygiene (AssetCore References)
**What**: Remove placeholders or replace with OSS-safe, generic integration
examples where AssetCore is referenced.
**Why**: Public OSS docs should not depend on private repo content.
**Status**: Open.
**Where**:
- `Docs/integrations/assetcore/examples.md`
- `Docs/integrations/assetcore/deployment.md`
- `Docs/integrations/assetcore/README.md`

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
