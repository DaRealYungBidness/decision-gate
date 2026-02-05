<!--
Docs/security/audits/OSS_launch_2.md
============================================================================
Document: OSS Launch Audit Findings (Pass 2)
Description: Track security and test gaps for OSS launch readiness.
Purpose: Record per-crate follow-ups that exceed a single pass.
Dependencies:
  - Docs/security/threat_model.md
  - Docs/security/audits/OSS_launch_0.md
============================================================================
-->

# OSS Launch Audit (Pass 2)

## decision-gate-core

### Open Findings

None. System-test gaps: none identified in this pass.

### Closed Findings

None.

## decision-gate-contract

### Open Findings

None.

### Closed Findings

1. Authoring normalization accepts unbounded JSON/RON inputs without explicit
   size or depth limits, which can enable resource exhaustion during CLI-driven
   validation and normalization.
   - Status: Closed (2026-02-01).
   - Severity: Medium (resource exhaustion / DoS risk).
   - Resolution: Added explicit authoring size and depth limits with canonical
     JSON size enforcement in `decision-gate-contract` normalization.

2. No system-test coverage for the `decision-gate-contract` CLI `generate` and
   `check` workflows (artifact emission + verification in a clean workspace).
   - Status: Closed (2026-02-01).
   - Severity: Low (coverage gap for tooling entry points).
   - Resolution: Added `contract_cli_generate_and_check` system-test to run the
     contract CLI in a temp workspace and assert fail-closed drift detection.

## decision-gate-sdk-gen

### Open Findings

None.

### Closed Findings

1. No system-test coverage for the `decision-gate-sdk-gen` CLI `generate` and
   `check` workflows (artifact emission + verification in a clean workspace).
   - Status: Closed (2026-02-01).
   - Severity: Low (coverage gap for tooling entry points).
   - Resolution: Added `sdk_gen_cli_generate_and_check` system-test exercising
     generate/check workflows and drift detection.

## decision-gate-broker

### Open Findings

None.

### Closed Findings

1. Missing system-test coverage for broker integration paths.
   - Status: Closed (2026-02-01).
   - Severity: Medium (coverage gap for dispatch entry points).
   - Resolution: Added `broker_composite_sources_and_sinks` system-test to
     validate CompositeBroker wiring for file/http/inline sources and sinks.

2. `HttpSource` lacks host allowlist / IP range guards for SSRF-sensitive environments.
   - Status: Closed (2026-02-01).
   - Severity: Medium (SSRF / data exfiltration risk).
   - Resolution: Added host allowlist/denylist policy with private/link-local
     IP guards; default policy denies private ranges unless explicitly allowed.

3. `FileSource` root protection is vulnerable to symlink/TOCTOU races.
   - Status: Closed (2026-02-01).
   - Severity: Medium (path traversal under concurrent write access).
   - Resolution: Enforced root constraints at open time using `openat` +
     `O_NOFOLLOW` on Unix and symlink rejection on other platforms.

## decision-gate-mcp

### Open Findings

None.

### Closed Findings

1. Missing system-test coverage for `max_body_bytes` enforcement on HTTP/SSE transports.
   - Status: Closed (2026-02-01).
   - Severity: Low (coverage gap for DoS guardrails).
   - Resolution: Added `http_payload_too_large_rejected` and `sse_payload_too_large_rejected`
     system-tests asserting oversized requests are rejected with `413 Payload Too Large` prior to
     JSON-RPC parsing, with transcripts captured under the operations suite.

## decision-gate-provider-sdk

### Open Findings

None.

### Closed Findings

1. No system-test coverage for provider SDK templates (Go, Python, TypeScript) to validate
   stdio MCP framing and `evidence_query` responses against the Decision Gate control plane.
   - Status: Closed (2026-02-01).
   - Severity: Low (coverage gap for SDK integration).
   - Resolution: Added `provider_templates` system-test suite covering Go/Python/TypeScript
     templates with MCP framing and fail-closed behavior checks.
