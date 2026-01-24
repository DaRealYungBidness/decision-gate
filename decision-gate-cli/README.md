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
and managing runpacks and authoring utilities.

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
