<!--
decision-gate-provider-sdk/python/README.md
============================================================================
Document: Python Evidence Provider Template
Description: Python MCP provider template for Decision Gate.
Purpose: Provide a minimal stdio JSON-RPC 2.0 provider implementation.
Dependencies:
  - ../spec/evidence_provider_protocol.md
============================================================================
-->

# Python Provider Template

Minimal Python MCP provider that implements `tools/list` and `tools/call` for
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

This template uses Content-Length framing and responds with JSON-RPC 2.0
messages. Replace `handle_evidence_query` with provider-specific logic and
keep the advertised tool metadata aligned with your contract.

## Files

- `provider.py` - JSON-RPC framing + tool handlers.

## Run

```bash
python3 provider.py
```

## Tests

```bash
python3 -m unittest test_provider.py
```

## Customization

1. Define predicates and parameters in `handle_evidence_query`.
2. Populate `tools/list` with the `evidence_query` tool metadata.
3. Generate a provider contract JSON and register it in `decision-gate.toml`.

## Framing Limits

The template enforces:
- Maximum header size: 8 KiB
- Maximum body size: 1 MiB

Requests exceeding these limits are rejected with JSON-RPC errors.

## References
