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

## Error Handling
Return JSON-RPC errors for:
- Unknown predicates
- Missing parameters
- Upstream fetch failures

Decision Gate treats errors as missing evidence and fails closed.

## Trust and Signatures
If your provider signs evidence:
- Populate `signature.scheme`, `signature.key_id`, and `signature.signature`.
- Configure Decision Gate trust policy to require the signing key.

See `Docs/guides/security_guide.md` for trust policy details.

