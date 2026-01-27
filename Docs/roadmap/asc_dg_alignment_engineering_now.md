<!--
Docs/roadmap/asc_dg_alignment_engineering_now.md
============================================================================
Document: Decision Gate + Asset Core Alignment (Engineering Now)
Description: Near-term engineering tasks for independent DG and optional ASC overlap.
Purpose: Define the immediate technical work needed to align namespace, auth, and evidence anchors.
Dependencies:
  - Docs/guides/assetcore_interop_runbook.md
  - Docs/security/threat_model.md
  - Docs/business/open_core_strategy.md
============================================================================
-->

# Decision Gate + Asset Core Alignment (Engineering Now)

## Overview
This roadmap captures the immediate engineering work needed to keep Decision Gate
independent while enabling world-class overlap with Asset Core. The goal is to
standardize integration boundaries (namespace, auth, evidence anchors) without
introducing tight code coupling or hidden dependencies.

## Current Status (As Implemented)
All V1 engineering tasks in this roadmap are implemented and validated. System
tests and workspace unit tests were executed and pass. Remaining open questions
for this scope: none.

## Strategic Goals (World-Class Standard)
- **Independence by design**: DG runs without ASC; ASC runs without DG.
- **Coherent overlap**: When integrated, namespaces and evidence anchors are
  deterministic, auditable, and fail closed.
- **Zero trust posture**: Trust boundaries are explicit; unknown inputs deny.
- **Determinism first**: Same inputs produce identical outputs and runpacks.
- **No adapter business logic**: Protocol adapters remain thin and stateless.

## Non-Negotiable Boundaries (Why)
- **DG is a control plane**; **ASC is a world-state substrate**. Each owns its
  domain to preserve clarity, auditability, and long-term evolution.
- **No direct auth coupling**: DG does not parse ASC auth tokens or reuse ASC
  auth internals. A narrow integration layer maps ASC principals to DG scope.
- **No write-path gating in V1**: DG does not sit in the ASC write path. This
  avoids dangerous coupling and preserves ASC determinism and availability.

## Locked Decisions (With Rationale)

### D1: Namespace Authority
Decision: In ASC-integrated deployments, DG reads the ASC namespace catalog
read-only and fails closed on unknown or unauthorized namespaces. DG standalone
deployments use DG’s own registry.
Why: One authoritative catalog avoids drift and ensures deterministic scoping.

### D2: Auth/RBAC Mapping
Decision: Use a dedicated integration layer to verify ASC auth and pass a minimal
DG PrincipalContext (tenant_id, principal_id, roles, policy_class, groups).
DG does not parse ASC tokens directly.
Why: Preserves DG open-core independence and prevents brittle coupling.

### D3: Evidence Anchor Canon
Decision: All ASC-backed evidence includes anchors:
`assetcore.namespace_id`, `assetcore.commit_id`, `assetcore.world_seq`.
Optional: `assetcore.chain_hash` if available.
Why: Guarantees deterministic replay and offline verification.

### D4: Latency + Retry Posture
Decision: Evidence queries against ASC have explicit timeouts and limited retry
for transport errors only. Timeouts return `Unknown` (fail-closed).
Why: Keeps determinism and avoids masking semantic failures.

### D5: Write-Path Gating
Decision: Skip write-path gating entirely in V1. Only revisit if a standalone
design emerges that does not compromise ASC’s write path.
Why: Coupling DG into the write path risks availability and correctness.

## Immediate Engineering Tasks (Implementation-Ready)

### 1) Integration Contract (Spec-First)
- Implemented: `Docs/architecture/decision_gate_assetcore_integration_contract.md`.
- Author a DG <-> ASC integration contract document that is the canonical
  implementation reference:
  - Namespace scoping rules (explicit `namespace_id`, no defaults, fail closed).
  - Evidence anchoring requirements (`world_seq`, `commit_id`, `namespace_id`).
  - Correlation ID passthrough rules (client + server identifiers).
  - Auth/RBAC mapping expectations (ASC principals -> DG permissions).
  - Error taxonomy and retry posture.
- Update tooltips/contracts to include ASC anchor types and definitions.
- Related docs updated:
  - `Docs/guides/assetcore_interop_runbook.md`
  - `Docs/security/threat_model.md`
  - `decision-gate-core/Docs/security/threat_model.md`
- Contract/schema updates:
  - `decision-gate-contract/src/schemas.rs`
  - `Docs/generated/decision-gate/schemas/config.schema.json`

### 2) Namespace Authority Implementation
- Add a read-only ASC namespace catalog backend to DG (for integrated mode).
- Ensure all DG tool calls validate namespace existence and authorization.
- Fail closed when the catalog is unreachable or inconsistent.
- Implementation:
  - `decision-gate-mcp/src/namespace_authority.rs`
  - `decision-gate-mcp/src/config.rs`
  - `decision-gate-mcp/src/server.rs`
  - `decision-gate-mcp/src/tools.rs`
  - `Docs/configuration/decision-gate.toml.md`
