<!--
Docs/security/audit.md
============================================================================
Document: Decision Gate Security Audit Log
Description: Track security issues, findings, and remediation status.
Purpose: Provide per-crate security findings with status and next steps.
Dependencies:
  - Docs/security/threat_model.md
  - Docs/standards/codebase_engineering_standards.md
============================================================================
-->

# Security Audit

## Overview
This document tracks security findings by crate or subsystem. Each item should
record status, severity, impacted files, and recommended remediation steps.

## decision-gate-broker

### Open Findings

1. Unbounded payload sizes in broker sources allow memory exhaustion.
   - Status: Open.
   - Severity: High (resource exhaustion / DoS).
   - Impact: `FileSource::fetch` reads entire files; `HttpSource::fetch` loads
     full responses; `InlineSource::fetch` decodes base64 into an unbounded
     `Vec<u8>`.
   - Affected files: `decision-gate-broker/src/source/file.rs`,
     `decision-gate-broker/src/source/http.rs`,
     `decision-gate-broker/src/source/inline.rs`.
   - Recommendation: enforce explicit byte caps for each source, reject
     oversized payloads early, and add tests that assert fail-closed behavior.

## decision-gate-cli

### Open Findings

None.

### Closed Findings

1. CLI file reads were unbounded and could exhaust memory on large inputs.
   - Status: Closed.
   - Severity: High (resource exhaustion / DoS).
   - Impact: `runpack export` spec/state reads, `runpack verify` manifest reads,
     and authoring input reads used unbounded `fs::read`/`read_to_string`.
   - Affected files: `decision-gate-cli/src/main.rs`.
   - Resolution: added hard byte limits and fail-closed errors for CLI input
     files aligned with runpack artifact sizing.

2. Runpack manifest name accepted path traversal outside the output directory.
   - Status: Closed.
   - Severity: Medium (path traversal / overwrite risk).
   - Impact: `runpack export --manifest-name ../...` could write manifests
     outside the intended output directory.
   - Affected files: `decision-gate-mcp/src/runpack.rs`,
     `decision-gate-cli/src/main.rs`.
   - Resolution: validate manifest paths as relative, non-traversing, and
     within length limits; added traversal rejection tests.

## decision-gate-core

### Open Findings

None.

### Closed Findings

1. Runpack verification reads unbounded artifact sizes into memory.
   - Status: Closed.
   - Severity: High (resource exhaustion / DoS).
   - Impact: `RunpackVerifier::verify_manifest` reads full artifacts into
     memory and hashes them without size limits; malicious runpacks can exhaust
     memory during offline verification.
   - Affected files: `decision-gate-core/src/runtime/runpack.rs`,
     `decision-gate-core/src/interfaces/mod.rs`.
   - Resolution: added `read_with_limit` enforcement with hard caps and a
     fail-closed verifier test.

2. Evidence and submission payload hashing has no size limits.
   - Status: Closed.
   - Severity: High (resource exhaustion / DoS).
   - Impact: `normalize_evidence_result` and `payload_hash` will hash arbitrary
     byte payloads, allowing untrusted providers or submissions to cause
     unbounded memory and CPU usage.
   - Affected files: `decision-gate-core/src/runtime/engine.rs`.
   - Resolution: added hard byte caps with typed errors and fail-closed tests.

## decision-gate-mcp

### Open Findings

None.

### Closed Findings

1. External MCP provider HTTP calls lacked request timeouts.
   - Status: Closed.
   - Severity: Medium (availability / DoS).
   - Impact: `McpProviderClient` used a blocking HTTP client without timeouts.
   - Affected files: `decision-gate-mcp/src/evidence.rs`,
     `decision-gate-mcp/src/config.rs`.
   - Resolution: added per-provider HTTP timeouts with bounded validation and
     fail-closed error handling plus system-test coverage.

