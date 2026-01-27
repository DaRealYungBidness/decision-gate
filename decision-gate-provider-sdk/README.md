<!--
decision-gate-provider-sdk/README.md
============================================================================
Document: Decision Gate Provider SDK Templates
Description: Language templates and specs for MCP evidence providers.
Purpose: Provide starter implementations for the Decision Gate evidence protocol.
Dependencies:
  - Docs/roadmap/decision_gate_mcp_roadmap.md
============================================================================
-->

# Decision Gate Provider SDK

## Overview
This folder contains templates for building MCP evidence providers that
implement the `evidence_query` tool used by Decision Gate. Each template
handles JSON-RPC 2.0 framing, tool dispatch, and EvidenceResult responses.
Providers should also publish a capabilities contract (JSON) that Decision Gate
loads via `capabilities_path` in the MCP config.

External providers are optional. Decision Gate ships built-in providers
(time/env/json/http) that cover most local workflows, especially when tools can
emit JSON artifacts for the `json` provider to read.

## Layout
- `spec/` - Protocol reference for the `evidence_query` tool contract.
- `typescript/` - Node/TypeScript template for a stdio MCP provider.
- `python/` - Python template for a stdio MCP provider.
- `go/` - Go template for a stdio MCP provider.

## Getting Started
1. Choose the language template.
2. Replace the `handleEvidenceQuery` implementation with real provider logic.
3. Emit a provider capabilities file describing predicates and params.
4. Run the provider over stdio or wrap it behind an HTTP handler.

For protocol details, see `decision-gate-provider-sdk/spec/evidence_provider_protocol.md`.
