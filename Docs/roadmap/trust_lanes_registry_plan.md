<!--
Docs/roadmap/trust_lanes_registry_plan.md
============================================================================
Document: Decision Gate Trust Lanes + Schema Registry Plan
Description: Final roadmap for data shape registry, trust lanes, namespaces,
             discovery tools, and precheck.
Purpose: Single source of truth for the next implementation phase.
Dependencies:
  - Docs/business/open_core_strategy.md
  - Docs/security/threat_model.md
  - Docs/roadmap/open_items.md
  - Asset Core namespace architecture (external repo)
============================================================================
-->

# Decision Gate Trust Lanes + Schema Registry Plan

This document is the final plan for the next implementation phase. It defines
features, scope, trust model, and an ordered build plan for registry, discovery
tools, namespaces, and precheck. This is the spec we should hand to an LLM for
implementation.

## Goals

- Support two evidence lanes: provider-pulled (verified) and agent-pushed
  (asserted), with strict monotonic trust enforcement.
- Add a runtime schema registry for data shapes, scoped by tenant + namespace.
- Add discovery tools so LLMs can interrogate providers, schemas, and scenarios.
- Add a precheck tool that evaluates provided data without mutating run state.
- Align namespaces with Asset Core (explicit namespace routing, isolation, and
  dev-permissive vs secure behavior modes).

## Non-Goals

- Do not replace or remove provider-pulled evidence; it remains the ultimate
  trust lane.
- Do not change existing scenario evaluation semantics unless explicitly
  required by trust-lane policy.
- Do not add UI or dashboard work in this phase.

## Terminology (Authoritative)

- Tenant: security principal / ownership boundary (org, team, or agent).
- Namespace: logical partition within a tenant for isolation (world/scope).
- Trust lane:
  - Verified: provider-pulled evidence (MCP providers).
  - Asserted: agent-pushed evidence or precheck inputs.
- Monotonic strictness: global policy <= scenario policy <= gate/predicate
  policy (strictest wins).

## Design Principles

- Zero trust default: strict, verified lane required unless explicitly relaxed.
- Explicit opt-in for untrusted mode (dev-permissive) with warnings.
- Read-only precheck that never mutates run state.
- Long-lived, versioned schemas with tenant+namespace isolation.
- Open core parity: all trust lanes, tools, and registry features are open.

## Feature Set

### 1) Trust Lanes + Policy Lattice

- Global policy (config): allowed evidence lanes, default strict.
- Scenario policy: default trust lane required for all gates.
- Gate/predicate policy: per-gate overrides to require verified lane.
- Monotonic enforcement: stricter requirement always wins.
- Dev-permissive mode (untrusted mode): global toggle that allows asserted lane
  without requiring per-scenario overrides. Must be opt-in and noisy.

Policy examples:
- Global: verified only (default).
- Scenario: allow asserted (explicit opt-in).
- Gate: verified only (tighten inside a relaxed scenario).

Outcome behavior:
- Evidence that does not meet required lane yields Unknown for the predicate and
  holds the run (fail-closed).

### 2) Namespaces (Explicit Routing)

- Introduce namespace_id as a first-class identifier across:
  - ScenarioSpec
  - RunConfig
  - EvidenceContext
  - MCP tool inputs that operate on scenarios/runs
- Default namespace permitted only in dev-permissive/single-tenant mode.
- Namespace isolation for:
  - Schema registry
  - Scenario registry
  - Run state
  - Provider allowlists (optional future enhancement)

Alignment with Asset Core:
- Mirror explicit namespace routing.
- Support secure vs dev-permissive behavior modes.

### 3) Schema Registry (Data Shapes)

- Tenant+namespace scoped registry for data shapes.
- Long-lived, versioned schemas (immutable once registered).
- Each schema includes:
  - schema_id
  - version
  - JSON Schema payload
  - metadata (description, owner, created_at)
  - optional allowed comparators
- Size and path limits enforced (similar to provider capability contracts).
- RBAC/ACL checks for register, update (new version), list, get.
- Quotas per tenant/namespace (open core defaults + optional enterprise tuning).

### 4) Discovery Tools

Add MCP tools (read-only):