2. Unbounded MCP stdio payload sizes allowed memory exhaustion.
   - Status: Closed.
   - Severity: High (resource exhaustion / DoS).
   - Impact: stdio transport and external provider framing accepted unbounded
     `Content-Length` values, allowing oversized allocations.
   - Affected files: `decision-gate-mcp/src/server.rs`,
     `decision-gate-mcp/src/evidence.rs`.
   - Resolution: enforced `max_body_bytes` for stdio server requests and added
     hard caps on provider responses (stdio and HTTP) with fail-closed tests.

## decision-gate-provider-sdk

### Open Findings

None.

### Closed Findings

1. Provider SDK stdio templates accepted unbounded Content-Length values.
   - Status: Closed.
   - Severity: High (resource exhaustion / DoS).
   - Impact: template servers could allocate or read unbounded payloads,
     allowing memory exhaustion through oversized frames.
   - Affected files: `decision-gate-provider-sdk/typescript/src/index.ts`,
     `decision-gate-provider-sdk/python/provider.py`,
     `decision-gate-provider-sdk/go/main.go`.
   - Resolution: enforced header/body size caps (8 KiB/1 MiB), discarded
     oversized payloads, and documented the limits in template READMEs.

## decision-gate-providers

### Open Findings

None.

### Closed Findings

1. HTTP provider followed redirects, bypassing host allowlist checks.
   - Status: Closed.
   - Severity: High (SSRF / allowlist bypass).
   - Impact: redirect responses could send requests to disallowed hosts even
     when `allowed_hosts` was configured.
   - Affected files: `decision-gate-providers/src/http.rs`.
   - Resolution: disabled automatic redirects in the HTTP provider client and
     added a regression test to assert 302 responses are returned directly.

## decision-gate-store-sqlite

### Open Findings

1. Unbounded run state blob loads can exhaust memory.
   - Status: Open.
   - Severity: High (resource exhaustion / DoS).
   - Impact: `SqliteRunStateStore::load_state` reads `state_json` blobs without
     a byte limit; a tampered SQLite file can force large allocations.
   - Affected files: `decision-gate-store-sqlite/src/store.rs`.
   - Recommendation: enforce a hard upper bound aligned with runpack artifact
     limits or stream blobs with explicit caps before deserialization; add
     oversized-blob tests that assert fail-closed behavior.

## ret-logic

### Open Findings

1. DSL parsing has no input size cap and uses recursive descent before depth
   validation, enabling stack exhaustion or large allocations with adversarial
   input.
   - Status: Open.
   - Severity: High (resource exhaustion / DoS).
   - Impact: `Parser` recursion and full-token buffering occur before structural
     depth checks.
   - Affected files: `ret-logic/src/dsl.rs`.
   - Recommendation: enforce maximum input length and nesting depth during
     parse (fail closed), or replace recursion with an explicit stack.

2. PlanExecutor stack depth (16) is lower than the default validation depth
   (32), causing deep but otherwise valid requirements to evaluate as false
   without diagnostics.
   - Status: Open.
   - Severity: Medium (availability / correctness).
   - Impact: nested plans beyond the executor stack size early-return `false`,
     which can be triggered by untrusted plans.
   - Affected files: `ret-logic/src/executor.rs`, `ret-logic/src/dsl.rs`,
     `ret-logic/src/serde_support.rs`.
   - Recommendation: align depth limits across parser/validator/executor and
     surface over-depth as a typed error.

3. ExecutorBuilder leaks dispatch tables by using `Box::leak`.
   - Status: Open.
   - Severity: Low (resource leak).
   - Impact: repeated `ExecutorBuilder::build` calls can leak memory over time.
   - Affected files: `ret-logic/src/executor.rs`.
   - Recommendation: require static dispatch tables or return an owned/Arc table
     to avoid leaking allocations.

### Closed Findings

1. Unbounded RON file reads could exhaust memory during requirement loading.
   - Status: Closed.
   - Severity: Medium (resource exhaustion / DoS).
   - Impact: `ron_utils::load_from_file` and `ron_utils::validate_file` used
     unbounded `fs::read_to_string` on attacker-controlled paths.
   - Affected files: `ret-logic/src/serde_support.rs`.
   - Resolution: added `MAX_RON_FILE_BYTES` cap with bounded reads and
     fail-closed errors for oversized files.
