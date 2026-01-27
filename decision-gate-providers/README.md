<!--
Decision Gate Providers README
============================================================================
Document: decision-gate-providers
Description: Built-in evidence providers for Decision Gate.
Purpose: Provide deterministic, auditable predicates for common sources.
============================================================================
-->

# decision-gate-providers

## Overview
This crate implements the built-in evidence providers used by Decision Gate.
Providers answer `evidence_query` calls and return `EvidenceResult` values with
hashes and optional metadata for audit.

These providers are intentionally narrow, deterministic, and safe-by-default.
They do not execute arbitrary code.

## Built-in Providers
- `time`: trigger-time predicates (e.g., `after`, `before`) based on the trigger
  timestamp supplied to the run.
- `env`: environment variable queries.
- `json`: JSON/YAML file extraction and comparisons (ideal for tool artifacts).
- `http`: HTTP checks (status, body, headers) with strict limits.

The `json` provider is the primary bridge for local workflows: any tool that
emits JSON can be gated without adding new providers.

## Security Notes
- Providers must validate inputs and fail closed.
- HTTP and file providers must enforce size limits and path safety.

## Testing
```bash
cargo test -p decision-gate-providers
```

## References
- Docs/security/threat_model.md
- decision-gate-core/README.md
