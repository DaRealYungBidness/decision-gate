<!--
Docs/security/audits/OSS_launch.md
============================================================================
Document: OSS Launch Audit Findings
Description: Track security and test gaps for OSS launch readiness.
Purpose: Record per-crate follow-ups that exceed a single pass.
Dependencies:
  - Docs/security/threat_model.md
  - Docs/security/audit.md
============================================================================
-->

# OSS Launch Audit

## decision-gate-core

### Open Findings

1. Missing system-level fuzz/property coverage for schema validation and cursor parsing.
   - Status: Open.
   - Severity: Medium (input validation / robustness).
   - Impact: `DataShapeRegistry` cursor parsing and schema validation lack fuzz/property tests; could miss adversarial edge cases in registry/listing and precheck paths.
   - Evidence: `system-tests/tests/suites/stress.rs` TODOs.
   - Recommended remediation: Add fuzz/property system tests covering cursor parsing, schema validation, and pathological inputs; exercise failure modes to ensure fail-closed behavior.

### Closed Findings

None.

## decision-gate-cli

### Open Findings

1. Missing system-test coverage for CLI workflows (serve/runpack/authoring/interop).
   - Status: Open.
   - Severity: Medium (coverage gap for entry-point workflows).
   - Impact: CLI integration paths are covered by unit/integration tests but not exercised end-to-end in `system-tests/`, leaving gaps for packaging, transport, and policy wiring.
   - Evidence: No CLI-focused suites under `system-tests/`.
   - Recommended remediation: Add system-tests that invoke the CLI against a test MCP server, including runpack export/verify, authoring validate/normalize, serve with local-only enforcement, and interop evaluation.

### Closed Findings

None.

## ret-logic

### Open Findings

1. Missing system-test coverage for ret-logic evaluation paths.
   - Status: Open.
   - Severity: Low (coverage gap).
   - Impact: Requirement parsing/serialization and plan execution are validated via unit/integration
     tests, but there are no `system-tests/` suites exercising ret-logic through
     end-to-end Decision Gate flows or CLI-driven authoring scenarios.
   - Evidence: No ret-logic-focused suites under `system-tests/`.
   - Recommended remediation: Add system-tests that load authored requirements (RON/DSL),
     compile plans, and evaluate them through Decision Gate entry points to exercise
     fail-closed behavior under malformed inputs.

2. Recursive evaluation APIs lack explicit depth guards when used without validation.
   - Status: Open.
   - Severity: Low (defense-in-depth).
   - Impact: `Requirement::eval` and `Requirement::eval_tristate` are recursive and can
     overflow the stack if callers bypass `RequirementValidator` and construct extremely
     deep trees from untrusted input.
   - Evidence: `ret-logic/src/requirement.rs`.
   - Recommended remediation: Provide optional depth-limited evaluators or add a guard
     that mirrors validator limits for direct evaluation paths.

### Closed Findings

None.

## decision-gate-sdk-gen

### Open Findings

1. Missing system-test coverage for SDK generator CLI workflows.
   - Status: Open.
   - Severity: Low (coverage gap for tooling entry points).
   - Impact: SDK generation is covered by crate-level integration tests, but no
     end-to-end system-tests exercise the CLI against actual workspace outputs.
   - Evidence: No `system-tests/` suites invoke `decision-gate-sdk-gen`.
   - Recommended remediation: Add system-tests that run
     `decision-gate-sdk-gen generate` and `decision-gate-sdk-gen check` against
     a fixture tooling.json and validate outputs in a temp workspace.

### Closed Findings

None.

## decision-gate-provider-sdk

### Open Findings

1. Missing system-test coverage for provider templates (stdio framing + tool responses).
   - Status: Open.
   - Severity: Medium (coverage gap for external provider integration paths).
   - Impact: Template providers are not exercised in `system-tests/`, leaving gaps for framing limits, tool responses, and evidence lane handling across languages.
   - Evidence: No provider-template-focused suites under `system-tests/`.
   - Recommended remediation: Add system tests that spawn the Go/Python/TypeScript templates (stdio + optional HTTP) and validate `tools/list`, `tools/call`, framing limits, and fail-closed error behavior.

### Closed Findings

None.

## decision-gate-store-sqlite

### Open Findings

1. Missing system-test coverage for SQLite-backed persistence workflows.
   - Status: Open.
   - Severity: Medium (coverage gap for durability + integrity at system boundary).
   - Impact: SQLite run state and schema registry behavior is covered by unit tests, but end-to-end paths (MCP/CLI integration, runpack export/verify with persisted state) are not exercised in `system-tests/`.
   - Evidence: No SQLite-focused suites under `system-tests/`.
   - Recommended remediation: Add system tests that run Decision Gate with the SQLite store enabled, exercise scenario lifecycle + registry operations across restarts, and verify integrity checks on corrupted or oversized records.

### Closed Findings

None.
