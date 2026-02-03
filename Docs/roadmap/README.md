<!--
Docs/roadmap/README.md
============================================================================
Document: Decision Gate Roadmap (Public)
Description: Public roadmap and release-readiness gaps.
Purpose: Track remaining work after MCP core, trust lanes, and strict validation.
Dependencies:
  - Docs/security/threat_model.md
  - Docs/architecture/comparator_validation_architecture.md
  - Docs/roadmap/foundational_correctness_roadmap.md
============================================================================
-->

# Decision Gate Roadmap

## Status Snapshot (as of 2026-02-03)

Decision Gate is preparing for OSS launch. This roadmap captures the
remaining launch blockers, production-readiness work, and documentation gaps.
We are not accepting external PRs at this time; feedback via issues and
discussion is welcome.

This document is the public summary. The authoritative correctness gate
checklist lives in `Docs/roadmap/foundational_correctness_roadmap.md`.

## Stability Tiers

Stable: Deterministic, audited, and ready for production use.
Beta: Feature-complete but still receiving hardening or scale validation.
Experimental: Works, but behavior or contracts may change; feedback requested.

External provider integrations via MCP are Experimental while hardening and
real-world feedback cycles continue.

## Roadmap Semantics

P0: Launch blockers. OSS launch does not ship until all P0 items are complete.
P1: Production readiness. Required for production-grade guidance and support.
P2: Docs, examples, and optional flows.

## Current Defaults (Authoritative Until Revised)

1. Canonical Contract Location
   `decision-gate-contract` owns contract generation and derived docs artifacts.
2. Generated Docs and Artifacts
   Contract outputs are committed under `Docs/generated/decision-gate/`.
3. Authoring Formats
   Canonical ScenarioSpec format is JSON. RON is accepted and normalized to
   canonical JSON. YAML is not supported unless added later.
4. Cross-OS Determinism
   Linux and Windows are required targets. macOS is best-effort.

## P0) Launch Blockers (World-Class Bar)

Status: Launch-blocking until all gates pass.

- Foundational correctness remaining gates summary
  Reference: `Docs/roadmap/foundational_correctness_roadmap.md`.
  Remaining gaps are primarily around adversarial depth and cross-surface
  confidence: legacy runpack compatibility vectors, defined capacity thresholds
  and performance/scaling gates, Windows Quick Start validation and
  reproducible build guidance, and live-mode agentic harness parity
  (report-only integration reality checks). These are security- and
  trust-critical: without them, determinism, fail-closed guarantees, and
  auditability regress under adversarial or high-load conditions.
- Metamorphic determinism coverage
  Status: Partial (2026-02-02). Canonical evidence ordering and concurrent
  runpack hashing are covered. Provider-order shuffle and evidence-arrival
  reorder cases are still missing.
- Fuzzing expansion
  Status: Completed (2026-02-02). ScenarioSpec, Evidence payloads, JSONPath,
  and comparator edge cases.
- Chaos provider matrix
  Status: Completed (2026-02-02). TLS oddities, redirect loops, slow-loris,
  truncation.
- Runpack backward compatibility verification (legacy vectors)
  Status: Open.
- SQLite durability tests
  Status: Completed (2026-02-02). Crash/partial write/rollback recovery.
- Log leakage scanning
  Status: Completed (2026-02-02). Secret exposure across error paths/panics.
- Performance/scaling targets with at least one gated benchmark
  Status: Open.
- Agentic flow harness live-mode + cross-OS CI parity for deterministic runs
  Status: Open. Reference:
  `Docs/roadmap/decision_gate_agentic_flow_harness_plan.md`.
- CLI world-class readiness gaps
  Status: Completed (2026-02-02). Any remaining partial items stay tracked in
  `Docs/roadmap/decision_gate_cli_world_class_readiness.md`.

## P1) Production Readiness

- Reproducible build guidance and version stamping for CLI.
- Quick Start validation on Linux and Windows.
- Contract/tooling regeneration checks (schemas, tooltips, examples).
- Capacity/limits documentation (max payloads, runpack sizes).
- Governance/OSS policy docs (CLA, trademark, contribution model).

## P2) Docs, Examples, and Optional Flows

1. Scenario examples for hold/unknown/branch outcomes
   What: Add canonical scenarios that demonstrate unknown outcomes, hold
   decisions, and branch routing for true/false/unknown.
   Why: Scenario authors need precise, audited examples that show how tri-state
   outcomes affect routing and hold behavior.
   Status: Partial (only happy-path examples today).
   Where: `Docs/generated/decision-gate/examples/`.
2. Agent progress vs plan state guidance
   What: Clarify that Decision Gate evaluates evidence and run state, while
   agent planning is external. Progress signals should be modeled as evidence
   or submissions.
   Why: Keeps Decision Gate deterministic and avoids embedding agent logic.
   Status: Open (guidance).
