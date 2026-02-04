<!--
Docs/architecture/decision_gate_auth_disclosure_architecture.md
============================================================================
Document: Decision Gate Authn/Authz + Disclosure Architecture
Description: Current-state reference for MCP authentication, tool authorization,
             audit emission, and error disclosure posture.
Purpose: Provide an implementation-grade map of how DG authenticates callers,
         authorizes tool access, and maps failures to JSON-RPC/HTTP responses.
Dependencies:
  - decision-gate-mcp/src/auth.rs
  - decision-gate-mcp/src/tools.rs
  - decision-gate-mcp/src/server.rs
  - decision-gate-config/src/config.rs
============================================================================
Last Updated: 2026-02-04 (UTC)
============================================================================
-->

# Decision Gate Authn/Authz + Disclosure Architecture

> **Audience:** Engineers implementing or reviewing MCP authentication,
> authorization, and error disclosure behavior.

---

## Table of Contents

1. [Executive Overview](#executive-overview)
2. [Request Context and Identity](#request-context-and-identity)
3. [Authentication Modes](#authentication-modes)
4. [Tool Authorization (Allowlist)](#tool-authorization-allowlist)
5. [Tenant Authorization (Pluggable)](#tenant-authorization-pluggable)
6. [Usage Metering and Quotas (Pluggable)](#usage-metering-and-quotas-pluggable)
7. [Auth Audit Events](#auth-audit-events)
8. [Disclosure Posture (JSON-RPC + HTTP)](#disclosure-posture-json-rpc--http)
9. [Rate Limiting and Overload Responses](#rate-limiting-and-overload-responses)
10. [File-by-File Cross Reference](#file-by-file-cross-reference)

---

## Executive Overview

Decision Gate MCP enforces strict, fail-closed authentication and authorization
for tool calls. Authentication is transport-aware (stdio, HTTP, SSE) and
configured via `server.auth`. Authorization is enforced per tool call via
`DefaultToolAuthz`, with optional tool allowlists. A separate, pluggable tenant
authorization layer can enforce tenant/namespace scoping before tool execution.
Auth decisions emit structured audit events, and request failures are mapped to
stable JSON-RPC error codes and HTTP status codes for deterministic disclosure
and metrics labeling.
[F:decision-gate-mcp/src/auth.rs L293-L372](decision-gate-mcp/src/auth.rs#L293-L372) [F:decision-gate-mcp/src/tools.rs L1436-L1454](decision-gate-mcp/src/tools.rs#L1436-L1454) [F:decision-gate-mcp/src/tools.rs L2857-L2878](decision-gate-mcp/src/tools.rs#L2857-L2878) [F:decision-gate-mcp/src/server.rs L1984-L2017](decision-gate-mcp/src/server.rs#L1984-L2017)

---

## Request Context and Identity

### Request Context
Incoming requests are normalized into a `RequestContext` that records transport,
peer IP, auth header, client subject, and an optional request id plus correlation
metadata. For HTTP/SSE transports, the context is built from the
`Authorization` header and the `x-decision-gate-client-subject` header for mTLS
proxy identity. Client-provided correlation identifiers arrive via
`x-correlation-id` and are treated as **unsafe input**: they are strictly
validated and rejected if invalid. The server always issues its own
`x-server-correlation-id` and returns it on responses, providing a stable,
auditable identifier even when client IDs are missing or rejected.
[F:decision-gate-mcp/src/auth.rs L82-L173](decision-gate-mcp/src/auth.rs#L82-L173) [F:decision-gate-mcp/src/server.rs L993-L1072](decision-gate-mcp/src/server.rs#L993-L1072) [F:decision-gate-mcp/src/server.rs L1648-L1734](decision-gate-mcp/src/server.rs#L1648-L1734)

### Principal Identity
`AuthContext` captures the authentication method plus either an explicit subject
or a bearer token fingerprint. If a local-only request has no subject, the
subject is set to `stdio` or `loopback` based on transport. For bearer tokens, a
sha256 fingerprint is computed and used as a stable identity label.
[F:decision-gate-mcp/src/auth.rs L181-L216](decision-gate-mcp/src/auth.rs#L181-L216) [F:decision-gate-mcp/src/auth.rs L503-L517](decision-gate-mcp/src/auth.rs#L503-L517)

---

## Authentication Modes

Auth mode is configured via `server.auth.mode` with supporting allowlists:

- `local_only`: stdio is allowed; HTTP/SSE are only allowed for loopback IPs.
- `bearer_token`: bearer token must match `server.auth.bearer_tokens`.
- `mtls`: subject must be present in `x-decision-gate-client-subject` and match
  `server.auth.mtls_subjects` when configured.

Configuration surface:
- `server.auth.mode`, `bearer_tokens`, `mtls_subjects`, `allowed_tools`.
[F:decision-gate-config/src/config.rs L789-L937](decision-gate-config/src/config.rs#L789-L937)

Implementation details:
- Local-only rejects non-loopback HTTP/SSE.
- Bearer tokens are parsed with size and scheme validation; invalid/missing
  headers fail authentication.
- mTLS requires a subject; unauthorized subjects are rejected.
[F:decision-gate-mcp/src/auth.rs L479-L552](decision-gate-mcp/src/auth.rs#L479-L552)

---

## Tool Authorization (Allowlist)

Tool authorization is enforced per request by `DefaultToolAuthz`. If
`server.auth.allowed_tools` is configured, any tool not in the allowlist is
rejected with an unauthorized error. Invalid tool names in the allowlist are
treated as a fail-closed configuration (empty allowlist).
[F:decision-gate-mcp/src/auth.rs L293-L372](decision-gate-mcp/src/auth.rs#L293-L372)

Tool authorization results are emitted by the tool router:
- `AuthAuditEvent::allowed` on success
- `AuthAuditEvent::denied` on failure
[F:decision-gate-mcp/src/tools.rs L3131-L3144](decision-gate-mcp/src/tools.rs#L3131-L3144) [F:decision-gate-mcp/src/auth.rs L379-L445](decision-gate-mcp/src/auth.rs#L379-L445)

---

## Tenant Authorization (Pluggable)

Tenant/namespace authorization is enforced by a pluggable `TenantAuthorizer`
hook. The default implementation allows all access, but enterprise deployments
can supply an authorizer that binds principals to tenant and namespace scopes.
Tenant authorization runs after tool allowlist checks and before tool execution.
Tenant denials emit dedicated audit events (`tenant_authz`).

Implementation references:
- Tenant authz interface: [F:decision-gate-mcp/src/tenant_authz.rs L29-L65](decision-gate-mcp/src/tenant_authz.rs#L29-L65)
- Enforcement and audit emission: [F:decision-gate-mcp/src/tools.rs L2857-L2935](decision-gate-mcp/src/tools.rs#L2857-L2935)

---

## Usage Metering and Quotas (Pluggable)

Usage metering and quota checks are enforced by a pluggable `UsageMeter` hook.
The default implementation is a no-op, but enterprise deployments can supply a
meter that enforces quotas and records billing-grade usage. Usage checks run
before tool execution; denials emit `usage_audit` events.

Implementation references:
- Usage metering interface: [F:decision-gate-mcp/src/usage.rs L28-L105](decision-gate-mcp/src/usage.rs#L28-L105)
- Enforcement and audit emission: [F:decision-gate-mcp/src/tools.rs L2937-L2999](decision-gate-mcp/src/tools.rs#L2937-L2999)

---

## Auth Audit Events

Auth decisions emit structured JSON audit events with transport, subject,
method, and failure reason details. The default audit sink logs JSON lines to
stderr; tests can use a no-op sink.
[F:decision-gate-mcp/src/auth.rs L379-L445](decision-gate-mcp/src/auth.rs#L379-L445)

---

## Disclosure Posture (JSON-RPC + HTTP)

### Feedback Disclosure (scenario_next)
`scenario_next` responses are summary-only by default. Optional feedback levels
(`trace`, `evidence`) are gated by `server.feedback.scenario_next` policy, with
role/subject checks resolved from `server.auth.principals`. Evidence feedback is
still filtered through the evidence disclosure policy (raw values may be
redacted unless explicitly allowed).
[F:decision-gate-config/src/config.rs L452-L545](decision-gate-config/src/config.rs#L452-L545) [F:decision-gate-mcp/src/tools.rs L2144-L2257](decision-gate-mcp/src/tools.rs#L2144-L2257)

### JSON-RPC Error Envelope
The MCP server responds using JSON-RPC error codes and structured metadata
(`kind`, `retryable`, `request_id`, optional `retry_after_ms`). Error kinds are
stable labels used for metrics and audit categorization.
[F:decision-gate-mcp/src/server.rs L1961-L2043](decision-gate-mcp/src/server.rs#L1961-L2043)

### Error Mapping (Tool Errors)
Tool errors are mapped to HTTP status + JSON-RPC error codes:

| ToolError | HTTP | JSON-RPC Code | Message |
| --- | --- | --- | --- |
| Unauthenticated | 401 | -32001 | unauthenticated |
| Unauthorized | 403 | -32003 | unauthorized |
| InvalidParams | 400 | -32602 | provided message |
| CapabilityViolation | 400 | -32602 | `code: message` |
| UnknownTool | 400 | -32601 | unknown tool |
| ResponseTooLarge | 200 | -32070 | provided message |
| RateLimited | 200 | -32071 | provided message |
| NotFound | 200 | -32004 | provided message |
| Conflict | 200 | -32009 | provided message |
| Evidence | 200 | -32020 | provided message |
| ControlPlane | 200 | -32030 | provided message |
| Runpack | 200 | -32040 | provided message |
| Internal | 200 | -32050 | provided message |
| Serialization | 200 | -32060 | serialization failed |

These mappings are implemented in `jsonrpc_error`.
[F:decision-gate-mcp/src/server.rs L1984-L2015](decision-gate-mcp/src/server.rs#L1984-L2015)

### Auth Challenge Header (RFC 6750)
HTTP/SSE responses for unauthenticated requests include a `WWW-Authenticate`
header with a Bearer realm when bearer token auth is enabled. This aligns with
RFC 6750 and keeps auth challenges explicit without leaking token validation
details.
[F:decision-gate-mcp/src/auth.rs L46-L75](decision-gate-mcp/src/auth.rs#L46-L75) [F:decision-gate-mcp/src/server.rs L1706-L1718](decision-gate-mcp/src/server.rs#L1706-L1718)

### Correlation Headers
HTTP/SSE responses always include a server-issued `x-server-correlation-id`.
If the client supplied a valid `x-correlation-id`, it is echoed back. Invalid
client correlation IDs are rejected before request parsing and are **not**
echoed. The invalid-correlation rejection uses HTTP 400 with JSON-RPC error
code `-32073` (`invalid_correlation_id`).
[F:decision-gate-mcp/src/server.rs L993-L1072](decision-gate-mcp/src/server.rs#L993-L1072) [F:decision-gate-mcp/src/server.rs L1648-L1749](decision-gate-mcp/src/server.rs#L1648-L1749)

### Request Parsing Failures
Invalid JSON-RPC versions, unknown methods, and malformed request bodies are
rejected with standard JSON-RPC error codes and HTTP 400.
[F:decision-gate-mcp/src/server.rs L1505-L1583](decision-gate-mcp/src/server.rs#L1505-L1583)

---

## Rate Limiting and Overload Responses

The server enforces:
- Inflight request limits (reject with 503 and `-32072`).
- Rate limiting (reject with 429 and `-32071`, including retry-after hints).
- Payload size limits (reject with 413 and `-32070`).

These failures are reported with structured JSON-RPC error metadata and are
marked retryable when appropriate.
[F:decision-gate-mcp/src/server.rs L1505-L1568](decision-gate-mcp/src/server.rs#L1505-L1568) [F:decision-gate-mcp/src/server.rs L2051-L2053](decision-gate-mcp/src/server.rs#L2051-L2053)

---

## File-by-File Cross Reference

| Area | File | Notes |
| --- | --- | --- |
| Auth config surface | `decision-gate-config/src/config.rs` | Auth modes, token/subject allowlists, tool allowlist. |
| Auth policy engine | `decision-gate-mcp/src/auth.rs` | DefaultToolAuthz, auth modes, audit events, token parsing. |
| Tool auth integration | `decision-gate-mcp/src/tools.rs` | Per-call authorization + audit emission. |
| Tenant authz interface | `decision-gate-mcp/src/tenant_authz.rs` | Pluggable tenant/namespace authorization seam. |
| Usage metering interface | `decision-gate-mcp/src/usage.rs` | Pluggable usage metering + quota enforcement seam. |
| JSON-RPC disclosure | `decision-gate-mcp/src/server.rs` | Error mapping and response codes. |
