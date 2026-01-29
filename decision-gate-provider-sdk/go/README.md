<!--
decision-gate-provider-sdk/go/README.md
============================================================================
Document: Go Evidence Provider Template
Description: Go MCP provider template for Decision Gate.
Purpose: Provide a minimal stdio JSON-RPC 2.0 provider implementation.
Dependencies:
  - ../spec/evidence_provider_protocol.md
============================================================================
-->

# Go Provider Template

Minimal Go MCP provider that implements `tools/list` and `tools/call` for
`evidence_query` over stdio.

## Table of Contents

- [Overview](#overview)
- [Files](#files)
- [Run](#run)
- [Tests](#tests)
- [Customization](#customization)
- [Framing Limits](#framing-limits)
- [References](#references)

## Overview

This template uses Content-Length framing and replies with JSON-RPC 2.0
responses. Replace `handleEvidenceQuery` with provider-specific logic and keep
`tools/list` aligned with your contract.

## Files

- `main.go` - JSON-RPC framing + tool handlers.

## Run

```bash
go run .
```

## Tests

```bash
go test ./...
```

## Customization

1. Define predicates and parameters in `handleEvidenceQuery`.
2. Populate `tools/list` with the `evidence_query` tool metadata.
3. Generate a provider contract JSON and register it in `decision-gate.toml`.

## Framing Limits

The template enforces:
- Maximum header size: 8 KiB
- Maximum body size: 1 MiB

Requests exceeding these limits are rejected with JSON-RPC errors.

## References
