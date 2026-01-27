<!--
Docs/roadmap/world_class_implementation_roadmap.md
============================================================================
Document: Decision Gate World-Class Implementation Roadmap
Description: End-state definition for features, docs, and test coverage.
Purpose: Specify the full, world-class implementation bar for open core.
Dependencies:
  - Docs/roadmap/open_items.md
  - Docs/security/threat_model.md
  - Docs/architecture/comparator_validation_architecture.md
============================================================================
-->

# Decision Gate World-Class Implementation Roadmap

## Overview
This roadmap defines the **full, world-class implementation bar** for Decision
Gate as an open-core system suitable for hyperscalers, DoD, and high-assurance
customers. It specifies feature requirements, documentation updates, and test
coverage for unit and system tests.

**Compatibility policy**: breaking changes are allowed. Update schemas, tool
contracts, and docs freely to reach the best design.

## Global Quality Bar (Non-Negotiable)
- Deterministic behavior across runs and platforms (RFC 8785 canonical JSON).
- Fail-closed semantics at every trust boundary.
- Explicit, versioned contracts for all tool inputs/outputs.
- Strict limits on untrusted input sizes and resource usage.
- Every security-sensitive feature has audit logs and tests.

## Implementation Phases

### Phase 0: Contract and Doc Alignment (Mandatory)
**Features**:
- Bump contract version if required for breaking changes.
- Align tool schemas and tooltips with runtime behavior.

**Docs updates**:
- `Docs/roadmap/open_items.md` (status and priorities)
- `Docs/security/threat_model.md` (new trust boundaries)
- `Docs/generated/decision-gate/` (regenerated artifacts)

**Unit tests**:
- Contract schema validation in `decision-gate-contract/tests/schema_validation.rs`.

**System tests**:
- `system-tests/tests/suites/contract.rs` (schema conformance across tools).

---

### Phase 1: Policy Engine Integration (P1)
**Status**: Implemented (static policy engine only).
**Features**:
- Add a real policy adapter interface with **swappable engines** (OPA, Cedar,
  OpenFGA/Zanzibar-style, custom in-house).
- Provide a deterministic, local reference engine (static rules) as the
  default implementation.
- Ensure policy evaluation is deterministic and pure.
- Separate deny vs error paths with typed errors.

**Primary code locations**:
- `decision-gate-core/src/interfaces/mod.rs` (PolicyDecider)
- `decision-gate-core/src/runtime/engine.rs` (policy enforcement)
- `decision-gate-mcp/src/tools.rs` (policy wiring)
- `decision-gate-mcp/src/policy.rs` (policy engines)
- `decision-gate-contract/src/schemas.rs` (policy config schema)

**Docs updates**:
- `Docs/guides/security_guide.md` (policy boundaries)
- `Docs/guides/integration_patterns.md` (policy usage patterns)

**Unit tests**:
- New policy decision tests in `decision-gate-core/tests/policy.rs`.
- Tool adapter policy tests in `decision-gate-mcp/tests/tool_adapter.rs`.
- Static policy engine tests in `decision-gate-mcp/tests/policy_engine.rs`.

**System tests**:
- `policy_denies_dispatch_targets` in `system-tests/tests/suites/security.rs`.
- `policy_error_fails_closed` in `system-tests/tests/suites/security.rs`.

**Roadmap (External Adapters)**:
- Add pluggable adapters for OPA, Cedar, and OpenFGA/Zanzibar-style engines.
- Define adapter config schemas and include them in the contract bundle.
- Add system tests for adapter denial/error behavior and determinism.

---

### Phase 2: Dev-Permissive Mode + Default Namespace Policy (P1)
**Status**: Implemented.
**Features**:
- Explicit dev-permissive toggle (asserted evidence allowed).
- Strict mode rejects default namespace unless explicit.
- Startup emits warnings when dev-permissive is enabled.

**Primary code locations**:
- `decision-gate-mcp/src/config.rs` (config surface)
- `decision-gate-mcp/src/server.rs` (startup warnings)
- `decision-gate-mcp/src/tools.rs` (namespace enforcement)

**Docs updates**:
- `Docs/guides/security_guide.md` (mode behavior)
- `Docs/configuration/` (config docs)

