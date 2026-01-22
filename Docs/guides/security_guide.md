<!--
Docs/guides/security_guide.md
============================================================================
Document: Decision Gate Security Guide
Description: Security posture and trust policy guidance.
Purpose: Explain evidence trust, disclosure policy, and local-only constraints.
Dependencies:
  - Docs/security/threat_model.md
  - decision-gate-mcp configuration
============================================================================
-->

# Security Guide

## Overview
Decision Gate is built for hostile inputs and fails closed on missing or
invalid evidence. This guide summarizes the trust and disclosure controls
exposed by the MCP layer.

## Trust Policies
`decision-gate-mcp` enforces provider trust policies:

- `audit` (default): accept evidence without signature verification.
- `require_signature`: verify Ed25519 signatures against configured keys.

When signature enforcement is enabled, unsigned or untrusted evidence is
rejected and the gate remains held.

## Evidence Disclosure Policy
`evidence_query` is a debug surface and is denied by default for raw values.

Configuration controls:
- `evidence.allow_raw_values = false` blocks raw values globally.
- `evidence.require_provider_opt_in = true` requires providers to opt in.
- Provider config `allow_raw = true` allows raw results for that provider.

## Local-Only Transport
The MCP server currently runs in local-only mode without a full auth/policy
layer. HTTP/SSE transports are restricted to loopback addresses and the CLI
emits warnings on startup.

## Runpack Integrity
Runpacks are hashed using RFC 8785 canonical JSON and verified offline with
SHA-256 digests. Any missing or tampered artifacts fail verification.

See `Docs/security/threat_model.md` for the full threat model.

