<!--
Docs/roadmap/decision_gate_cli_world_class_readiness.md
============================================================================
Document: Decision Gate CLI World-Class Readiness Plan
Description: CLI-focused roadmap to reach FAANG/hyperscaler/DoD readiness.
Purpose: Close all open CLI gaps with security- and audit-grade guarantees.
Dependencies:
  - Docs/generated/decision-gate/tooling.json
  - Docs/roadmap/decision_gate_world_class_onboarding_plan.md
  - Docs/security/threat_model.md
============================================================================
Last Updated: 2026-01-31 (UTC)
============================================================================
-->

# Decision Gate CLI World-Class Readiness Plan

## Executive Intent

Deliver a CLI that is feature-complete relative to the MCP tool surface, secure
by default, deterministic in output, and auditable under hostile review. The
CLI must hold up to FAANG/hyperscaler/DoD scrutiny and adversarial analysis.

This plan focuses only on the CLI experience and adjacent operator workflows.

---

## Scope

In scope:
- The `decision-gate` CLI behavior and operator workflows.
- MCP tool coverage, transport support, auth flows, and output formats.
- CLI integration points for runpacks, config validation, schema registry,
  provider discovery, docs search, and interoperability.
- Security posture and auditability of CLI outputs.

Out of scope:
- Enterprise-only code paths.
- Hosted services or managed control plane features.

---

## Non-Negotiable Invariants

1. **Full MCP parity**: the CLI must expose every tool listed in
   `Docs/generated/decision-gate/tooling.json`.
2. **Deterministic outputs**: identical inputs yield byte-identical CLI outputs.
3. **Fail closed**: invalid inputs, auth failure, or policy violation must not
   proceed.
4. **Secure by default**: network exposure, raw evidence, and non-loopback
   binding require explicit opt-in and config validation.
5. **Explicit transport**: stdio/HTTP/SSE support must be explicit and verifiable.
6. **Auditable artifacts**: CLI must be able to emit signed/hashed outputs and
   include trace metadata (correlation IDs, request IDs, tool names).

---

## Open CLI Points and Closure Requirements

### 1) Full MCP Tool Surface (Parity)

**Open Point:** CLI currently exposes a subset of tools and does not provide a
generic `tools/call` wrapper.

**Closure Requirements:**
- Add a `decision-gate mcp` client command group:
  - `mcp tools list`
  - `mcp tools call --tool <name> --input <json|file>`
  - `mcp resources list`
  - `mcp resources read --uri <resource>`
- Provide one CLI subcommand per tool for typed UX (optional but preferred).
- Validate input schemas using canonical JSON schemas when available.
- Ensure tooling parity with the 18 tools in the contract.

**Acceptance Criteria:**
- `decision-gate mcp tools list` matches the MCP server list.
- Each tool can be called with validated JSON input and deterministic output.
- CLI coverage includes: scenario lifecycle, evidence query, providers list,
  schema registry, precheck, runpack export/verify, and docs search.

---

### 2) Multi-Transport Support (stdio/HTTP/SSE)

**Open Point:** CLI interop uses HTTP only and does not support stdio or SSE.

**Closure Requirements:**
- Transport selection in CLI config or flags:
  - stdio: launch or connect to stdio MCP server.
  - HTTP: JSON-RPC via `POST /rpc`.
  - SSE: JSON-RPC via `POST /rpc` with event-stream responses.
- Enforce max body sizes and timeouts consistently per transport.
- Support local-only policy gating for network transports.

**Acceptance Criteria:**
- End-to-end tool call succeeds on all transports.
- Timeouts, size limits, and error decoding are identical across transports.

---

### 3) Auth and Identity Support

**Open Point:** CLI supports bearer token and a client subject header for interop
only, not full auth surface across all commands.

**Closure Requirements:**
- Global auth flags and config integration for CLI client commands:
  - Bearer token
  - mTLS client subject header (proxy mode)
- Ensure auth headers are never logged unless explicitly requested.
- Provide `--auth-profile` for safe reuse of named auth settings in config.

**Acceptance Criteria:**
- Auth behavior matches MCP server auth modes.
- CLI never prints or logs credentials by default.

---

### 4) Schema Registry CLI

**Open Point:** MCP exposes schema registry tools; CLI has no support.

**Closure Requirements:**
- `schema register`, `schema list`, `schema get` CLI commands.
- JSON schema validation before registration where possible.
- Support for schema signatures if enabled in config.

**Acceptance Criteria:**
- CLI can register, list, and fetch schemas with deterministic output.
- ACLs and namespace constraints are enforced consistently with MCP.

---

### 5) Providers List + Contract Inspection

**Open Point:** CLI supports `provider_contract_get` and `provider_check_schema_get`,
but not `providers_list`.