**Unit tests**:
- Config validation in `decision-gate-mcp/tests/config_validation.rs`.
- Tool namespace checks in `decision-gate-mcp/tests/tool_router.rs`.

**System tests**:
- `strict_mode_rejects_default_namespace` in `system-tests/tests/suites/security.rs`.
- `dev_permissive_emits_warning` in `system-tests/tests/suites/operations.rs`.

---

### Phase 3: Schema Registry RBAC/ACL + Audit Events (P1)
**Features**:
- Per-tenant and per-namespace ACL rules for registry operations.
- Audit log entries for registry writes (hash-only payloads).
- Optional audit for registry reads, configurable.

**Primary code locations**:
- `decision-gate-mcp/src/auth.rs` (authorization policy)
- `decision-gate-mcp/src/tools.rs` (schemas_register/list/get)
- `decision-gate-mcp/src/audit.rs` (audit events)

**Docs updates**:
- `Docs/guides/security_guide.md` (registry trust boundary)
- `Docs/decision_gate_data_shapes.md` (registry policy and audit)

**Unit tests**:
- ACL rejection tests in `decision-gate-mcp/tests/tool_router.rs`.
- Audit formatting tests in `decision-gate-mcp/tests/mcp_hardening.rs`.

**System tests**:
- Add `schema_register_denied_by_acl` to `system-tests/tests/suites/security.rs`.
- Add `schema_register_audited_hash_only` to `system-tests/tests/suites/operations.rs`.

---

### Phase 4: Precheck Hash-Only Audit Logging (P1)
**Status**: Implemented.
**Features**:
- Emit hash-only audit events for precheck request/response by default.
- Never log raw asserted payload unless explicit opt-in.

**Primary code locations**:
- `decision-gate-mcp/src/audit.rs`
- `decision-gate-mcp/src/tools.rs` (precheck handler)

**Docs updates**:
- `Docs/guides/security_guide.md` (precheck audit behavior)

**Unit tests**:
- Audit payload redaction tests in `decision-gate-mcp/tests/mcp_hardening.rs`.

**System tests**:
- `precheck_audit_hash_only` in `system-tests/tests/suites/operations.rs`.

---

### Phase 5: Durable Runpack Storage Beyond Filesystem (P1)
**Features**:
- Implement object-store `ArtifactSink`/`ArtifactReader` adapters.
- Strict path and size validation for external storage.

**Primary code locations**:
- `decision-gate-core/src/interfaces/mod.rs` (ArtifactSink/Reader traits)
- `decision-gate-mcp/src/runpack.rs` (file-backed reference)

**Docs updates**:
- `Docs/guides/security_guide.md` (runpack storage threat model)
- `Docs/guides/integration_patterns.md` (storage examples)

**Unit tests**:
- Adapter-specific tests in a new module under `decision-gate-mcp/tests/`.

**System tests**:
- Add `runpack_export_object_store` to `system-tests/tests/suites/runpack.rs`.

---

### Phase 6: Documentation and Examples (P2 but required for world-class)
**Features**:
- Add canonical examples for hold/unknown/branch outcomes.
- Add a run lifecycle guide with tool-call timeline and runpack artifacts.

**Primary docs**:
- `Docs/guides/run_lifecycle.md` (new)
- `Docs/generated/decision-gate/examples/` (new examples)
- `Docs/guides/predicate_authoring.md` (examples reference)

**System tests**:
- Add `hold_unknown_examples_valid` to `system-tests/tests/suites/contract.rs`.

---

## Acceptance Criteria (World-Class)
- All P1 features implemented with unit + system coverage.
- Security guide and threat model updated for every new boundary.
- Contract schemas/tooltips regenerated and aligned with runtime behavior.
- System tests P0/P1 pass on Windows and Linux.
- No silent defaults that weaken security; all relaxations require explicit config.

## Recommended Execution Order
1) Phase 0 (contract/doc alignment)
2) Phase 4 (precheck audit logging)
3) Phase 3 (registry ACL + audit)
4) Phase 2 (dev-permissive + namespace policy)
5) Phase 1 (policy engine integration)
6) Phase 5 (durable runpack storage)
7) Phase 6 (docs/examples)
