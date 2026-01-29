<!--
decision-gate-provider-sdk/typescript/README.md
============================================================================
Document: TypeScript Evidence Provider Template
Description: Node/TypeScript MCP provider template for Decision Gate.
Purpose: Provide a minimal stdio JSON-RPC 2.0 provider implementation.
Dependencies:
  - ../spec/evidence_provider_protocol.md
============================================================================
-->

# TypeScript Provider Template

Minimal Node/TypeScript MCP provider that implements `tools/list` and
`tools/call` for `evidence_query` over stdio.

## Table of Contents

- [Overview](#overview)
- [Files](#files)
- [Build and Run](#build-and-run)
- [Tests](#tests)
- [Customization](#customization)
- [Framing Limits](#framing-limits)
- [References](#references)

## Overview

This template implements Content-Length framing over stdio and responds with
JSON-RPC 2.0 envelopes. Replace the stubbed `handleEvidenceQuery` with your
provider logic and keep `tools/list` aligned with your contract JSON.

## Files

- `src/index.ts` - JSON-RPC framing + tool handlers.
- `package.json` - build script (`tsc`).
- `tsconfig.json` - TypeScript compiler config.

## Build and Run

```bash
npm install
npm run build
node dist/index.js
```

## Tests

```bash
npm test
```

## Customization

1. Define predicates and parameters in `handleEvidenceQuery`.
2. Populate `tools/list` with the `evidence_query` tool metadata.
3. Generate a provider contract JSON (capabilities) and register it in
   `decision-gate.toml` via `capabilities_path`.

## Framing Limits

The template enforces:
- Maximum header size: 8 KiB
- Maximum body size: 1 MiB

Requests exceeding these limits are rejected with JSON-RPC errors.

## References
