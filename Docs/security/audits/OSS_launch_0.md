<!--
Docs/security/audits/OSS_launch_0.md
============================================================================
Document: OSS Launch Audit Findings (Pass 0)
Description: Baseline security audit findings for OSS launch readiness.
Purpose: Provide per-crate security findings with status and next steps.
Dependencies:
  - Docs/security/threat_model.md
  - Docs/standards/codebase_engineering_standards.md
============================================================================
-->

# OSS Launch Audit (Pass 0)

## Overview
This document tracks security findings by crate or subsystem. Each item should
record status, severity, impacted files, and recommended remediation steps.

## decision-gate-broker

### Open Findings

None.

### Closed Findings

1. Unbounded payload sizes in broker sources allowed memory exhaustion.
   - Status: Closed.
   - Severity: High (resource exhaustion / DoS).
   - Impact: `FileSource::fetch` read entire files; `HttpSource::fetch` loaded
     full responses; `InlineSource::fetch` decoded base64 into an unbounded
     `Vec<u8>`.
   - Affected files: `crates/decision-gate-broker/src/source/file.rs`,
     `crates/decision-gate-broker/src/source/http.rs`,
     `crates/decision-gate-broker/src/source/inline.rs`.
   - Resolution: added hard byte caps aligned with payload limits, enforced
     size checks before decoding/reading, and added fail-closed tests.

## decision-gate-cli

### Open Findings

None.

### Closed Findings

1. CLI file reads were unbounded and could exhaust memory on large inputs.
   - Status: Closed.
   - Severity: High (resource exhaustion / DoS).
   - Impact: `runpack export` spec/state reads, `runpack verify` manifest reads,
     and authoring input reads used unbounded `fs::read`/`read_to_string`.
   - Affected files: `crates/decision-gate-cli/src/main.rs`.
   - Resolution: added hard byte limits and fail-closed errors for CLI input
     files aligned with runpack artifact sizing.

2. Runpack manifest name accepted path traversal outside the output directory.
   - Status: Closed.
   - Severity: Medium (path traversal / overwrite risk).
   - Impact: `runpack export --manifest-name ../...` could write manifests
     outside the intended output directory.
   - Affected files: `crates/decision-gate-mcp/src/runpack.rs`,
     `crates/decision-gate-cli/src/main.rs`.
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
   - Affected files: `crates/decision-gate-core/src/runtime/runpack.rs`,
     `crates/decision-gate-core/src/interfaces/mod.rs`.
   - Resolution: added `read_with_limit` enforcement with hard caps and a
     fail-closed verifier test.

2. Evidence and submission payload hashing has no size limits.
   - Status: Closed.
   - Severity: High (resource exhaustion / DoS).
   - Impact: `normalize_evidence_result` and `payload_hash` will hash arbitrary
     byte payloads, allowing untrusted providers or submissions to cause
     unbounded memory and CPU usage.
   - Affected files: `crates/decision-gate-core/src/runtime/engine.rs`.
   - Resolution: added hard byte caps with typed errors and fail-closed tests.

## decision-gate-mcp

### Open Findings

None.

### Closed Findings

1. External MCP provider HTTP calls lacked request timeouts.
   - Status: Closed.
   - Severity: Medium (availability / DoS).
   - Impact: `McpProviderClient` used a blocking HTTP client without timeouts.
   - Affected files: `crates/decision-gate-mcp/src/evidence.rs`,
     `crates/decision-gate-config/src/config.rs`.
   - Resolution: added per-provider HTTP timeouts with bounded validation and
     fail-closed error handling plus system-test coverage.

2. Unbounded MCP stdio payload sizes allowed memory exhaustion.
   - Status: Closed.
   - Severity: High (resource exhaustion / DoS).
   - Impact: stdio transport and external provider framing accepted unbounded
     `Content-Length` values, allowing oversized allocations.
   - Affected files: `crates/decision-gate-mcp/src/server.rs`,
     `crates/decision-gate-mcp/src/evidence.rs`.
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
   - Affected files: `crates/decision-gate-providers/src/http.rs`.
   - Resolution: disabled automatic redirects in the HTTP provider client and
     added a regression test to assert 302 responses are returned directly.

## decision-gate-store-sqlite

### Open Findings

None.

### Closed Findings

1. Unbounded run state blob loads could exhaust memory.
   - Status: Closed.
   - Severity: High (resource exhaustion / DoS).
   - Impact: `SqliteRunStateStore::load_state` read `state_json` blobs without a
     byte limit; a tampered SQLite file could force large allocations.
   - Affected files: `crates/decision-gate-store-sqlite/src/store.rs`.
   - Resolution: added a hard byte cap aligned with runpack artifact limits,
     rejected oversized blobs before loading, and added a fail-closed oversized
     blob test.

## ret-logic

### Open Findings

None.

### Closed Findings

1. Unbounded RON file reads could exhaust memory during requirement loading.
   - Status: Closed.
   - Severity: Medium (resource exhaustion / DoS).
   - Impact: `ron_utils::load_from_file` and `ron_utils::validate_file` used
     unbounded `fs::read_to_string` on attacker-controlled paths.
   - Affected files: `crates/ret-logic/src/serde_support.rs`.
   - Resolution: added `MAX_RON_FILE_BYTES` cap with bounded reads and
     fail-closed errors for oversized files.

2. DSL parsing lacked input size caps and depth checks, enabling stack or memory
   exhaustion with adversarial input.
   - Status: Closed.
   - Severity: High (resource exhaustion / DoS).
   - Impact: recursive descent and token buffering occurred before depth
     validation.
   - Affected files: `crates/ret-logic/src/dsl.rs`.
   - Resolution: enforced maximum input size and nesting depth during parsing
     with typed, fail-closed errors and added regression tests.

3. Plan executor stack depth was lower than validation depth, causing valid
   inputs to fail closed without diagnostics.
   - Status: Closed.
   - Severity: Medium (availability / correctness).
   - Impact: nested plans beyond the executor stack size returned `false`.
   - Affected files: `crates/ret-logic/src/executor.rs`,
     `crates/ret-logic/src/serde_support.rs`.
   - Resolution: increased executor stack depth beyond the default validation
     limit so validated plans no longer underflow the executor stack.

4. ExecutorBuilder leaked dispatch tables by using `Box::leak`.
   - Status: Closed.
   - Severity: Low (resource leak).
   - Impact: repeated `ExecutorBuilder::build` calls could leak memory.
   - Affected files: `crates/ret-logic/src/executor.rs`.
   - Resolution: removed the leak by passing owned dispatch tables into the
     executor builder and tests now use owned tables.
