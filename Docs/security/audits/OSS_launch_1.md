<!--
Docs/security/audits/OSS_launch_1.md
============================================================================
Document: OSS Launch Audit Findings (Pass 1)
Description: Track security and test gaps for OSS launch readiness.
Purpose: Record per-crate follow-ups that exceed a single pass.
Dependencies:
  - Docs/security/threat_model.md
  - Docs/security/audits/OSS_launch_0.md
============================================================================
-->

# OSS Launch Audit (Pass 1)

## decision-gate-core

### Open Findings

None.

### Closed Findings

1. Missing system-level fuzz/property coverage for schema validation and cursor parsing.
   - Status: Closed (2026-01-29).
   - Severity: Medium (input validation / robustness).
   - Resolution: Added deterministic malformed cursor/limit coverage plus invalid schema + precheck payload validation in `system-tests/tests/suites/schema_registry_fuzz.rs`.

## decision-gate-cli

### Open Findings

None.

### Closed Findings

1. Missing system-test coverage for CLI workflows (serve/runpack/authoring/interop).
   - Status: Closed (2026-01-29).
   - Severity: Medium (coverage gap for entry-point workflows).
   - Resolution: Added `system-tests/tests/suites/cli_workflows.rs` covering serve, runpack export/verify, authoring validate/normalize, config validate, provider discovery, interop eval, and non-loopback bind enforcement.

## ret-logic

### Open Findings

None.

### Closed Findings

1. Missing system-test coverage for ret-logic evaluation paths.
   - Status: Closed (2026-01-29).
   - Severity: Low (coverage gap).
   - Resolution: Added `system-tests/tests/suites/ret_logic_authoring.rs` covering RON normalization + execution and DSL-based execution/depth rejection.

2. Recursive evaluation APIs lack explicit depth guards when used without validation.
   - Status: Closed (2026-01-29).
   - Severity: Low (defense-in-depth).
   - Resolution: DSL depth guard coverage added in `system-tests/tests/suites/ret_logic_authoring.rs` via deep-nesting rejection.

## decision-gate-sdk-gen

### Open Findings

None.

### Closed Findings

1. Missing system-test coverage for SDK generator CLI workflows.
   - Status: Closed (2026-01-29).
   - Severity: Low (coverage gap for tooling entry points).
   - Resolution: Added `system-tests/tests/suites/sdk_gen_cli.rs` covering generate/check, drift detection, and invalid input handling.

## decision-gate-provider-sdk

### Open Findings

None.

### Closed Findings

1. Missing system-test coverage for provider templates (stdio framing + tool responses).
   - Status: Closed (2026-01-29).
   - Severity: Medium (coverage gap for external provider integration paths).
   - Resolution: Added `system-tests/tests/suites/provider_templates.rs` covering python/go/ts templates, framing limits, and Decision Gate flows.

## decision-gate-store-sqlite

### Open Findings

None.

### Closed Findings

1. Missing system-test coverage for SQLite-backed persistence workflows.
   - Status: Closed (2026-01-29).
   - Severity: Medium (coverage gap for durability + integrity at system boundary).
   - Resolution: Added `system-tests/tests/suites/sqlite_registry_runpack.rs` covering registry + runpack persistence and oversize schema rejection.

## decision-gate-enterprise

**Note**: The `decision-gate-enterprise` paths referenced below live in the
private AssetCore monorepo and are not present in the OSS repository.

### Open Findings

None. Enterprise system-tests cover audit, authz, usage, and storage paths in this pass.

### Closed Findings

1. Audit chain integrity was not validated on load.
   - Status: Closed (2026-01-29).
   - Severity: Medium (tamper-evidence hardening).
   - Resolution: Validate hash chain on startup in `decision-gate-enterprise/src/audit_chain.rs` and added unit coverage.

2. Tenant admin store allowed issuing keys for unknown tenants.
   - Status: Closed (2026-01-29).
   - Severity: Low (fail-closed alignment).
   - Resolution: Enforced tenant existence checks in `decision-gate-enterprise/src/tenant_admin.rs` and updated tests.

3. Namespace authority lacked unit coverage for lifecycle and tenant enforcement.
   - Status: Closed (2026-01-29).
   - Severity: Low (coverage gap).
   - Resolution: Added `decision-gate-enterprise/tests/namespace_authority.rs`.