- providers/list
  - Returns provider_id, transport, predicates summary (optional), trust policy.
- schemas/list
  - Returns schemas with pagination and filters (tenant/namespace required).
- schemas/get
  - Returns schema details by schema_id + version.
- scenarios/list
  - Returns registered scenarios (optionally status per run).

Notes:
- Results must be RBAC-scoped and namespace-scoped.
- Provide pagination tokens for large registries.

### 5) Precheck Tool

Add MCP tool: precheck

Input:
- tenant_id
- namespace_id
- scenario_id or ScenarioSpec
- data_payload (asserted evidence)
- data_shape (schema_id + version)
- optional overrides (e.g., gate subset)

Behavior:
- Validates payload against schema.
- Runs comparator logic with asserted data (no provider calls).
- Produces a predicted gate evaluation and decision outcome.
- Does not mutate run state.
- Emits audit log event with hashed request/response (no payload by default).

### 6) Agent-Pushed Evidence (Optional Tooling)

- Consider separate evidence_submit tool (distinct from scenario_submit) for
  asserted evidence intended for evaluation.
- Alternatively, restrict agent-pushed data usage to precheck only in this phase
  and expand later.

Decision for this phase: precheck only (no run-state mutation).

## Open Core vs Enterprise Boundary

Open Core (must be open):
- Trust lanes + policy lattice
- Schema registry (basic)
- Discovery tools
- Precheck
- Namespace routing + isolation

Enterprise candidates (optional):
- Advanced multi-tenant quotas and analytics
- SSO/SAML/OIDC policy enforcement
- Large-scale registry storage + search
- Managed audit log retention + WORM storage

## Security and Audit

- Default strict mode (verified only).
- Dev-permissive mode requires explicit config and emits warnings.
- Precheck audit logs hash-only by default.
- All registry writes are audited (who, what, when).

## Implementation Plan (Phased)

Phase 0: Specs + Contracts
- Define TrustLane enum and policy lattice in contract and core types.
- Extend EvidenceContext with namespace_id.
- Add schema registry contract types.

Phase 1: Namespace Plumbing
- Add namespace_id to ScenarioSpec, RunConfig, and tool inputs.
- Implement namespace-scoped registries (scenario + schema).
- Add dev-permissive defaults for single-tenant mode.

Phase 2: Schema Registry
- Storage backend (in-memory + SQLite parity with run state store).
- CRUD API + validation + quotas.
- RBAC checks on registry operations.

Phase 3: Discovery Tools
- Implement providers/list, schemas/list/get, scenarios/list.
- Add tool schemas to contract generator.

Phase 4: Trust Lanes
- Enforce policy lattice in predicate evaluation.
- Gate/predicate policy overrides.
- Dev-permissive global toggle.

Phase 5: Precheck
- Implement precheck tool (read-only, audit logged).
- Schema validation + comparator evaluation against asserted data.

Phase 6: Documentation + Examples
- Add docs for trust lanes, registry, precheck.
- Add examples showing asserted vs verified flows.

## Implementation Status Matrix (Current)

Status key:
- Done: implemented with unit/system coverage.
- Partial: implemented but missing policy/config hardening or audit/RBAC guarantees.
- Open: not implemented.

