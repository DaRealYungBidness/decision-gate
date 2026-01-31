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
Last Updated: 2026-01-30 (UTC)
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
[F:decision-gate-mcp/src/auth.rs L217-L296](decision-gate-mcp/src/auth.rs#L217-L296)[F:decision-gate-mcp/src/tools.rs L1420-L1435](decision-gate-mcp/src/tools.rs#L1420-L1435)[F:decision-gate-mcp/src/tools.rs L1286-L1370](decision-gate-mcp/src/tools.rs#L1286-L1370)[F:decision-gate-mcp/src/server.rs L1341-L1399](decision-gate-mcp/src/server.rs#L1341-L1399)

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
[F:decision-gate-mcp/src/auth.rs L32-L100](decision-gate-mcp/src/auth.rs#L32-L100)[F:decision-gate-mcp/src/server.rs L1158-L1171](decision-gate-mcp/src/server.rs#L1158-L1171)

### Principal Identity
`AuthContext` captures the authentication method plus either an explicit subject
or a bearer token fingerprint. If a local-only request has no subject, the
subject is set to `stdio` or `loopback` based on transport. For bearer tokens, a
sha256 fingerprint is computed and used as a stable identity label.
[F:decision-gate-mcp/src/auth.rs L106-L141](decision-gate-mcp/src/auth.rs#L106-L141)[F:decision-gate-mcp/src/auth.rs L268-L295](decision-gate-mcp/src/auth.rs#L268-L295)[F:decision-gate-mcp/src/auth.rs L418-L433](decision-gate-mcp/src/auth.rs#L418-L433)

---

## Authentication Modes

Auth mode is configured via `server.auth.mode` with supporting allowlists:

- `local_only`: stdio is allowed; HTTP/SSE are only allowed for loopback IPs.
- `bearer_token`: bearer token must match `server.auth.bearer_tokens`.
- `mtls`: subject must be present in `x-decision-gate-client-subject` and match
  `server.auth.mtls_subjects` when configured.

Configuration surface:
- `server.auth.mode`, `bearer_tokens`, `mtls_subjects`, `allowed_tools`.
[F:decision-gate-config/src/config.rs L565-L651](decision-gate-config/src/config.rs#L565-L651)

Implementation details:
- Local-only rejects non-loopback HTTP/SSE.
- Bearer tokens are parsed with size and scheme validation; invalid/missing
  headers fail authentication.
- mTLS requires a subject; unauthorized subjects are rejected.
[F:decision-gate-mcp/src/auth.rs L394-L467](decision-gate-mcp/src/auth.rs#L394-L467)

---

## Tool Authorization (Allowlist)

Tool authorization is enforced per request by `DefaultToolAuthz`. If
`server.auth.allowed_tools` is configured, any tool not in the allowlist is
rejected with an unauthorized error. Invalid tool names in the allowlist are
treated as a fail-closed configuration (empty allowlist).
[F:decision-gate-mcp/src/auth.rs L229-L258](decision-gate-mcp/src/auth.rs#L229-L258)[F:decision-gate-mcp/src/auth.rs L268-L285](decision-gate-mcp/src/auth.rs#L268-L285)

Tool authorization results are emitted by the tool router:
- `AuthAuditEvent::allowed` on success
- `AuthAuditEvent::denied` on failure
[F:decision-gate-mcp/src/tools.rs L1420-L1435](decision-gate-mcp/src/tools.rs#L1420-L1435)[F:decision-gate-mcp/src/auth.rs L302-L360](decision-gate-mcp/src/auth.rs#L302-L360)

---

## Tenant Authorization (Pluggable)

Tenant/namespace authorization is enforced by a pluggable `TenantAuthorizer`
hook. The default implementation allows all access, but enterprise deployments
can supply an authorizer that binds principals to tenant and namespace scopes.
Tenant authorization runs after tool allowlist checks and before tool execution.
Tenant denials emit dedicated audit events (`tenant_authz`).

Implementation references:
- Tenant authz interface: `decision-gate-mcp/src/tenant_authz.rs`
- Enforcement and audit emission: `decision-gate-mcp/src/tools.rs`

---

## Usage Metering and Quotas (Pluggable)

Usage metering and quota checks are enforced by a pluggable `UsageMeter` hook.
The default implementation is a no-op, but enterprise deployments can supply a
meter that enforces quotas and records billing-grade usage. Usage checks run
before tool execution; denials emit `usage_audit` events.

Implementation references:
- Usage metering interface: `decision-gate-mcp/src/usage.rs`
- Enforcement and audit emission: `decision-gate-mcp/src/tools.rs`

---

## Auth Audit Events

Auth decisions emit structured JSON audit events with transport, subject,
method, and failure reason details. The default audit sink logs JSON lines to
stderr; tests can use a no-op sink.
[F:decision-gate-mcp/src/auth.rs L302-L379](decision-gate-mcp/src/auth.rs#L302-L379)

---

## Disclosure Posture (JSON-RPC + HTTP)

### Feedback Disclosure (scenario_next)
`scenario_next` responses are summary-only by default. Optional feedback levels
(`trace`, `evidence`) are gated by `server.feedback.scenario_next` policy, with
role/subject checks resolved from `server.auth.principals`. Evidence feedback is
still filtered through the evidence disclosure policy (raw values may be
redacted unless explicitly allowed).
[F:decision-gate-config/src/config.rs L468-L539](decision-gate-config/src/config.rs#L468-L539)[F:decision-gate-mcp/src/tools.rs L600-L720](decision-gate-mcp/src/tools.rs#L600-L720)

### JSON-RPC Error Envelope
The MCP server responds using JSON-RPC error codes and structured metadata
(`kind`, `retryable`, `request_id`, optional `retry_after_ms`). Error kinds are
stable labels used for metrics and audit categorization.
[F:decision-gate-mcp/src/server.rs L781-L805](decision-gate-mcp/src/server.rs#L781-L805)[F:decision-gate-mcp/src/server.rs L1319-L1399](decision-gate-mcp/src/server.rs#L1319-L1399)

### Error Mapping (Tool Errors)
Tool errors are mapped to HTTP status + JSON-RPC error codes:

| ToolError | HTTP | JSON-RPC Code | Message |
| --- | --- | --- | --- |
| Unauthenticated | 401 | -32001 | unauthenticated |
| Unauthorized | 403 | -32003 | unauthorized |
| InvalidParams | 400 | -32602 | provided message |
| UnknownTool | 400 | -32601 | unknown tool |
| NotFound | 200 | -32004 | provided message |
| Conflict | 200 | -32009 | provided message |
| Evidence | 200 | -32020 | provided message |
| ControlPlane | 200 | -32030 | provided message |
| Runpack | 200 | -32040 | provided message |
| Internal | 200 | -32050 | provided message |
| Serialization | 200 | -32060 | serialization failed |

These mappings are implemented in `jsonrpc_error`.
[F:decision-gate-mcp/src/server.rs L1341-L1364](decision-gate-mcp/src/server.rs#L1341-L1364)

### Auth Challenge Header (RFC 6750)
HTTP/SSE responses for unauthenticated requests include a `WWW-Authenticate`
header with a Bearer realm when bearer token auth is enabled. This aligns with
RFC 6750 and keeps auth challenges explicit without leaking token validation
details.
[F:decision-gate-mcp/src/auth.rs L53-L87](decision-gate-mcp/src/auth.rs#L53-L87)[F:decision-gate-mcp/src/server.rs L1445-L1465](decision-gate-mcp/src/server.rs#L1445-L1465)

### Correlation Headers
HTTP/SSE responses always include a server-issued `x-server-correlation-id`.
If the client supplied a valid `x-correlation-id`, it is echoed back. Invalid
client correlation IDs are rejected before request parsing and are **not**
echoed. The invalid-correlation rejection uses HTTP 400 with JSON-RPC error
code `-32073` (`invalid_correlation_id`).
[F:decision-gate-mcp/src/server.rs L831-L915](decision-gate-mcp/src/server.rs#L831-L915)[F:decision-gate-mcp/src/server.rs L1352-L1435](decision-gate-mcp/src/server.rs#L1352-L1435)

### Request Parsing Failures
Invalid JSON-RPC versions, unknown methods, and malformed request bodies are
rejected with standard JSON-RPC error codes and HTTP 400.
[F:decision-gate-mcp/src/server.rs L866-L1003](decision-gate-mcp/src/server.rs#L866-L1003)[F:decision-gate-mcp/src/server.rs L1128-L1155](decision-gate-mcp/src/server.rs#L1128-L1155)

---

## Rate Limiting and Overload Responses

The server enforces:
- Inflight request limits (reject with 503 and `-32072`).
- Rate limiting (reject with 429 and `-32071`, including retry-after hints).
- Payload size limits (reject with 413 and `-32070`).

These failures are reported with structured JSON-RPC error metadata and are
marked retryable when appropriate.
[F:decision-gate-mcp/src/server.rs L1028-L1126](decision-gate-mcp/src/server.rs#L1028-L1126)[F:decision-gate-mcp/src/server.rs L1392-L1399](decision-gate-mcp/src/server.rs#L1392-L1399)

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