- Tests:
  - `decision-gate-mcp/tests/tool_router.rs`
  - `decision-gate-mcp/tests/config_validation.rs`

### 3) Asset Core Evidence Provider Contract
- Specify the ASC evidence provider interface (inputs, outputs, anchor fields).
- Define required read-daemon queries and determinism expectations.
- Enforce strict limits (payload size, rate limits, timeouts).
- Implementation:
  - `system-tests/tests/fixtures/assetcore/providers/assetcore_read.json`
  - `system-tests/tests/fixtures/assetcore/decision-gate.toml`
  - `system-tests/tests/helpers/provider_stub.rs`
  - `system-tests/tests/providers.rs`
  - `Docs/guides/assetcore_interop_runbook.md`

### 4) Runpack Anchor Semantics
- Require ASC anchor metadata for any evidence derived from ASC.
- Verify anchors during runpack export and offline verification.
- Ensure anchors are included in manifests and verification reports.
- Implementation:
  - `decision-gate-core/src/core/evidence.rs`
  - `decision-gate-core/src/runtime/engine.rs`
  - `decision-gate-core/src/runtime/runpack.rs`
  - `decision-gate-core/src/core/runpack.rs`
- Tests:
  - `decision-gate-core/tests/runpack.rs`
  - `decision-gate-core/tests/evidence_errors.rs`

### 5) Observability and Correlation IDs
- Preserve client and server correlation IDs end-to-end (DG -> ASC).
- Ensure audit logs and runpacks include correlation identifiers.
- Implementation:
  - `decision-gate-mcp/src/evidence.rs`
  - `decision-gate-mcp/src/namespace_authority.rs`
  - `decision-gate-mcp/src/evidence/tests.rs`

### 6) Full Verification Test Suite (Maximum Coverage)
The test suite must be exhaustive (not minimal). Required categories:
- **Contract tests**: schema/tooltips/anchor definitions are consistent.
- **Namespace isolation**: cross-namespace queries fail closed.
- **Auth mapping**: ASC role/policy_class -> DG permissions matrix.
- **Evidence anchors**: every ASC-backed evidence result includes required anchors.
- **Determinism**: identical ASC state -> identical DG outcomes and runpacks.
- **Failure modes**: timeouts, read-daemon errors, invalid anchors -> `Unknown`.
- **Correlation IDs**: client + server IDs preserved across calls.
- **Replay verification**: runpack verification succeeds across environments.
- **Load/concurrency**: high parallel evidence queries remain deterministic.
- **Security fuzzing**: malformed inputs, oversized payloads, anchor injection.
- **Multi-transport parity**: MCP/CLI/HTTP produce identical outcomes.
- Implemented coverage (new/expanded):
  - `decision-gate-core/tests/runpack.rs`
  - `decision-gate-core/tests/evidence_errors.rs`
  - `decision-gate-mcp/tests/tool_router.rs`
  - `decision-gate-mcp/tests/config_validation.rs`
  - `decision-gate-mcp/src/evidence/tests.rs`
  - `system-tests/tests/providers.rs`
  - `system-tests/tests/assetcore_integration.rs`
  - `system-tests/tests/auth_matrix.rs`
  - `system-tests/tests/determinism.rs`
  - `system-tests/tests/transport_parity.rs`
  - `system-tests/tests/anchor_fuzz.rs`
  - `system-tests/tests/helpers/namespace_authority_stub.rs`
  - `system-tests/tests/helpers/provider_stub.rs`
- Coverage gaps tracked for follow-up:
  - `system-tests/test_gaps.toml` (all ASC/DG alignment gaps closed as of this pass)

## Deferred (Explicitly Out of Scope for V1)
- **Write-path gating**: do not integrate DG into ASC write precheck in V1.
  TODO: Revisit only if a safe, decoupled design is proposed.

## Resolved Decisions (Former Open Questions)
- **Integration contract location**: DG repo is canonical. ASC docs may reference
  it, but the open-surface contract must be visible to all adopters.
- **Provider ID strategy**: Canonical `assetcore` provider ID with optional
  scoped aliases (`assetcore:<cluster>` or `assetcore:<tenant>`) for multi-scope
  deployments.
- **`assetcore.chain_hash` requirement**: Optional in v1. Required fields are
  `assetcore.namespace_id`, `assetcore.commit_id`, `assetcore.world_seq`.
- **Auth mapping matrix**: Defined in
  `Docs/architecture/decision_gate_assetcore_integration_contract.md` and
  validated by `system-tests/tests/auth_matrix.rs` using the auth proxy.

## Remaining Open Questions
- None for the V1 engineering scope in this document. Future updates should be
  added here explicitly if new coupling or policy constraints emerge.

## Validation (Executed)
- `cargo test -p system-tests --features system-tests`
- `cargo test --workspace --exclude system-tests --exclude enterprise-system-tests`
