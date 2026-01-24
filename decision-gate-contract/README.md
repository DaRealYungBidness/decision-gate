<!--
Decision Gate Contract README
============================================================================
Document: decision-gate-contract
Description: Canonical schemas, tooling, and examples for Decision Gate.
Purpose: Single source of truth for MCP tool contracts and JSON schemas.
============================================================================
-->

# decision-gate-contract

## Overview
`decision-gate-contract` defines the canonical JSON schemas and tool contracts
for Decision Gate. It is the source of truth for MCP tooling schemas, provider
capabilities, and example payloads.

## Responsibilities
- Define MCP tool contracts and JSON schema generation.
- Provide canonical examples for SDKs and docs.
- Validate schema conformance in tests.

## Commands
```bash
# Generate contract artifacts
cargo run -p decision-gate-contract -- generate

# Verify artifacts are up to date
cargo run -p decision-gate-contract -- check
```

## Artifacts
- `Docs/generated/decision-gate/tooling.json`
- `Docs/generated/decision-gate/providers.json`
- `Docs/generated/decision-gate/schemas/`
- `Docs/generated/decision-gate/examples/`

## Testing
```bash
cargo test -p decision-gate-contract
```

## References
- Docs/roadmap/trust_lanes_registry_plan.md
- Docs/security/threat_model.md
