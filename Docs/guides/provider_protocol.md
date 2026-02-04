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

## How Decision Gate Calls Providers

- Decision Gate calls **one tool**: `evidence_query`.
- Calls are always `tools/call` with `name = "evidence_query"`.
- Decision Gate **does not** rely on `tools/list` at runtime; provider capabilities are loaded from `capabilities_path` in config.

Providers should still implement `tools/list` for MCP compatibility and SDK templates, but Decision Gate does not depend on it.

---

## External Provider Configuration

External providers use `type = "mcp"` and must declare `capabilities_path`:

```toml dg-parse dg-level=fast
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
Provider names must be unique; built-in identifiers (`time`, `env`, `json`, `http`)
are reserved and cannot be used by MCP providers.

---

## Tool Call: evidence_query

### Request (JSON-RPC)

```json dg-parse dg-level=fast
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "evidence_query",
    "arguments": {
      "query": {
        "provider_id": "file-provider",
        "check_id": "file_size",
        "params": { "path": "report.json" }
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

```json dg-parse dg-level=fast
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
          "evidence_ref": { "uri": "dg+file://evidence-root/report.json" },
          "evidence_anchor": {
            "anchor_type": "file_path_rooted",
            "anchor_value": "{\"path\":\"report.json\",\"root_id\":\"evidence-root\",\"size\":1024}"
          },
          "signature": null,
          "content_type": "application/json"
        }
      }
    ]
  }
}
```

**Important:** `evidence_anchor.anchor_value` is a **string**. If you need structured anchor data, encode it as canonical JSON and store the JSON string. For `file_path_rooted`, include scalar `root_id` and `path` fields.

---

## EvidenceQuery Structure

```json dg-skip dg-reason="non-json-example" dg-expires=2026-06-30
{
  "provider_id": "string",
  "check_id": "string",
  "params": "any"  // optional
}
```

- `provider_id` matches the provider name in config.
- `check_id` is the provider's capability name (not the scenario condition ID).
- `params` is provider-specific; it may be omitted or `null`.

---

## EvidenceContext Structure

```json dg-parse dg-level=fast
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

## EvidenceResult Structure

```json dg-skip dg-reason="non-json-example" dg-expires=2026-06-30
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
- If `evidence_hash` is present, it must match the canonical hash of `value` or the response is rejected.

---

## Error Handling

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

**EvidenceQuery:** Provider request `{ provider_id, check_id, params }`.
**EvidenceResult:** Provider response (value + metadata).
**MCP:** Model Context Protocol (JSON-RPC 2.0 tool calls).
**Provider Contract:** JSON file declaring checks, params schema, result schema, and allowed comparators.
