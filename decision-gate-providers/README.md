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

## Built-in Providers
- `time`: monotonic time predicates (e.g., `after`, `before`).
- `env`: environment variable queries.
- `json`: JSON file extraction and comparisons.
- `http`: HTTP checks (status, body, headers) with strict limits.

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
