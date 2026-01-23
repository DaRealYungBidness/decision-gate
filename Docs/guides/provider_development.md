<!--
Docs/guides/provider_development.md
============================================================================
Document: Provider Development Guide
Description: Guidance for building MCP evidence providers.
Purpose: Help developers implement `evidence_query` for Decision Gate.
Dependencies:
  - decision-gate-provider-sdk/spec/evidence_provider_protocol.md
============================================================================
-->

# Provider Development Guide

## Overview
Evidence providers are standalone MCP servers that implement the `evidence_query`
tool. Decision Gate federates providers via JSON-RPC 2.0 over stdio or HTTP.

## Protocol Contract
Providers must follow the MCP evidence provider protocol. See
`Docs/guides/provider_protocol.md` for the wire-level request/response contract
and schema references. The canonical SDK reference lives at
`decision-gate-provider-sdk/spec/evidence_provider_protocol.md`.

## Start from a Template
Use the provider SDK templates:
- `decision-gate-provider-sdk/typescript`
- `decision-gate-provider-sdk/python`
- `decision-gate-provider-sdk/go`

Each template includes `tools/list`, `tools/call`, and Content-Length framing.

## Implement Evidence Queries
Your provider should:
1. Validate the incoming `query` and `context`.
2. Fetch or compute evidence deterministically.
3. Return an `EvidenceResult` with `value` and optional anchors or references.

Example response shape:

```json
{
  "value": { "kind": "json", "value": true },
  "evidence_hash": null,
  "evidence_ref": null,
  "evidence_anchor": { "anchor_type": "receipt_id", "anchor_value": "abc" },
  "signature": null,
  "content_type": "application/json"
}
```

Decision Gate recomputes `evidence_hash` from `value` when present, so providers
may omit it. Always set `content_type` and return `value` whenever possible so
hashing and audit logs stay complete. The MCP layer may redact `value` for
`evidence_query` responses based on disclosure policy; this is handled by
Decision Gate, not the provider.

## Protocol Payloads at a Glance
The provider receives a `tools/call` payload with:

- `query`: `provider_id`, `predicate`, and optional `params`.
- `context`: tenant/run/scenario/stage identifiers plus trigger metadata.

The provider returns a JSON-RPC response containing a JSON `EvidenceResult`:

- `value`: JSON or bytes payload (optional).
- `evidence_hash`: hash of the value (optional).
- `evidence_anchor`: anchor metadata (optional).
- `evidence_ref`: external URI reference (optional).
- `signature`: signature metadata (optional).
- `content_type`: MIME type for the evidence payload.

Use the provider protocol doc for full JSON examples and field constraints.

## Error Handling
Return JSON-RPC errors for:
- Unknown predicates
- Missing parameters
- Upstream fetch failures

Decision Gate treats errors as missing evidence and fails closed.

## Capability Contracts (Required)
Decision Gate requires every MCP provider to ship a capability contract that
declares:
- Provider metadata (`provider_id`, description, transport)
- Predicate names and descriptions
- JSON Schemas for `params` and predicate results
- Determinism classification and allowed comparator allow-lists
- Anchor types and content types emitted by the predicate
- Example predicate payloads

The contract is loaded from `capabilities_path` in `decision-gate.toml` and is
validated before any scenario or evidence query is accepted.

Example configuration:
```toml
[[providers]]
name = "mongo"
type = "mcp"
command = ["mongo-provider", "--stdio"]
capabilities_path = "contracts/mongo_provider.json"
```

Use `Docs/generated/decision-gate/providers.json` as a reference for the
canonical contract shape and predicate schema patterns.

## Predicate Schema Guidance
Provide precise JSON schemas for both params and results:

- Params schemas should set `additionalProperties: false` and declare required fields.
- Result schemas should reflect actual value types returned in EvidenceResult.
- Allowed comparators should be minimal and intentional for the data shape.

Include at least one example per predicate (params + result) to help authors
build correct ScenarioSpec predicates.

For scenario authors, see `Docs/guides/predicate_authoring.md`.

## Trust and Signatures
If your provider signs evidence:
- Populate `signature.scheme`, `signature.key_id`, and `signature.signature`.
- Configure Decision Gate trust policy to require the signing key.

See `Docs/guides/security_guide.md` for trust policy details.