3. Runpack verification with evidence replay (optional)
   What: Optional CLI/MCP flow to re-query evidence and compare against
   runpack anchors/hashes during verification.
   Why: Provides an additional audit mode when evidence sources are stable.
   Status: Open (not implemented).
4. Public integration docs hygiene (AssetCore references)
   What: Remove placeholders or replace with OSS-safe, generic integration
   examples where AssetCore is referenced.
   Why: Public OSS docs should not depend on private repo content.
   Status: Open.
   Where: `Docs/integrations/assetcore/examples.md`.
   Where: `Docs/integrations/assetcore/deployment.md`.
   Where: `Docs/integrations/assetcore/README.md`.
5. AssetCore example and deployment placeholders (launch TODOs)
   What: Replace explicit TODO placeholders for AssetCore examples and
   deployment recipes with validated, OSS-safe guidance, or remove the
   sections entirely if they cannot be made public.
   Why: These placeholders are explicit pre-launch gaps and create a
   documentation trust risk. They also imply dependencies on private content,
   which violates the OSS boundary and can mislead adopters.
   Status: Open (tracked in this roadmap).
   Where: `Docs/integrations/assetcore/examples.md` (world-class placeholder).
   Where: `Docs/integrations/assetcore/deployment.md` (deployment TODO).
6. Provider contract bulk export (deferred)
   What: A discovery endpoint or CLI command that exports all provider
   contracts or compiled check schemas in a single request.
   Why: Enables offline indexing, UI catalogs, and build-time caching without
   per-provider calls.
   Security or risk: High data-volume disclosure surface. Without explicit
   opt-in, pagination, and strict size limits, it can leak sensitive metadata,
   create denial-of-service vectors, and blow out LLM context windows. Must be
   gated by authz and allow or deny lists and protected by response byte caps.
   Status: Open (explicitly deferred; no `provider_contracts_export` tool or
   CLI command implemented).

## Delivered (Reference)

A. Canonical contract and generated docs bundle
Status: Implemented. Contract artifacts are generated under
`Docs/generated/decision-gate/`.

B. System tests crate (end-to-end)
Status: Implemented. System-tests crate, registry, scripts, and coverage docs
are in place. See `system-tests/` and `Docs/testing/`.

C. Authoring formats (ScenarioSpec and Requirements)
Status: Implemented. JSON is canonical. RON is accepted and normalized to
canonical JSON (RFC 8785). Examples are generated in
`Docs/generated/decision-gate/examples/`.

D. MCP tool surface: docs, schemas, and enums
Status: Implemented. Tool schemas, examples, and tooltips are generated from
the contract bundle and aligned with runtime behavior.

E. Provider capability metadata and validation
Status: Implemented. Capability registry validation is enforced for
ScenarioSpec and evidence queries. Provider docs are generated from the same
contract metadata.

F. Durable run state store
Status: Implemented. SQLite WAL-backed store with snapshots, integrity checks,
typed errors, and retention is available and configurable.

G. Timeout policy enforcement and documentation
Status: Implemented. Timeout policies are enforced by tick triggers and
documented in tooltips and generated contract docs.

H. Inbound authn and authz plus transport hardening
Status: Implemented. MCP tool calls enforce authn/authz with local-only
defaults, bearer token or mTLS subject allowlists, per-tool authorization,
rate limits, TLS or mTLS, and audit logging.

I. Strict comparator validation (default-on)
Status: Implemented. See
`Docs/architecture/comparator_validation_architecture.md`.

J. Trust lanes, schema registry, discovery tools, precheck
Status: Implemented. Trust lanes, registry storage, discovery tools, and
precheck are present; remaining items are audit hardening and docs.

K. Policy engine integration
Status: Implemented. Swappable policy engine selection with deterministic
static rules, deny/permit/error effects, and contract schema support.
Refs:
`decision-gate-mcp/src/policy.rs`.
`decision-gate-config/src/config.rs`.
`decision-gate-contract/src/schemas.rs`.
`decision-gate-contract/src/tooltips.rs`.

L. Dev-permissive mode plus default namespace policy
Status: Implemented. `dev.permissive` provides explicit dev-only trust
relaxation with warnings and TTL checks. Default namespace id usage is
allowlisted via `namespace.default_tenants` and is never implicitly enabled.
Dev-permissive is disallowed when `namespace.authority.mode = "assetcore_http"`.

M. Schema registry RBAC or ACL plus audit events
Status: Implemented. Registry access is enforced by `schema_registry.acl`
(builtin or custom), backed by `server.auth.principals` role mappings. Registry
allow or deny decisions emit audit events, and signing metadata can be required
for schema writes.

N. Durable runpack storage beyond filesystem
Status: Implemented. OSS object-store sink/reader and configuration are in
place, with strict key validation and size limits. Enterprise S3 adapters in
the private monorepo support optional Object Lock (WORM) retention and legal
hold.

## Notes on Structural Readiness

Evidence, storage, and dispatch interfaces already exist in
`decision-gate-core/src/interfaces/mod.rs`, enabling policy enforcement and
durable backends without core rewrites.
