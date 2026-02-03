<!--
Docs/roadmap/decision_gate_cli_world_class_readiness.md
============================================================================
Document: Decision Gate CLI High-Bar Readiness Plan
Description: CLI-focused roadmap to reach strict-environment readiness.
Purpose: Close all open CLI gaps with security- and audit-grade guarantees.
Dependencies:
  - Docs/generated/decision-gate/tooling.json
  - Docs/security/threat_model.md
============================================================================
Last Updated: 2026-02-03 (UTC)
============================================================================
-->

# Decision Gate CLI High-Bar Readiness Plan

## Executive Intent

Deliver a CLI that is feature-complete relative to the MCP tool surface, secure
by default, deterministic in output, and auditable under hostile review. The
CLI must hold up to strict-environment scrutiny and adversarial analysis.

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

## Status Summary (as of 2026-02-03)

**Implemented:**
- Full MCP tool surface (generic `mcp tools` + typed tool wrappers).
- Multi-transport MCP client (stdio/HTTP/SSE) with size limits and timeouts.
- Auth flags + auth profiles (bearer token + client subject header).
- Schema registry CLI (`schema register/list/get`).
- Provider discovery CLI (`provider list/contract/check-schema`).
- Docs search + resources (`docs search/list/read`).
- Runpack storage integration (`runpack export --storage`, `runpack verify --storage`).
- Store administration CLI (`store list/get/export/verify/prune`).
- Broker utilities CLI (`broker resolve/dispatch`).
- Contract + SDK generator wrappers.
- i18n readiness (`--lang` + `DECISION_GATE_LANG`, en/ca catalogs, parity tests).

**Partial:**
- Output format standardization (canonical JSON is common, but not universal).
- CLI security posture is mostly enforced, but not uniformly documented or
  enforced at the command level (e.g., evidence redaction is MCP-driven).
- Auditable artifacts are available via runpacks; CLI output signing is now
  supported for store/broker outputs, but not yet across every command.

**Missing:**
- CLI security posture documentation (command-by-command hardening notes).

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

## Open CLI Points and Closure Requirements (with Status)

### 1) Full MCP Tool Surface (Parity)

**Status:** Implemented.

**Evidence (current files):**
- `decision-gate-cli/src/main.rs` (MCP commands + typed tool wrappers)
- `decision-gate-core/src/tooling.rs` (18 tools list)

**Notes:**
- Generic MCP client commands (`mcp tools list/call`, `mcp resources list/read`) exist.
- Typed tool wrappers exist for all 18 tools.
- Input schema validation is wired through contract schemas.

---

### 2) Multi-Transport Support (stdio/HTTP/SSE)

**Status:** Implemented.

**Evidence (current files):**
- `decision-gate-cli/src/mcp_client.rs`
- `decision-gate-cli/src/main.rs`

**Notes:**
- Transport selection is explicit via CLI flags.
- Size limits and timeouts are enforced in the client.
- Server-side non-loopback gating exists in `serve` (see `serve_policy.rs`).

---

### 3) Auth and Identity Support

**Status:** Implemented.

**Evidence (current files):**
- `decision-gate-cli/src/main.rs` (auth flags + auth profile loading)
- `decision-gate-cli/src/mcp_client.rs` (headers)

**Notes:**
- Bearer token and client subject headers are supported.
- Auth profiles are loaded from config and tokens are redacted in debug output.

---

### 4) Schema Registry CLI

**Status:** Implemented.

**Evidence (current files):**
- `decision-gate-cli/src/main.rs` (`schema register/list/get`)

---

### 5) Providers List + Contract Inspection

**Status:** Implemented.

**Evidence (current files):**
- `decision-gate-cli/src/main.rs` (`provider list/contract/check-schema`)

---

### 6) Docs Search and Resources

**Status:** Implemented.

**Evidence (current files):**
- `decision-gate-cli/src/main.rs` (`docs search/list/read`)

---

### 7) Runpack Storage Integration

**Status:** Implemented.

