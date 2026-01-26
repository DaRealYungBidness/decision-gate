<!--
AGENTS.md
============================================================================
Document: Decision Gate Agent Notes
Description: Guidance for maintaining architecture documentation.
Purpose: Keep architecture references current as implementation changes.
============================================================================
-->

# Decision Gate Agent Notes

## Architecture Docs (Current-State)

These documents describe the *current* implementation. When code changes
affect any of the areas below, update the corresponding architecture doc(s)
in the same change so the docs remain accurate.

- `Docs/architecture/comparator_validation_architecture.md`
- `Docs/architecture/decision_gate_assetcore_integration_contract.md`
- `Docs/architecture/decision_gate_namespace_registry_rbac_architecture.md`
- `Docs/architecture/decision_gate_auth_disclosure_architecture.md`
- `Docs/architecture/decision_gate_evidence_trust_anchor_architecture.md`
- `Docs/architecture/decision_gate_runpack_architecture.md`
- `Docs/architecture/decision_gate_scenario_state_architecture.md`
- `Docs/architecture/decision_gate_provider_capability_architecture.md`
- `Docs/architecture/decision_gate_system_test_architecture.md`

## OSS vs Enterprise Boundary (Authoritative)

Decision Gate is open-core. Enterprise features must NOT contaminate OSS crates.
When adding enterprise functionality, follow these rules:

- **No enterprise deps in OSS crates.** OSS crates may define traits/interfaces
  but must not depend on enterprise crates.
- **Enterprise code lives under `enterprise/` only.** Keep private crates and
  tests isolated in that subtree.
- **OSS remains deterministic and auditable.** Enterprise features must not
  change core semantics or weaken security defaults.
- **Seams, not forks.** Extend via traits/config (authz, stores, audit sinks)
  instead of modifying OSS behavior for enterprise needs.
- **Tests stay split.** OSS system tests remain in `system-tests/`. Enterprise
  system tests live in `enterprise/enterprise-system-tests/` once created.
