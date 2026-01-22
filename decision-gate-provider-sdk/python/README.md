<!--
decision-gate-provider-sdk/python/README.md
============================================================================
Document: Python Evidence Provider Template
Description: Python MCP provider template for Decision Gate.
Purpose: Provide a minimal stdio JSON-RPC 2.0 provider implementation.
Dependencies:
  - decision-gate-provider-sdk/spec/evidence_provider_protocol.md
============================================================================
-->

# Python Provider Template

## Overview
This template implements a stdio MCP server that supports `tools/list` and
`tools/call` for `evidence_query`. Replace `handle_evidence_query` with
real provider logic.

## Run
```bash
python provider.py
```

## Notes
- Uses Content-Length framing over stdio.
- Returns JSON-RPC errors for invalid requests.