**Closure Requirements:**
- Add `provider list` to enumerate providers and their contract metadata.
- Provide a summary view vs full JSON.

**Acceptance Criteria:**
- CLI matches `providers_list` output from MCP.
- CLI includes deterministic ordering and stable formatting.

---

### 6) Docs Search and Resources

**Open Point:** MCP provides `decision_gate_docs_search` plus resources;
CLI does not expose them.

**Closure Requirements:**
- `docs search` and `docs read` CLI commands.
- Support query and max section count; default to safe caps.

**Acceptance Criteria:**
- Output mirrors MCP docs search results.
- Empty query yields deterministic overview output.

---

### 7) Runpack Storage Integration

**Open Point:** MCP includes runpack storage hooks; CLI only writes local files.

**Closure Requirements:**
- CLI support for object-store-backed runpack export and verification, when
  configured.
- Explicit opt-in for remote storage sinks.

**Acceptance Criteria:**
- CLI can export runpack to local or configured storage and report the storage
  URI deterministically.
- Verification supports reading from the same storage backend.

---

### 8) Store Administration (SQLite)

**Open Point:** SQLite store is available but no CLI to inspect or validate.

**Closure Requirements:**
- `store list-runs`, `store get-run`, `store export-run`, `store verify`.
- Optional `store prune` respecting `max_versions` semantics.

**Acceptance Criteria:**
- CLI can list and fetch run state without violating size limits.
- Hash verification matches core deterministic rules.

---

### 9) Broker Test Utilities

**Open Point:** Disclosure broker sources/sinks exist but no CLI for testing.

**Closure Requirements:**
- `broker resolve` to fetch and validate `ContentRef` payloads.
- `broker dispatch` to send packets to a sink.

**Acceptance Criteria:**
- CLI can resolve file/http/inline sources with strict size limits.
- Dispatch uses deterministic envelopes and explicit targets.

---

### 10) Contract + SDK Generation Entry Points

**Open Point:** Contract and SDK generators are separate binaries; no single
operator entrypoint.

**Closure Requirements:**
- `decision-gate contract generate/check` wrappers.
- `decision-gate sdk generate/check` wrappers.

**Acceptance Criteria:**
- CLI invokes the generators and validates outputs in-place.
- Exit codes are stable for CI gating.

---

### 11) CLI UX and Output Standards

**Open Point:** Output formats are inconsistent across commands.

**Closure Requirements:**
- Default to canonical JSON for machine consumption.
- `--format` flags for `json`, `markdown`, `text` where appropriate.
- Always include correlation metadata when available.

**Acceptance Criteria:**
- Commands produce deterministic output order and formatting.
- Structured output is stable across versions for CI parsing.

---

### 12) Hardening and Security Posture

**Open Point:** CLI lacks a unified security posture doc for operator use.

**Closure Requirements:**
- Ensure all commands enforce size limits and safe file handling.
- Explicit guardrails for non-loopback networking.
- Redaction of raw evidence by default.
- No implicit environment variable expansion for secrets unless requested.

**Acceptance Criteria:**
- CLI fails closed on invalid or oversized inputs.
- Security warnings shown on network exposure.

---

## Quality Gates

### Functional
- Full MCP tool parity and resource coverage.
- Multi-transport support with equivalent behavior.
- Schema registry and provider listing support.

### Security
- All network actions require explicit opt-in and auth where configured.
- Credentials never logged by default.
- All input size limits enforced with clear error messages.

### Determinism
- Canonical JSON output for all machine-readable responses.
- Stable ordering for lists and transcripts.

### Auditability
- Correlation and request metadata preserved in outputs.
- Optional JSON transcript output for all tool calls.

---

## Testing and Validation Plan

1. **CLI conformance tests**: verify every tool command against a local MCP
   server with deterministic fixtures.
2. **Transport matrix**: stdio/HTTP/SSE parity tests.
3. **Auth matrix**: local-only, bearer token, mTLS proxy subject.
4. **Size-limit tests**: verify hard caps on input/output across commands.
5. **Golden outputs**: snapshot tests for canonical JSON output.

---

## Release Readiness Checklist (CLI)

- [ ] All 18 tools exposed and validated.
- [ ] Multi-transport client modes supported.
- [ ] Auth profiles and safe defaults enforced.
- [ ] Schema registry CLI complete.
- [ ] Provider list and contract views complete.
- [ ] Docs search CLI complete.
- [ ] Runpack storage integration complete.
- [ ] Store admin CLI complete.
- [ ] Broker test CLI complete.
- [ ] Contract/SDK generators integrated.
- [ ] Output formats deterministic + documented.
- [ ] Security posture validated in tests.

---

## Decision Gate

We consider CLI readiness **green** only when every checklist item above is
implemented, tested, and verified in CI. Any exception must be documented and
explicitly approved in a launch readiness review.
