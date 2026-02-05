<!--
Decision Gate CLI README
============================================================================
Document: decision-gate-cli
Description: CLI entry point for running the MCP server and offline utilities.
Purpose: Provide operational commands for serving and validating Decision Gate.
Dependencies:
  - ../../README.md (Decision Gate overview)
  - ../decision-gate-mcp/README.md
  - ../../Docs/configuration/decision-gate.toml.md
============================================================================
-->

# decision-gate-cli

Command-line entry point for running the Decision Gate MCP server and executing
offline utilities (runpack export/verify, authoring validation, config checks,
and provider discovery helpers).

## Table of Contents

- [Overview](#overview)
- [Command Groups](#command-groups)
- [Configuration](#configuration)
- [Usage Examples](#usage-examples)
- [Interop Evaluation](#interop-evaluation)
- [Testing](#testing)
- [References](#references)

## Overview

`decision-gate-cli` wraps the MCP server in `decision-gate-mcp` and exposes
utility commands powered by `decision-gate-core` and `decision-gate-contract`.
The installed binary name is `decision-gate`. When run via Cargo, use:
`cargo run -p decision-gate-cli -- <args>`.

## Command Groups

- `serve` - start the MCP server using `decision-gate.toml`.
- `runpack export` - build a runpack from a scenario spec and run state.
- `runpack verify` - verify a runpack manifest against artifacts.
- `runpack pretty` - render a human-readable view of runpack JSON artifacts.
- `authoring validate` - validate `ScenarioSpec` authoring inputs (JSON/RON).
- `authoring normalize` - normalize authoring inputs to canonical JSON.
- `config validate` - validate `decision-gate.toml`.
- `provider contract get` - fetch provider contract JSON from the registry.
- `provider check-schema get` - fetch check schema details for a provider.
- `provider list` - list configured providers and checks.
- `schema register/list/get` - manage schema registry records via MCP.
- `docs search/list/read` - search and read documentation resources via MCP.
- `interop eval` - drive an MCP server via HTTP/SSE/stdio for integration checks.
- `mcp tools/resources/tool` - MCP client commands for tools and docs resources.
- `contract generate/check` - generate or verify Decision Gate contract artifacts.
- `sdk generate/check` - generate or verify SDK + OpenAPI artifacts.

Run `decision-gate --help` (or `cargo run -p decision-gate-cli -- --help`) for
full flag details.

## Configuration

`serve`, `provider`, and `interop` commands read `decision-gate.toml`. The
config path is optional; if not supplied, the CLI uses the default resolution
rules documented in `Docs/configuration/decision-gate.toml.md`.

CLI output language can be set with `--lang` (preferred) or the
`DECISION_GATE_LANG` environment variable. Supported locales are `en` and
`ca`. When a non-English locale is selected, the CLI prints a disclaimer that
the output is machine-translated.

MCP client auth profiles can be defined in `decision-gate.toml` under
`[client.auth_profiles.<name>]` with `bearer_token` and/or `client_subject`.
Use `--auth-profile <name>` on `mcp` commands to apply the profile.

## Usage Examples

Start the MCP server:

```bash
cargo run -p decision-gate-cli -- serve --config decision-gate.toml
```

Export a runpack from a spec and run state:

```bash
cargo run -p decision-gate-cli -- runpack export \
  --spec ./spec.json \
  --state ./run_state.json \
  --output-dir ./runpack
```

Verify a runpack manifest:

```bash
cargo run -p decision-gate-cli -- runpack verify \
  --manifest ./runpack/runpack.json
```

Render a human-readable runpack view:

```bash
cargo run -p decision-gate-cli -- runpack pretty \
  --manifest ./runpack/runpack.json \
  --output-dir ./runpack-pretty
```

Normalize authoring input (RON -> JSON):

```bash
cargo run -p decision-gate-cli -- authoring normalize \
  --input ./scenario.ron \
  --format ron \
  --output ./scenario.json
```

Fetch provider schema details:

```bash
cargo run -p decision-gate-cli -- provider check-schema get \
  --provider time \
  --check-id after \
  --config decision-gate.toml
```

Search docs and list resources:

```bash
cargo run -p decision-gate-cli -- docs search \
  --query "decision gate" \
  --endpoint http://127.0.0.1:8080/rpc
```

```bash
cargo run -p decision-gate-cli -- docs list \
  --endpoint http://127.0.0.1:8080/rpc
```

List schemas for a tenant/namespace:

```bash
cargo run -p decision-gate-cli -- schema list \
  --tenant-id 1 \
  --namespace-id 1 \
  --endpoint http://127.0.0.1:8080/rpc
```

Fetch a schema record:

```bash
cargo run -p decision-gate-cli -- schema get \
  --tenant-id 1 \
  --namespace-id 1 \
  --schema-id cli-schema \
  --schema-version v1 \
  --endpoint http://127.0.0.1:8080/rpc
```

List MCP tools from a running server:

```bash
cargo run -p decision-gate-cli -- mcp tools list \
  --endpoint http://127.0.0.1:8080/rpc
```

Call a tool via the MCP client:

```bash
cargo run -p decision-gate-cli -- mcp tools call \
  --tool scenario_define \
  --input ./scenario.json \
  --endpoint http://127.0.0.1:8080/rpc
```

## Interop Evaluation

`interop eval` drives a remote MCP server over HTTP/SSE/stdio JSON-RPC and
validates expected run status. It is designed for integration and smoke
validation, not for load testing.

```bash
cargo run -p decision-gate-cli -- interop eval \
  --endpoint http://127.0.0.1:8080/rpc \
  --spec ./scenario.json \
  --run-config ./run_config.json \
  --trigger ./trigger.json \
  --expect-status completed
```

## Testing

```bash
cargo test -p decision-gate-cli
```

## References

Tevvez. (2020). _Let Me Down Slowly (Alec Benjamin) - Parallel Universe Remix_ [Audio recording]. YouTube. https://www.youtube.com/watch?v=8Gs6pFM-B5I

Ava Crown, & YKATI. (2024). _La Luna_ [Audio recording]. YouTube. https://www.youtube.com/watch?v=T52RbsZYonA
