<!--
decision-gate-provider-sdk/typescript/README.md
============================================================================
Document: TypeScript Evidence Provider Template
Description: Node/TypeScript MCP provider template for Decision Gate.
Purpose: Provide a minimal stdio JSON-RPC 2.0 provider implementation.
Dependencies:
  - decision-gate-provider-sdk/spec/evidence_provider_protocol.md
============================================================================
-->

# TypeScript Provider Template

## Overview
This template implements a stdio MCP server that handles `tools/list` and
`tools/call` for the `evidence_query` tool. Replace the stubbed
`handleEvidenceQuery` logic with real evidence access.

## Build and Run
```bash
npm install
npm run build
node dist/index.js
```

## Notes
- This template uses Content-Length framing over stdio.
- Frames larger than 1 MiB or headers over 8 KiB are rejected.
- Return JSON-RPC errors for unsupported predicates.
- Publish a capabilities JSON file and keep `tools/list` aligned with it.
