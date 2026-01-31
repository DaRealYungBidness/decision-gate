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

## At a Glance

**What:** Build custom evidence providers for Decision Gate
**Why:** Extend evidence sources without modifying core
**Who:** Provider developers, integration engineers
**Prerequisites:** [provider_protocol.md](provider_protocol.md), JSON-RPC 2.0

---

## Provider Architecture

A provider is an MCP server that implements **one tool**: `evidence_query`.

```dg-skip dg-reason="output-only" dg-expires=2026-06-30
Provider
  - Provider Contract (JSON)
    - provider_id, name, description
    - transport = "mcp"
    - config_schema
    - checks (params schema, result schema, comparators, examples)
    - notes
  - MCP Server
    - tools/call -> evidence_query

Decision Gate
  - Loads contract from capabilities_path
  - Validates ScenarioSpec conditions against contract
  - Calls evidence_query during scenario_next
```

Decision Gate does **not** use `tools/list` at runtime. Implement it for MCP compatibility, but keep the contract file authoritative.

---

## Provider Types

| Type | Config | Transport | Use Case |
|------|--------|-----------|----------|
| Built-in | `type = "builtin"` | In-process | `time`, `env`, `json`, `http` |
| External MCP | `type = "mcp"` | stdio or HTTP | Custom providers |

External MCP providers are configured with **either**:
- `command = ["/path/to/provider", "arg"]` (stdio)
- `url = "https://provider/rpc"` (HTTP)

`capabilities_path` is required for all MCP providers.

---

## Quick Start: Minimal Provider

### Step 1: Use an SDK Template

Templates live in `decision-gate-provider-sdk/` (Python, TypeScript, Go). They implement:
- Content-Length framing (stdio)
- `tools/list` and `tools/call`
- JSON-RPC parsing

### Step 2: Implement `evidence_query`

Your handler must return an **EvidenceResult** object (not a JSON-RPC error) for normal failures.

Example (pseudocode):

```python dg-skip dg-reason="pseudocode" dg-expires=2026-06-30
def handle_evidence_query(query, context):
    if query["check_id"] != "file_exists":
        return {
            "value": None,
            "lane": "verified",
            "error": {
                "code": "unsupported_check",
                "message": "unknown check",
                "details": {"check_id": query["check_id"]}
            },
            "evidence_hash": None,
            "evidence_ref": None,
            "evidence_anchor": None,
            "signature": None,
            "content_type": None
        }

    path = query.get("params", {}).get("path")
    if not path:
        return {
            "value": None,
            "lane": "verified",
            "error": {
                "code": "params_missing",
                "message": "missing path",
                "details": {"param": "path"}
            },
            "evidence_hash": None,
            "evidence_ref": None,
            "evidence_anchor": None,
            "signature": None,
            "content_type": None
        }

    exists = os.path.exists(path)
    return {
        "value": {"kind": "json", "value": exists},
        "lane": "verified",
        "error": None,
        "evidence_hash": None,
        "evidence_ref": {"uri": path},
        "evidence_anchor": {
            "anchor_type": "file_path",
            "anchor_value": json.dumps({"path": path}, separators=(",", ":"), sort_keys=True)
        },
        "signature": None,
        "content_type": "application/json"
    }
```

### Step 3: Create a Provider Contract

All fields below are **required** by the contract schema.

```json dg-parse dg-level=fast
{
  "provider_id": "file-provider",
  "name": "File Provider",
  "description": "File existence checks",
  "transport": "mcp",
  "config_schema": {
    "type": "object",
    "additionalProperties": false,
    "properties": {}
  },
  "checks": [
    {
      "check_id": "file_exists",
      "description": "Check if a file exists",
      "determinism": "external",
      "params_required": true,
      "params_schema": {
        "type": "object",
        "additionalProperties": false,
        "properties": { "path": { "type": "string" } },
        "required": ["path"]
      },
      "result_schema": { "type": "boolean" },
      "allowed_comparators": ["equals", "not_equals"],
      "anchor_types": ["file_path"],
      "content_types": ["application/json"],
      "examples": [
        {
          "description": "Check a report file",
          "params": { "path": "/tmp/report.json" },
          "result": true
        }
      ]
    }
  ],
  "notes": [
    "External: depends on the local filesystem state."
  ]
}
```

**Important contract rules:**
- `allowed_comparators` must be **non-empty** and in canonical order.
- `params_required` must match whether `params_schema` requires fields.
- `transport` must be `"mcp"` for external providers.

### Step 4: Configure Decision Gate

```toml dg-parse dg-level=fast
[[providers]]
name = "file-provider"
type = "mcp"
command = ["python3", "/path/to/provider.py"]
capabilities_path = "contracts/file-provider.json"
```

---

## Timeouts

Decision Gate only applies HTTP timeouts to **HTTP MCP** providers:

```toml dg-parse dg-level=fast
[[providers]]
name = "cloud"
type = "mcp"
url = "https://provider.example.com/rpc"
capabilities_path = "contracts/cloud.json"
timeouts = { connect_timeout_ms = 2000, request_timeout_ms = 10000 }
```

There is **no** Decision Gate timeout for stdio MCP providers; keep their handlers fast and deterministic.

---

## Signing Evidence

When `trust.default_policy = { require_signature = { keys = [...] } }`, providers must include signatures:

```json dg-skip dg-reason="non-json-example" dg-expires=2026-06-30
"signature": {
  "scheme": "ed25519",
  "key_id": "/etc/decision-gate/keys/provider.pub",
  "signature": [1, 2, 3, 4]
}
```

**Signing algorithm (exact):**
1. Compute `evidence_hash` from the evidence value.
   - JSON value -> canonical JSON bytes -> sha256
   - Bytes value -> sha256
2. Serialize the **HashDigest object** as canonical JSON.
3. Sign those bytes with Ed25519.

Decision Gate verifies that signature against the configured public key file. If `evidence_hash` is missing, DG computes it before verification.

---

## Error Handling

- Return **EvidenceResult.error** for expected errors.
- Use JSON-RPC errors only for protocol failures (invalid JSON-RPC, internal crashes). DG converts JSON-RPC errors to `provider_error`.

Error codes are provider-defined. Keep them stable and machine-readable (e.g., `params_missing`, `file_not_found`).

---

## Reference

- Provider contract schema: `decision-gate-contract/src/schemas.rs` (`provider_contract_schema`)
- Built-in provider contracts: `Docs/generated/decision-gate/providers.json`
- SDK templates: `decision-gate-provider-sdk/`

---

## Glossary

**EvidenceQuery:** Request `{ provider_id, check_id, params }`.
**EvidenceResult:** Provider response value + metadata.
**Provider Contract:** JSON document describing checks, schemas, comparators, and examples.
**Signature:** Ed25519 signature over canonical JSON of `evidence_hash`.