| Plan item | Status | Notes | Code refs |
| --- | --- | --- | --- |
| Trust lanes (verified vs asserted, strictest-wins) | Done | Global + gate/predicate enforcement present. | `decision-gate-core/src/core/evidence.rs`, `decision-gate-core/src/runtime/engine.rs` |
| Trust lane config surface | Done | `trust.min_lane` is the global gate. | `decision-gate-mcp/src/config.rs` |
| Dev-permissive mode toggle + warnings | Open | No explicit untrusted/dev toggle beyond `trust.min_lane`. | `decision-gate-mcp/src/config.rs`, `decision-gate-mcp/src/server.rs` |
| Namespace propagation (ScenarioSpec/RunConfig/tool inputs) | Done | Namespace is first-class across spec, run config, and tool inputs. | `decision-gate-core/src/core/spec.rs`, `decision-gate-mcp/src/tools.rs` |
| Default namespace behavior (non-Asset-Core) | Open | Defaults exist in fixtures but no explicit policy. | `system-tests/tests/helpers/scenarios.rs` |
| Schema registry storage (memory + SQLite) | Done | Versioned, immutable entries. | `decision-gate-core/src/runtime/store.rs`, `decision-gate-store-sqlite/src/store.rs` |
| Registry size limits + quotas | Done | Size + max_entries enforced. | `decision-gate-core/src/runtime/store.rs`, `decision-gate-mcp/src/tools.rs` |
| Registry RBAC/ACL | Open | Only tool allowlists; no per-tenant/role ACL. | `decision-gate-mcp/src/auth.rs`, `decision-gate-mcp/src/tools.rs` |
| Registry audit logging | Partial | MCP request audit exists; no schema-specific hash-only audit. | `decision-gate-mcp/src/audit.rs`, `decision-gate-mcp/src/server.rs` |
| Discovery tools (providers/list, schemas/list/get, scenarios/list) | Done | Tool contracts + router support present. | `decision-gate-mcp/src/tools.rs`, `decision-gate-contract/src/tooling.rs` |
| Precheck tool (read-only) | Done | No run-state mutation; schema-validated. | `decision-gate-mcp/src/tools.rs`, `decision-gate-core/src/runtime/engine.rs` |
| Precheck audit hash-only | Open | Current audit is request-level; no hash-only enforcement. | `decision-gate-mcp/src/audit.rs` |
| Docs + examples for trust lanes/registry/precheck | Partial | Roadmap updated; examples/docs still thin. | `Docs/roadmap/trust_lanes_registry_plan.md` |

## Implementation Checklist (Ordered)

This is the execution checklist to hand to an implementing agent. Each item is
blocking for the next unless noted.

1) Contract + type scaffolding
- Add TrustLane enum and policy lattice types in contract + core crates.
- Add namespace_id to ScenarioSpec, RunConfig, EvidenceContext contracts.
- Add schema registry types (SchemaId, SchemaVersion, SchemaRecord, SchemaRef).
- Add tool contracts for new MCP tools (providers/list, schemas/list/get,
  scenarios/list, precheck).

2) Namespace plumbing
- Propagate namespace_id through MCP tool inputs/outputs.
- Update run state store interfaces to be namespace-scoped.
- Add dev-permissive defaults for single-tenant mode.
- Enforce namespace isolation on scenario registry and run state.

3) Schema registry (storage + API)
- Implement registry storage (in-memory + SQLite parity).
- Enforce size limits, quotas, and immutability (versioned only).
- Add RBAC checks for register/list/get.
- Add audit logging for registry writes.

4) Trust lanes (policy lattice)
- Implement global/scenario/gate/predicate policies.
- Enforce monotonic strictness (strictest wins).
- Add dev-permissive mode toggle (explicit opt-in + warnings).

5) Discovery tools
- Implement providers/list (registry summary + trust policy).
- Implement schemas/list + schemas/get (pagination + filters).
- Implement scenarios/list (namespace scoped).

6) Precheck tool (read-only)
- Validate data payload against schema registry.
- Evaluate comparator logic against asserted evidence.
- Return predicted gate outcomes and decision (no state mutation).
- Emit audit event (hash-only by default).

7) Documentation + examples
- Trust lanes + dev-permissive behavior.
- Registry usage and schema versioning.
- Precheck workflow examples.

## Unit Test Plan (Per-File Failure Modes)

This section enumerates failure-mode tests to implement per file touched. Tests
must be exhaustive for error paths, invalid inputs, and policy violations.

Contract + core type tests:
- decision-gate-contract/src/schemas.rs
  - Invalid trust lane values rejected by schema.
  - Namespace required where mandated.
  - Schema registry records validate type/size constraints.
- decision-gate-contract/src/tooling.rs
  - Tool schemas for providers/list, schemas/list/get, scenarios/list, precheck
    compile and examples validate.
- decision-gate-core/src/core/spec.rs
  - ScenarioSpec rejects missing namespace_id when required.
  - Gate/predicate trust policy overrides parsed and validated.
- decision-gate-core/src/core/evidence.rs
  - EvidenceContext requires namespace_id.

