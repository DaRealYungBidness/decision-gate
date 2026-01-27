<!--
Decision Gate CLI README
============================================================================
Document: decision-gate-cli
Description: CLI for MCP server and runpack workflows.
Purpose: Provide operational entry points for Decision Gate.
============================================================================
-->

# decision-gate-cli

## Overview
`decision-gate-cli` is the command-line entry point for running the MCP server
and managing runpacks and authoring utilities for Decision Gate's deterministic
checkpoint and requirement-evaluation workflows.

The CLI runs the MCP server with built-in providers enabled by default
(time/env/json/http). For local workflows, a common pattern is: run a tool,
emit a JSON artifact, and gate it with the `json` provider—no external MCP
provider required.

## AssetCore Integration
DG integrates with AssetCore via explicit interop workflows. The canonical
integration hub lives at `Docs/integrations/assetcore/`, with implementation
details in `Docs/guides/assetcore_interop_runbook.md`.

## Commands
- `serve`: run the MCP server (stdio, HTTP, or SSE).
- `runpack export`: export a runpack for a run.
- `runpack verify`: verify a runpack manifest and artifacts.
- `authoring validate`: validate scenario specs.
- `authoring normalize`: normalize scenario specs.
- `config validate`: validate Decision Gate config.

## Examples
```bash
# Start MCP server with config
cargo run -p decision-gate-cli -- serve --config decision-gate.toml

# Export a runpack
cargo run -p decision-gate-cli -- runpack export --config decision-gate.toml \
  --scenario-id scenario-1 --run-id run-1 --out ./runpack
```

## Testing
```bash
cargo test -p decision-gate-cli
```

## References
- Docs/guides/getting_started.md
- decision-gate-mcp/README.md
