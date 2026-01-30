<!--
Docs/guides/provider_protocol.md
============================================================================
Document: Evidence Provider Protocol
Description: JSON-RPC contract for Decision Gate MCP evidence providers.
Purpose: Define the `evidence_query` tool surface and expected payloads.
Dependencies:
  - decision-gate-provider-sdk/spec/evidence_provider_protocol.md
============================================================================
-->

# Evidence Provider Protocol

## At a Glance

**What:** JSON-RPC 2.0 protocol for external evidence providers (MCP)
**Why:** Let Decision Gate call custom evidence sources without core changes
**Who:** Provider developers, integration engineers, MCP implementors
**Prerequisites:** JSON-RPC 2.0 and [evidence_flow_and_execution_model.md](evidence_flow_and_execution_model.md)

---

## How Decision Gate Calls Providers (Exact)

- Decision Gate calls **one tool**: `evidence_query`.
- Calls are always `tools/call` with `name = "evidence_query"`.
- Decision Gate **does not** rely on `tools/list` at runtime; provider capabilities are loaded from `capabilities_path` in config.

Providers should still implement `tools/list` for MCP compatibility and SDK templates, but Decision Gate does not depend on it.

---

## External Provider Configuration (Exact)

External providers use `type = "mcp"` and must declare `capabilities_path`:

```toml
[[providers]]
name = "git"
type = "mcp"
# stdio transport
command = ["/usr/local/bin/git-provider", "--repo", "/repo"]
capabilities_path = "contracts/git.json"

[[providers]]
name = "cloud"
type = "mcp"
# HTTP transport
url = "https://evidence.example.com/rpc"
allow_insecure_http = false
capabilities_path = "contracts/cloud.json"
# Optional auth + timeouts
# auth = { bearer_token = "YOUR_TOKEN" }
# timeouts = { connect_timeout_ms = 2000, request_timeout_ms = 10000 }
```

Built-in providers use `type = "builtin"` and are **not** MCP servers.

---

## Tool Call: evidence_query

### Request (JSON-RPC)

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "evidence_query",
    "arguments": {
      "query": {
        "provider_id": "file-provider",
        "predicate": "file_size",
        "params": { "path": "/tmp/report.json" }
      },
      "context": {
        "tenant_id": 1,
        "namespace_id": 1,
        "run_id": "run-123",
        "scenario_id": "ci-gate",
        "stage_id": "main",
        "trigger_id": "commit-abc",
        "trigger_time": { "kind": "unix_millis", "value": 1710000000000 },
        "correlation_id": null
      }
    }
  }
}
```

### Response (JSON-RPC)

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "content": [
      {
        "type": "json",
        "json": {
          "value": { "kind": "json", "value": 1024 },
          "lane": "verified",
          "error": null,
          "evidence_hash": null,
          "evidence_ref": { "uri": "/tmp/report.json" },
          "evidence_anchor": {
            "anchor_type": "file_path",
            "anchor_value": "{\"path\":\"/tmp/report.json\",\"size\":1024}"
          },
          "signature": null,
          "content_type": "application/json"
        }
      }
    ]
  }
}
```

**Important:** `evidence_anchor.anchor_value` is a **string**. If you need structured anchor data, encode it as canonical JSON and store the JSON string.

---

## EvidenceQuery (Exact Structure)

```json
{
  "provider_id": "string",
  "predicate": "string",
  "params": "any"  // optional
}
```

- `provider_id` matches the provider name in config.
- `predicate` is the provider's capability name (not the scenario predicate ID).
- `params` is provider-specific; it may be omitted or `null`.

---

## EvidenceContext (Exact Structure)

```json
{
  "tenant_id": 1,
  "namespace_id": 1,
  "run_id": "run-123",
  "scenario_id": "ci-gate",
  "stage_id": "main",
  "trigger_id": "commit-abc",
  "trigger_time": { "kind": "unix_millis", "value": 1710000000000 },
  "correlation_id": null
}
```

The context is provided by Decision Gate and is available to providers for deterministic queries (e.g., point-in-time checks).

---

## EvidenceResult (Exact Structure)

```json
{
  "value": { "kind": "json|bytes", "value": "any" } | null,
  "lane": "verified|asserted",
  "error": { "code": "string", "message": "string", "details": "object|null" } | null,
  "evidence_hash": { "algorithm": "sha256", "value": "hex" } | null,
  "evidence_ref": { "uri": "string" } | null,
  "evidence_anchor": { "anchor_type": "string", "anchor_value": "string" } | null,
  "signature": { "scheme": "ed25519", "key_id": "string", "signature": [0, 1, 2] } | null,
  "content_type": "string" | null
}
```

Notes:
- `value.kind = "bytes"` uses a JSON array of integers `0..255`.
- `signature.signature` is a JSON array of bytes (the `Vec<u8>` serialization).
- If `evidence_hash` is missing, Decision Gate computes it from `value` before verification.

---

## Error Handling (Exact)

Providers should return an **EvidenceResult** with `error` set for expected failures (missing files, invalid params, etc.).

If the provider returns a **JSON-RPC error**, Decision Gate treats it as a provider failure and surfaces it as `code = "provider_error"` on its side.

---

## Transport Notes

### stdio
- Decision Gate spawns the provider using `command = [..]`.
- Messages are framed with `Content-Length` headers.

### HTTP
- Decision Gate sends a JSON-RPC `POST` to the configured `url`.
- Optional bearer token can be configured via `auth.bearer_token`.

---

## Glossary

**EvidenceQuery:** Provider request `{ provider_id, predicate, params }`.
**EvidenceResult:** Provider response (value + metadata).
**MCP:** Model Context Protocol (JSON-RPC 2.0 tool calls).
**Provider Contract:** JSON file declaring predicates, params schema, result schema, and allowed comparators.
