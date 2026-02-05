<!--
decision-gate-provider-sdk/README.md
============================================================================
Document: Decision Gate Provider SDK Templates
Description: Language templates for MCP evidence providers.
Purpose: Provide starter implementations of the evidence_query protocol.
Dependencies:
  - ../../Docs/configuration/decision-gate.toml.md
  - ./spec/evidence_provider_protocol.md
============================================================================
-->

# Decision Gate Provider SDK

> **Warning: Unstable and Under Active Development**
> This SDK is experimental and may change at any time without notice.
> It is not production-ready. No guarantees are made for correctness,
> security, compatibility, performance, or support. Use at your own risk.

Language templates for building MCP evidence providers that implement the
`evidence_query` tool used by Decision Gate.

## Table of Contents

- [Overview](#overview)
- [Layout](#layout)
- [Protocol](#protocol)
- [Integrating with Decision Gate](#integrating-with-decision-gate)
- [Getting Started](#getting-started)
- [References](#references)

## Overview

External evidence providers are optional: Decision Gate ships built-in providers
(time, env, json, http). Use an external provider when evidence must be queried
from a custom backend (databases, SaaS APIs, internal services).

Each template:

- Implements JSON-RPC 2.0 framing over stdio.
- Exposes `tools/list` and `tools/call` for `evidence_query`.
- Returns `EvidenceResult` objects compatible with Decision Gate contracts.

## Layout

- `spec/` - protocol reference for `evidence_query`.
- `typescript/` - Node/TypeScript stdio provider template.
- `python/` - Python stdio provider template.
- `go/` - Go stdio provider template.

## Protocol

The authoritative protocol definition is in:
`decision-gate-provider-sdk/spec/evidence_provider_protocol.md`.

Providers must:

- Advertise the `evidence_query` tool via `tools/list`.
- Accept `EvidenceQuery` + `EvidenceContext` payloads via `tools/call`.
- Return an `EvidenceResult` with `lane`, `value`, and optional metadata.
- Return structured `EvidenceResult.error` metadata for missing/invalid evidence;
  reserve JSON-RPC errors for malformed requests or unsupported checks.

## Integrating with Decision Gate

Register a provider in `decision-gate.toml`:

```toml
[[providers]]
name = "custom"
type = "mcp"
command = ["python", "provider.py"]
capabilities_path = "contracts/custom_provider.json"
```

HTTP MCP provider example:

```toml
[[providers]]
name = "custom"
type = "mcp"
url = "https://provider.example.com/rpc"
allow_insecure_http = false
auth = { bearer_token = "${PROVIDER_TOKEN}" }
capabilities_path = "contracts/custom_provider.json"
```

See `Docs/configuration/decision-gate.toml.md` for full provider configuration
options and built-in provider configs.

## Getting Started

1. Choose a language template.
2. Replace the `handleEvidenceQuery`/`handle_evidence_query` implementation with
   real provider logic.
3. Generate a provider contract JSON describing checks and params.
4. Register the provider with `decision-gate.toml`.

## References
