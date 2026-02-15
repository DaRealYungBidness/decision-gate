<!--
Docs/security/audits/OSS_launch_4.md
============================================================================
Document: OSS Launch Audit Findings (Pass 4)
Description: Track security and test gaps for OSS launch readiness.
Purpose: Record per-crate follow-ups that exceed a single pass.
Dependencies:
  - Docs/security/threat_model.md
  - Docs/security/audits/OSS_launch_3.md
============================================================================
-->

# OSS Launch Audit (Pass 4)

## decision-gate-config

### Open Findings

None.

System-test gaps: none identified in this pass.

### Closed Findings

1. Config validation accepted malformed endpoint URLs in
   `providers.url`, `runpack_storage.endpoint`, and
   `namespace.authority.assetcore.base_url`.
   - Status: Closed (2026-02-15).
   - Severity: Medium (fail-closed validation gap and runtime instability risk).
   - Resolution: Added strict HTTP(S) URL validation in config loading
     (trimmed input, no whitespace/control characters, required scheme, required
     host), plus schema constraints and unit-test coverage.

2. Bearer token validation accepted whitespace and control characters for
   inbound server auth tokens and outbound provider auth tokens.
   - Status: Closed (2026-02-15).
   - Severity: Medium (ambiguous credential handling and header-splitting risk).
   - Resolution: Added fail-closed token sanitization
     (reject whitespace/control characters), aligned schema patterns for
     server bearer tokens, and expanded boundary/security test coverage.

## decision-gate-mcp

### Open Findings

None.

System-test gaps: none identified in this pass.

### Closed Findings

1. Stdio framing in MCP server/provider paths only bounded body size; hostile
   peers could send oversized headers before `Content-Length` parsing and force
   unbounded pre-parse memory growth.
   - Status: Closed (2026-02-15).
   - Severity: Medium (resource exhaustion / availability risk).
   - Resolution: Added explicit cumulative framing-header limits (8 KiB) and
     fail-closed duplicate `Content-Length` rejection in `server` and
     external-provider `evidence` parsers, with unit tests.
2. Docs extra-path ingestion recursively traversed directories without
   deterministic ordering or early budget caps, allowing startup-time
   amplification and nondeterministic duplicate-id suffix assignment.
   - Status: Closed (2026-02-15).
   - Severity: Low (determinism drift + startup amplification risk).
   - Resolution: Added deterministic lexicographic traversal, canonical
     visited-directory tracking, and ingestion budgets tied to remaining
     `max_docs`/`max_total_bytes`, with unit tests.

## decision-gate-store-sqlite

### Open Findings

None.

System-test gaps: none identified in this pass.

### Closed Findings

1. Run-state loads trusted `runs.latest_version` without cross-checking
   `run_state_versions`, allowing tampered metadata to replay stale state or
   hide orphaned rows.
   - Status: Closed (2026-02-15).
   - Severity: Medium (integrity / replay risk under untrusted storage).
   - Resolution: Added fail-closed latest-version reconciliation between
     `runs` and `run_state_versions` and regression tests for mismatch/orphan
     scenarios.

2. Schema registry reads trusted `schema_size_bytes` metadata and
   `registry_namespace_counters.entry_count` without verifying against actual
   stored rows, allowing tampered metadata to bypass size/entry limits.
   - Status: Closed (2026-02-15).
   - Severity: Medium (resource exhaustion and policy bypass under tampered
     storage).
   - Resolution: Added fail-closed metadata consistency checks (`length(schema_json)`
     and real row counts), and regression tests for mismatched size metadata and
     counter tampering.

## decision-gate-cli

### Open Findings

None.

System-test gaps: none identified in this pass.

### Closed Findings

1. The stdio JSON-RPC framing parser accepted unbounded header-line growth and
   tolerated duplicate `Content-Length` headers, allowing hostile stdio peers
   to force memory growth or create ambiguous framing.
   - Status: Closed (2026-02-15).
   - Severity: Medium (resource exhaustion / protocol ambiguity).
   - Resolution: Added strict stdio header limits (total bytes, line bytes, and
     line count) and fail-closed duplicate `Content-Length` rejection in
     `crates/decision-gate-cli/src/mcp_client.rs`, with unit-test coverage.

