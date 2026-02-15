<!--
Docs/architecture/decision_gate_provider_capability_architecture.md
============================================================================
Document: Decision Gate Provider Integration + Capability Registry Architecture
Description: Current-state reference for provider configuration, capability
             contract loading, and evidence provider federation.
Purpose: Provide an implementation-grade map of how DG integrates providers and
         validates conditions/checks/queries.
Dependencies:
  - crates/decision-gate-config/src/config.rs
  - crates/decision-gate-mcp/src/capabilities.rs
  - crates/decision-gate-mcp/src/evidence.rs
  - crates/decision-gate-mcp/src/tools.rs
============================================================================
Last Updated: 2026-02-15 (UTC)
============================================================================
-->

# Decision Gate Provider Integration + Capability Registry Architecture

> **Audience:** Engineers implementing provider integration or capability
> validation for checks, conditions, and evidence queries.

---

## Table of Contents

1. [Executive Overview](#executive-overview)
2. [Provider Configuration](#provider-configuration)
3. [Capability Registry](#capability-registry)
4. [External Provider Contracts](#external-provider-contracts)
5. [Evidence Provider Federation](#evidence-provider-federation)
6. [Tool-Level Enforcement](#tool-level-enforcement)
7. [File-by-File Cross Reference](#file-by-file-cross-reference)

---

## Executive Overview

Decision Gate supports two provider types:

- **Built-in providers** (compiled into the binary)
- **External MCP providers** (stdio or HTTP transport)

Provider capability contracts are the authoritative schema for check
parameters, results, and allowed comparators. The capability registry validates
scenario specs and evidence queries before evaluation. Evidence federation
routes queries to providers and enforces trust policies.
[F:crates/decision-gate-config/src/config.rs L1883-L1990](crates/decision-gate-config/src/config.rs#L1883-L1990) [F:crates/decision-gate-mcp/src/capabilities.rs L229-L379](crates/decision-gate-mcp/src/capabilities.rs#L229-L379) [F:crates/decision-gate-mcp/src/evidence.rs L138-L210](crates/decision-gate-mcp/src/evidence.rs#L138-L210)

---

## Provider Configuration

Provider configuration is defined in `ProviderConfig`:

- `type`: `builtin` or `mcp`
- `command` / `url`: transport selection for MCP providers
- `capabilities_path`: contract JSON path (required for MCP providers)
- `auth.bearer_token`: optional provider auth
- `trust`: per-provider trust override
- `allow_raw`: opt-in for raw evidence disclosure
- `timeouts`: HTTP connect and request timeouts

Discovery controls are defined in `provider_discovery`:
- `allowlist` / `denylist`: restrict which provider contracts are disclosed
- `max_response_bytes`: cap discovery response size

Validation enforces:
- MCP providers must specify `command` or `url` and `capabilities_path`.
- `allow_insecure_http` is required for `http://` URLs.
- Provider names are unique and trimmed.
- Built-in identifiers (`time`, `env`, `json`, `http`) are reserved; MCP providers cannot use them.
- Built-ins must use a reserved identifier and reject MCP-only fields (`command`, `url`,
  `allow_insecure_http`, `auth`, `capabilities_path`).

[F:crates/decision-gate-config/src/config.rs L1883-L1990](crates/decision-gate-config/src/config.rs#L1883-L1990)

---

## Capability Registry

The capability registry loads provider contracts and compiles JSON schemas for
check params and results. It validates:

- Provider and check existence
- Required params presence
- Params schema conformance
- Expected-value schema conformance
- Comparator allow-lists
- Anchor types declared by provider contracts (e.g., `file_path_rooted` for the
  built-in `json` provider)

[F:crates/decision-gate-mcp/src/capabilities.rs L313-L379](crates/decision-gate-mcp/src/capabilities.rs#L313-L379) [F:crates/decision-gate-mcp/src/capabilities.rs L598-L636](crates/decision-gate-mcp/src/capabilities.rs#L598-L636)

Capability registry queries are used by both scenario definition and evidence
query tools.
[F:crates/decision-gate-mcp/src/tools.rs L2029-L2050](crates/decision-gate-mcp/src/tools.rs#L2029-L2050) [F:crates/decision-gate-mcp/src/tools.rs L979-L1017](crates/decision-gate-mcp/src/tools.rs#L979-L1017)

---

## External Provider Contracts

External providers must supply a contract JSON file that:

- Matches the configured provider id
- Declares `transport = "mcp"`
- Defines checks with allowed comparator lists

Contracts are size-limited and path validated; invalid contracts fail closed.
[F:crates/decision-gate-mcp/src/capabilities.rs L533-L591](crates/decision-gate-mcp/src/capabilities.rs#L533-L591)

---

## Evidence Provider Federation

Evidence federation combines built-in providers and MCP providers:

- Built-ins are registered via the provider registry.
- MCP providers are instantiated with stdio or HTTP transport.
- Provider registry rejects duplicate registrations to prevent silent overrides.
- Stdio provider processes are terminated on drop to avoid orphaned provider
  runtimes during shutdown or test teardown.
- Provider policies (trust + allow_raw) are applied per provider.
- Evidence results may include **structured error metadata** (`code`, `message`,
  `details`) to support deterministic recovery loops.
- HTTP evidence providers enforce timeouts, disallow redirects, apply response
  size limits, fail closed on truncated bodies (Content-Length mismatch), pin
  DNS resolution per request, and deny private/link-local peers by default
  unless explicitly opted in.

[F:crates/decision-gate-mcp/src/evidence.rs L138-L210](crates/decision-gate-mcp/src/evidence.rs#L138-L210) [F:crates/decision-gate-mcp/src/evidence.rs L248-L266](crates/decision-gate-mcp/src/evidence.rs#L248-L266) [F:crates/decision-gate-providers/src/http.rs L82-L239](crates/decision-gate-providers/src/http.rs#L82-L239)

Trust policy enforcement (signature verification) runs per provider response.
[F:crates/decision-gate-mcp/src/evidence.rs L636-L677](crates/decision-gate-mcp/src/evidence.rs#L636-L677)

---

## Tool-Level Enforcement

Tool behavior enforces capability and disclosure policy:

- `scenario_define` validates the spec against capabilities before registering.
- `evidence_query` validates queries and applies raw evidence redaction policy.
- `evidence_query` execution is offloaded to a blocking task to isolate
  blocking providers (HTTP) from the async MCP runtime.
- `provider_contract_get` / `provider_check_schema_get` apply disclosure policy and
  return canonical provider contracts or check schemas.
- Comparator allow-lists are enforced from provider contracts; `json.path`
  exposes the full comparator surface area for deterministic JSON evidence.

[F:crates/decision-gate-mcp/src/tools.rs L2029-L2050](crates/decision-gate-mcp/src/tools.rs#L2029-L2050) [F:crates/decision-gate-mcp/src/tools.rs L979-L1037](crates/decision-gate-mcp/src/tools.rs#L979-L1037) [F:crates/decision-gate-mcp/src/tools.rs L1110-L1150](crates/decision-gate-mcp/src/tools.rs#L1110-L1150) [F:crates/decision-gate-mcp/src/tools.rs L2294-L2334](crates/decision-gate-mcp/src/tools.rs#L2294-L2334)

---

## File-by-File Cross Reference

| Area | File | Notes |
| --- | --- | --- |
| Provider config + validation | `crates/decision-gate-config/src/config.rs` | Provider type, transport, contract path, timeouts, discovery allow/deny. |
| Capability registry | `crates/decision-gate-mcp/src/capabilities.rs` | Contract loading, schema compilation, validation. |
| Evidence federation | `crates/decision-gate-mcp/src/evidence.rs` | Provider registry + trust enforcement. |
| Tool integration | `crates/decision-gate-mcp/src/tools.rs` | Spec/query validation and disclosure policy. |
