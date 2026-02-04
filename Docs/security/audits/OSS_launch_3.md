<!--
Docs/security/audits/OSS_launch_3.md
============================================================================
Document: OSS Launch Audit Findings (Pass 3)
Description: Track security and test gaps for OSS launch readiness.
Purpose: Record per-crate follow-ups that exceed a single pass.
Dependencies:
  - Docs/security/threat_model.md
  - Docs/security/audits/OSS_launch_2.md
============================================================================
-->

# OSS Launch Audit (Pass 3)

## decision-gate-broker

### Open Findings

None.

System-test gaps: none identified in this pass.

### Closed Findings

1. HTTP source policy checks were performed before the request, but DNS was
   resolved again during the actual connection. This allowed DNS rebinding or
   time-of-check/time-of-use bypass of private or link-local IP guards for
   hostnames.
   - Status: Closed (2026-02-04).
   - Severity: Medium (SSRF / private network access risk).
   - Resolution: Pinned HTTP requests to resolved IPs and re-validated the
     pinned peer IP before accepting responses; added per-request resolution
     and retry behavior in the HTTP source.

## decision-gate-mcp

### Open Findings

None.

System-test gaps: none identified in this pass.

### Closed Findings

1. Public API docs in `decision-gate-mcp` did not consistently declare explicit
   `# Invariants` sections for public structs/enums as required by
   `Docs/standards/codebase_formatting_standards.md`.
   - Status: Closed (2026-02-04).
   - Severity: Low (documentation consistency).
   - Resolution: Added explicit `# Invariants` sections to all public types.

## decision-gate-contract

### Open Findings

None.

System-test gaps: none identified in this pass.

### Closed Findings

None.

## decision-gate-sdk-gen

### Open Findings

None.

System-test gaps: No system-tests cover the SDK generator CLI or output drift
checks; only crate-level tests exist in `decision-gate-sdk-gen/tests/`.

### Closed Findings

None.

## decision-gate-store-sqlite

### Open Findings

None.

System-test gaps: No system-tests target the SQLite store or schema registry
durability/integrity paths in this pass.

### Closed Findings

1. SQLite schema registry enforcement relied on MCP-layer limits; the store
   itself did not enforce `schema_registry.max_entries` (or smaller
   `max_schema_bytes`) when used directly as a library.
   - Status: Closed (2026-02-04).
   - Severity: Medium (resource exhaustion / storage growth).
   - Resolution: Added optional registry limits to `SqliteStoreConfig` and
     enforced schema size and entry count limits within the SQLite-backed
     registry, with unit tests covering both limits.

## ret-logic

### Open Findings

None.

System-test gaps: ret-logic has no system-tests covering evaluation under
hostile inputs or deep trees in this pass.

### Closed Findings

1. `Requirement::eval` and `Requirement::eval_tristate` recurse without an
   internal depth guard. Callers could bypass `RequirementValidator` and trigger
   stack exhaustion with hostile or malformed requirement trees.
   - Status: Closed (2026-02-04).
   - Severity: Medium (resource exhaustion / crash risk).
   - Resolution: Added a default depth guard (`MAX_EVAL_DEPTH`) that fails
     closed across boolean, batch, and tri-state evaluation paths, with unit
     tests covering depth overflow behavior.

## decision-gate-provider-sdk

### Open Findings

None.

System-test gaps: Provider SDK templates are not exercised by `system-tests/`.
Consider a small integration test that compiles/runs each template and calls
`tools/list` and `tools/call` through the MCP server.

### Closed Findings

1. Provider templates returned JSON-RPC errors for missing evidence params,
   conflicting with the protocol guidance to return structured
   `EvidenceResult.error` metadata for invalid or missing evidence.
   - Status: Closed (2026-02-04).
   - Severity: Low (protocol consistency).
   - Resolution: Templates now return `EvidenceResult` error metadata and keep
     JSON-RPC errors reserved for malformed requests.

## system-tests

### Open Findings

None.

System-test gaps: Long-running soak/perf regression coverage remains unimplemented.
Tracked in `system-tests/test_gaps.toml` (id: `stress-soak-perf`).

### Closed Findings

None.
