<!--
Decision Gate SQLite Store README
============================================================================
Document: decision-gate-store-sqlite
Description: SQLite-backed run state store and schema registry.
Purpose: Durable persistence for Decision Gate core.
============================================================================
-->

# decision-gate-store-sqlite

## Overview
This crate provides a SQLite-backed implementation of Decision Gate's
`RunStateStore` and `DataShapeRegistry` interfaces. It enforces size limits,
validates hashes, and supports deterministic replay.

## Capabilities
- Durable run state storage with versioning.
- SQLite-backed data shape registry (versioned schemas).
- Strict hash validation on load.
- Path validation and size limits for safety.

## Configuration
Used via `decision-gate-mcp` config:
- `run_state_store.type = "sqlite"` with `path`.
- `schema_registry.type = "sqlite"` with `path`.

## Testing
```bash
cargo test -p decision-gate-store-sqlite
```

## References
- Docs/security/threat_model.md
- decision-gate-core/README.md
