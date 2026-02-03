<!--
Docs/architecture/decision_gate_provider_capability_architecture.md
============================================================================
Document: Decision Gate Provider Integration + Capability Registry Architecture
Description: Current-state reference for provider configuration, capability
             contract loading, and evidence provider federation.
Purpose: Provide an implementation-grade map of how DG integrates providers and
         validates conditions/checks/queries.
Dependencies:
  - decision-gate-config/src/config.rs
  - decision-gate-mcp/src/capabilities.rs
  - decision-gate-mcp/src/evidence.rs
  - decision-gate-mcp/src/tools.rs
============================================================================
Last Updated: 2026-02-03 (UTC)
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
[F:decision-gate-config/src/config.rs L1860-L1990](decision-gate-config/src/config.rs#L1860-L1990)[F:decision-gate-mcp/src/capabilities.rs L216-L369](decision-gate-mcp/src/capabilities.rs#L216-L369)[F:decision-gate-mcp/src/evidence.rs L137-L209](decision-gate-mcp/src/evidence.rs#L137-L209)

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

[F:decision-gate-config/src/config.rs L1860-L1990](decision-gate-config/src/config.rs#L1860-L1990)

---

## Capability Registry

The capability registry loads provider contracts and compiles JSON schemas for
check params and results. It validates:

- Provider and check existence
- Required params presence
- Params schema conformance
- Expected-value schema conformance
- Comparator allow-lists

[F:decision-gate-mcp/src/capabilities.rs L216-L309](decision-gate-mcp/src/capabilities.rs#L216-L309)[F:decision-gate-mcp/src/capabilities.rs L508-L520](decision-gate-mcp/src/capabilities.rs#L508-L520)

Capability registry queries are used by both scenario definition and evidence
query tools.
[F:decision-gate-mcp/src/tools.rs L717-L720](decision-gate-mcp/src/tools.rs#L717-L720)[F:decision-gate-mcp/src/tools.rs L869-L870](decision-gate-mcp/src/tools.rs#L869-L870)

---

## External Provider Contracts

External providers must supply a contract JSON file that:

- Matches the configured provider id
- Declares `transport = "mcp"`
- Defines checks with allowed comparator lists

Contracts are size-limited and path validated; invalid contracts fail closed.
[F:decision-gate-mcp/src/capabilities.rs L392-L451](decision-gate-mcp/src/capabilities.rs#L392-L451)[F:decision-gate-mcp/src/capabilities.rs L457-L487](decision-gate-mcp/src/capabilities.rs#L457-L487)

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
  size limits, and fail closed on truncated bodies (Content-Length mismatch).

[F:decision-gate-mcp/src/evidence.rs L137-L209](decision-gate-mcp/src/evidence.rs#L137-L209)[F:decision-gate-mcp/src/evidence.rs L220-L244](decision-gate-mcp/src/evidence.rs#L220-L244)[F:decision-gate-providers/src/http.rs L90-L266](decision-gate-providers/src/http.rs#L90-L266)

Trust policy enforcement (signature verification) runs per provider response.
[F:decision-gate-mcp/src/evidence.rs L639-L689](decision-gate-mcp/src/evidence.rs#L639-L689)

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

[F:decision-gate-mcp/src/tools.rs L694-L885](decision-gate-mcp/src/tools.rs#L694-L885)

---

## File-by-File Cross Reference

| Area | File | Notes |
| --- | --- | --- |
| Provider config + validation | `decision-gate-config/src/config.rs` | Provider type, transport, contract path, timeouts, discovery allow/deny. |
| Capability registry | `decision-gate-mcp/src/capabilities.rs` | Contract loading, schema compilation, validation. |
| Evidence federation | `decision-gate-mcp/src/evidence.rs` | Provider registry + trust enforcement. |
| Tool integration | `decision-gate-mcp/src/tools.rs` | Spec/query validation and disclosure policy. |
