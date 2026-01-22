<!--
decision-gate-provider-sdk/go/README.md
============================================================================
Document: Go Evidence Provider Template
Description: Go MCP provider template for Decision Gate.
Purpose: Provide a minimal stdio JSON-RPC 2.0 provider implementation.
Dependencies:
  - decision-gate-provider-sdk/spec/evidence_provider_protocol.md
============================================================================
-->

# Go Provider Template

## Overview
This template implements a stdio MCP server that supports `tools/list` and
`tools/call` for `evidence_query`. Replace `handleEvidenceQuery` with real
provider logic.

## Run
```bash
go run .
```

## Notes
- Uses Content-Length framing over stdio.
- Returns JSON-RPC errors for invalid requests.

