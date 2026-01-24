<!--
Decision Gate Broker README
============================================================================
Document: decision-gate-broker
Description: Reference sources/sinks and composite dispatcher.
Purpose: Resolve payloads and dispatch disclosures for Decision Gate.
============================================================================
-->

# decision-gate-broker

## Overview
`decision-gate-broker` provides reference implementations for payload sources
and sinks plus a composite broker that wires them together. It is a utility
layer for disclosure dispatch and payload resolution.

## Capabilities
- Sources: inline, file, HTTP.
- Sinks: channel, callback, log.
- Composite broker with structured error handling.

## Security Notes
- Enforces size limits and rejects unsafe paths.
- HTTP source validates responses and size bounds.

## Testing
```bash
cargo test -p decision-gate-broker
```

## References
- decision-gate-core/README.md
- Docs/security/threat_model.md
