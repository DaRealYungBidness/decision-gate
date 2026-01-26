<!--
Docs/guides/security_guide.md
============================================================================
Document: Decision Gate Security Guide
Description: Security posture and trust policy guidance.
Purpose: Explain evidence trust, disclosure policy, and local-only constraints.
Dependencies:
  - Docs/security/threat_model.md
  - decision-gate-mcp configuration
============================================================================
-->

# Security Guide

## Overview
Decision Gate is built for hostile inputs and fails closed on missing or
invalid evidence. This guide summarizes the trust and disclosure controls
exposed by the MCP layer.

## Trust Policies
`decision-gate-mcp` enforces provider trust policies:

- `audit` (default): accept evidence without signature verification.
- `require_signature`: verify Ed25519 signatures against configured keys.

When signature enforcement is enabled, unsigned or untrusted evidence is
rejected and the gate remains held.

## Server Mode and Namespace Policy
Decision Gate defaults to strict mode and explicit namespace allowlists:

- `server.mode = "strict"` (default): only verified evidence is accepted.
- `dev.permissive = true`: asserted evidence is allowed for dev use only.
  Startup emits warnings so operators can detect non-production posture.
  `server.mode = "dev_permissive"` remains as a legacy alias.

The literal `default` namespace is **never** implicitly allowed. Enable it with:

- `namespace.allow_default = true`
- `namespace.default_tenants = ["tenant-1", ...]`

Dev-permissive does **not** override namespace authority and is disallowed when
`namespace.authority.mode = "assetcore_http"`.

Use strict mode for production and high-assurance environments. Use
dev-permissive only for local development, tests, or controlled sandboxes.

## Precheck Audit Logging
Precheck requests handle asserted payloads and are audited hash-only by default.
Each precheck emits canonical JSON hashes for the request and response. Raw
payload logging is disabled unless explicitly enabled via
`server.audit.log_precheck_payloads = true`.

## Evidence Disclosure Policy
`evidence_query` is a debug surface and is denied by default for raw values.

Configuration controls:
- `evidence.allow_raw_values = false` blocks raw values globally.
- `evidence.require_provider_opt_in = true` requires providers to opt in.
- Provider config `allow_raw = true` allows raw results for that provider.

## Dispatch Policy Engines
Dispatch authorization is enforced by a swappable policy engine configured in
`decision-gate.toml` under `[policy]`. The default engine is `permit_all` for
ease of adoption, but production deployments should enable a real policy:

- `policy.engine = "static"` enables deterministic rule-based authorization.
- Rule effects are `permit`, `deny`, or `error` (fail closed).

Policies apply to dispatch targets, packet visibility labels, policy tags, and
schema/packet identifiers. Use deny-by-default (`static.default = "deny"`) for
high-assurance deployments.

## Provider Timeouts
External MCP providers called over HTTP are guarded by strict connect and
request timeouts. Overrides are supported per provider but are bounded to
prevent disabling safeguards. Timeouts are treated as missing evidence and
fail closed.

## MCP Tool Auth
Inbound MCP tool calls enforce authn/authz. The default mode is local-only:
stdio and loopback HTTP/SSE are permitted, while non-loopback binds require an
explicit auth policy. Configure `server.auth` to enable bearer-token or mTLS
subject enforcement, and optionally restrict calls with a tool allowlist.
Auth decisions are logged as structured JSON events on stderr.

## Schema Registry ACL
Schema registry operations (`schemas_register`, `schemas_list`, `schemas_get`)
are protected by `schema_registry.acl`. The default mode is built-in, deny-by-
default behavior based on roles provided by `server.auth.principals`.
Custom ACL rules can be configured for finer-grained allow/deny logic.

## Runpack Integrity
Runpacks are hashed using RFC 8785 canonical JSON and verified offline with
SHA-256 digests. Any missing or tampered artifacts fail verification.

## Run State Store (SQLite)
SQLite-backed run state storage treats the database as untrusted input. The
store verifies hashes on every load and fails closed on corruption or version
mismatches. In production deployments:

- Restrict filesystem permissions on the database and WAL files.
- Back up the `.db`, `-wal`, and `-shm` files together.
- Keep the storage path on a durable volume; do not use temp directories.

See `Docs/security/threat_model.md` for the full threat model.
