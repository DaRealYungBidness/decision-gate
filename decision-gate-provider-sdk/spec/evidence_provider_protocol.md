<!--
decision-gate-provider-sdk/spec/evidence_provider_protocol.md
============================================================================
Document: Evidence Provider Protocol
Description: JSON-RPC contract for Decision Gate MCP evidence providers.
Purpose: Define the `evidence_query` tool surface and expected payloads.
Dependencies:
  - decision-gate-core evidence schemas
  - decision-gate-mcp evidence federation
============================================================================
-->

# Evidence Provider Protocol (MCP)

## Overview
Decision Gate evidence providers expose a single MCP tool named `evidence_query`.
The Decision Gate MCP server calls this tool with an `EvidenceQuery` and
`EvidenceContext`, and expects an `EvidenceResult` inside the MCP tool result.

Providers can run over stdio (Content-Length framing) or HTTP (JSON-RPC 2.0).

## Tool List
Providers must advertise `evidence_query` via `tools/list`:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "tools": [
      {
        "name": "evidence_query",
        "description": "Resolve a Decision Gate evidence query.",
        "input_schema": { "type": "object" }
      }
    ]
  }
}
```

## Tool Call Request
Decision Gate calls the provider with `tools/call`:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "evidence_query",
    "arguments": {
      "query": { "provider_id": "env", "predicate": "get", "params": { "key": "DEPLOY_ENV" } },
      "context": {
        "tenant_id": "tenant-1",
        "run_id": "run-1",
        "scenario_id": "scenario-1",
        "stage_id": "stage-1",
        "trigger_id": "trigger-1",
        "trigger_time": { "kind": "unix_millis", "value": 1710000000000 },
        "correlation_id": null
      }
    }
  }
}
```

## Tool Call Response
The response must include a `content` array with a JSON EvidenceResult:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "content": [
      {
        "type": "json",
        "json": {
          "value": { "kind": "json", "value": "production" },
          "evidence_hash": null,
          "evidence_ref": null,
          "evidence_anchor": null,
          "signature": null,
          "content_type": "application/json"
        }
      }
    ]
  }
}
```

## EvidenceQuery Schema

```json
{
  "provider_id": "string",
  "predicate": "string",
  "params": { "any": "json" }
}
```

## EvidenceContext Schema

```json
{
  "tenant_id": "string",
  "run_id": "string",
  "scenario_id": "string",
  "stage_id": "string",
  "trigger_id": "string",
  "trigger_time": { "kind": "unix_millis|logical", "value": 0 },
  "correlation_id": "string|null"
}
```

## EvidenceResult Schema

```json
{
  "value": { "kind": "json|bytes", "value": "any" },
  "lane": "verified|asserted",
  "error": { "code": "string", "message": "string", "details": "object|null" },
  "evidence_hash": { "algorithm": "sha256", "value": "hex" },
  "evidence_ref": { "uri": "string" },
  "evidence_anchor": { "anchor_type": "string", "anchor_value": "string" },
  "signature": { "scheme": "string", "key_id": "string", "signature": [0] },
  "content_type": "string"
}
```

### Notes
- `value.kind = "bytes"` encodes `value` as a JSON array of 0-255 integers.
- `evidence_hash` is optional; Decision Gate can compute it if `value` is present.
- Use JSON-RPC errors for unsupported predicates or malformed requests.
- When evidence is invalid or missing, set `value = null` and include structured
  `error` metadata. This keeps evaluation fail-closed while enabling recovery.