**Current behavior:**
- CLI runpack export/verify use filesystem artifacts by default and can
  optionally upload/read from object storage with `--storage`.

**Evidence (current files):**
- `decision-gate-cli/src/main.rs` (`ObjectStoreRunpackBackend`, `--storage`).

---

### 8) Store Administration (SQLite)

**Status:** Implemented.

**Current behavior:**
- `store list/get/export/verify/prune` is available for SQLite stores.

---

### 9) Broker Test Utilities

**Status:** Implemented.

**Current behavior:**
- `broker resolve/dispatch` is available for local testing.

---

### 10) Contract + SDK Generation Entry Points

**Status:** Implemented.

**Evidence (current files):**
- `decision-gate-cli/src/main.rs` (`contract generate/check`, `sdk generate/check`)

---

### 11) CLI UX and Output Standards

**Status:** Partial.

**Current behavior:**
- Canonical JSON output is used for many commands, but not all.
- Several commands emit text output only (e.g., `config validate`, `runpack export`).
- `--format` is not consistently available across commands.

---

### 12) Hardening and Security Posture

**Status:** Partial.

**Current behavior:**
- Non-loopback bind guardrails exist for `serve`.
- Size limits are enforced for file reads and MCP responses.
- Evidence redaction is MCP-driven; CLI does not add extra redaction layers.
- No dedicated CLI security posture doc exists yet.

---

### 13) Internationalization (i18n) Readiness

**Status:** Implemented.

**Evidence (current files):**
- `decision-gate-cli/src/i18n.rs` (en/ca catalogs)
- `decision-gate-cli/src/tests/i18n.rs` (catalog parity tests)
- `decision-gate-cli/src/main.rs` (disclaimer + language selection)

---

## Quality Gates (Current Status)

### Functional
- Full MCP tool parity and resource coverage. **Met**
- Multi-transport support with equivalent behavior. **Met**
- Schema registry and provider listing support. **Met**

### Security
- Network actions require explicit opt-in and auth where configured. **Partial**
- Credentials never logged by default. **Met**
- All input size limits enforced with clear error messages. **Met**

### Determinism
- Canonical JSON output for all machine-readable responses. **Partial**
- Stable ordering for lists and transcripts. **Met**

### Internationalization
- Locale key parity enforced across all catalogs. **Met**
- Non-English disclaimer present and deterministic. **Met**

### Auditability
- Correlation and request metadata preserved in outputs. **Partial**
- Optional JSON transcript output for all tool calls. **Partial**
- Signed/hashed CLI output artifacts. **Partial** (store/broker outputs only)

---

## Testing and Validation Plan (Current Status)

1. **CLI conformance tests**: present for MCP tool coverage. **Met**
2. **Transport matrix**: stdio/HTTP/SSE parity tests exist. **Met**
3. **Auth matrix**: CLI auth coverage exists. **Met**
4. **Size-limit tests**: CLI limits are tested. **Met**
5. **Golden outputs**: CLI golden outputs present. **Met**
6. **i18n parity tests**: en/ca parity tests exist. **Met**

---

## Release Readiness Checklist (CLI)

- [x] All 18 tools exposed and validated.
- [x] Multi-transport client modes supported.
- [x] Auth profiles and safe defaults enforced.
- [x] Schema registry CLI complete.
- [x] Provider list and contract views complete.
- [x] Docs search CLI complete.
- [x] Runpack storage integration complete.
- [x] Store admin CLI complete.
- [x] Broker test CLI complete.
- [x] Contract/SDK generators integrated.
- [~] Output formats deterministic + documented.
- [~] Security posture validated in tests.
- [x] i18n parity tests green (en + ca).
- [x] Non-English disclaimer enabled and verified.
- [~] Signed/hashed CLI output artifacts (store/broker outputs only).

---

## Decision Gate

We consider CLI readiness **green** only when every checklist item above is
implemented, tested, and verified in CI. Any exception must be documented and
explicitly approved in a launch readiness review.