Namespace plumbing:
- decision-gate-mcp/src/tools.rs
  - Missing namespace_id rejected (except dev-permissive default).
  - Cross-namespace scenario_id access rejected.
  - tools/list unaffected.
- decision-gate-core/src/runtime/engine.rs
  - EvidenceContext constructed with namespace_id.
  - Cross-namespace run_id lookup fails closed.

Schema registry:
- decision-gate-mcp/src/registry (new)
  - Register schema succeeds with valid JSON schema.
  - Register schema rejects oversized payload.
  - Register schema rejects duplicate schema_id+version.
  - Get schema not found -> not_found error.
  - List schemas pagination boundaries.
  - RBAC denied -> forbidden error.

Trust lanes:
- decision-gate-core/src/runtime/engine.rs (or new policy module)
  - Evidence fails when lane does not meet policy (Unknown outcome).
  - Monotonic strictness: global < scenario < gate enforced.
  - Dev-permissive mode bypasses asserted restriction with warnings.

Discovery tools:
- decision-gate-mcp/src/tools.rs
  - providers/list returns only allowed providers.
  - schemas/list/get enforces namespace scoping + RBAC.
  - scenarios/list filters by namespace.

Precheck:
- decision-gate-mcp/src/tools.rs (or new precheck module)
  - Valid payload produces deterministic decision.
  - Invalid payload rejected with schema violation details.
  - Precheck does not mutate run state (no new runs, no logs).
  - Audit event emitted with hash-only payload.

## System Test Plan (World-Class Coverage)

Each system test must be registry-driven and assert both success and failure
modes. Add coverage across HTTP + stdio MCP transports where applicable.

Trust lanes:
- verified_only_blocks_asserted
- dev_permissive_allows_asserted_with_warning
- gate_requires_verified_overrides_scenario_allow_asserted

Namespace isolation:
- namespace_required_rejects_missing
- cross_namespace_scenario_access_denied
- namespace_scoped_run_state_isolation

Schema registry:
- register_schema_success
- register_schema_duplicate_version_rejected
- register_schema_oversize_rejected
- list_schemas_pagination
- get_schema_not_found

Discovery tools:
- providers_list_scoped
- schemas_list_scoped
- scenarios_list_scoped

Precheck:
- precheck_valid_payload_predicts_decision
- precheck_invalid_payload_schema_error
- precheck_no_state_mutation
- precheck_audit_hash_only

## Review Checklist (For Next Agent)

- All new MCP tools documented and schema-validated.
- Namespace_id is first-class across contracts, tools, and runtime.
- Trust lanes enforce strictest policy and fail closed.
- Dev-permissive mode is explicit and noisy in logs.
- Schema registry enforces immutability, size limits, and RBAC.
- Precheck never mutates run state.
- Unit tests cover all error paths listed above.
- System tests cover all trust, namespace, registry, and precheck scenarios.

## Acceptance Criteria

- Precheck never mutates run state.
- Trust lattice enforces strictest policy.
- Namespaces are explicit (no silent cross-namespace access).
- Registry isolation is tenant+namespace scoped.
- Discovery tools are RBAC scoped and paginated.
- Dev-permissive mode must be explicit and noisy.

## Risks and Mitigations

- Registry abuse (OOM): enforce size limits + quotas.
- Schema sprawl: require versioning, no in-place mutation.
- Confusing trust semantics: hard defaults + explicit opt-in; document clearly.
- Multi-tenant complexity: keep tenant+namespace model consistent with Asset Core.

## Outstanding Design Decisions (Updated)

Resolved in code:
- evidence_submit vs precheck-only: precheck-only is implemented; no evidence_submit tool exists.
- schema registry aliasing: versioned-only is implemented (schema_id + version), no alias layer.

Still open / not fully implemented:
- Default namespace behavior for non-Asset-Core deployments (dev-permissive single-tenant default vs strict required namespace).
- Dev-permissive (untrusted) mode toggle with explicit warnings (currently only trust.min_lane exists).
- Registry RBAC/ACL beyond tool allowlists (currently no per-tenant/role ACL).
- Precheck audit "hash-only by default" behavior (not yet enforced).