## decision-gate-contract

### Open Findings

None.

System-test gaps: none identified in this pass.

### Closed Findings

1. `evidence_query` tooling contract allowed `context` payloads without
   `namespace_id`, drifting from `decision_gate_core::EvidenceContext` and the
   fail-closed contract expectations for tenant/namespace scoping.
   - Status: Closed (2026-02-15).
   - Severity: Medium (contract/runtime drift at an authorization boundary).
   - Resolution: Updated `crates/decision-gate-contract/src/tooling.rs` so
     `evidence_context_schema` requires `namespace_id`, and added regression
     coverage in `crates/decision-gate-contract/src/tooling/tests.rs`.
2. `ContractBuilder` symlink safety checks in `write_to`/`verify_output` used
   pre-write path inspection (`symlink_metadata`) followed by normal
   filesystem writes/reads, leaving a TOCTOU race window under concurrent local
   filesystem mutation.
   - Status: Closed (2026-02-15).
   - Severity: Low (local integrity risk under concurrent attacker control of
     the output tree).
   - Resolution: Reworked contract output writes/verifies onto
     descriptor-relative, no-follow file operations with atomic sibling-temp
     writes + rename in `crates/decision-gate-contract/src/contract.rs`, and
     added unit/system coverage for symlinked output and artifact-path rejection
     in `crates/decision-gate-contract/src/contract/tests.rs` and
     `system-tests/tests/suites/contract_cli.rs`.

## decision-gate-providers

### Open Findings

None.

System-test gaps: none identified in this pass.

### Closed Findings

1. HTTP provider accepted URLs with embedded credentials
   (`user[:pass]@host`), which can leak sensitive material through logs and
   evidence metadata.
   - Status: Closed (2026-02-15).
   - Severity: Low (credential hygiene / data exposure).
   - Resolution: URL validation now rejects credential-bearing URLs; added
     integration test coverage in `crates/decision-gate-providers/tests/http_provider.rs`.
2. HTTP provider host allowlist checks operated on URL hostname text only and
   did not pin eventual peer IPs, allowing DNS-rebinding to private/link-local
   destinations.
   - Status: Closed (2026-02-15).
   - Severity: Medium (SSRF / internal network access).
   - Resolution: Added per-request DNS resolution + pinned connection behavior
     with peer-IP policy enforcement in
     `crates/decision-gate-providers/src/http.rs` (default deny private/link-local
     including IPv4-mapped IPv6 unless `allow_private_networks=true`), plus
     crate and system-test coverage in
     `crates/decision-gate-providers/tests/http_provider.rs`,
     `crates/decision-gate-providers/tests/http_provider_unit.rs`, and
     `system-tests/tests/suites/providers.rs`.

## decision-gate-core

### Open Findings

None.

System-test gaps: none identified in this pass.

### Closed Findings

None.

## decision-gate-sdk-gen

### Open Findings

None.

System-test gaps: none identified in this pass.

### Closed Findings

1. SDK generation accepted untrusted schema property names without validating
   Python identifier safety, which could emit invalid Python `TypedDict` code
   or unsafe generated surfaces under hostile tooling input.
   - Status: Closed (2026-02-15).
   - Severity: Medium (codegen integrity / fail-closed input handling).
   - Resolution: Added fail-closed Python identifier validation for schema
     properties and defensive quoting for TypeScript interface property keys,
     with hostile-input tests.

## decision-gate-broker

### Open Findings

None.

System-test gaps: none identified in this pass.

### Closed Findings

1. HTTP source private-network blocking did not cover IPv4-mapped IPv6
   addresses (for example, `::ffff:127.0.0.1`). This allowed bypass of the
   default SSRF guard for loopback/private IPv4 ranges when represented as IPv6
   literals.
   - Status: Closed (2026-02-15).
   - Severity: Medium (SSRF / private network access risk).
   - Resolution: Extended IP classification to apply private/link-local checks
     to IPv4-mapped IPv6 addresses and added unit tests for both deny-default
     and allow-private policy behavior.
