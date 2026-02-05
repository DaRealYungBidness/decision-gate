# Adapter Scripts

Adapter validation and smoke-test utilities.

- `adapter_tests.sh`: Main harness. Spawns a local MCP server (unless `--endpoint` is set), installs deps in a venv, and runs conformance + roundtrip checks.
- `adapter_conformance.py`: Compares adapter tool surfaces with `Docs/generated/decision-gate/tooling.json`.
- `adapter_roundtrip.py`: Runs per-tool adapter roundtrip calls against a live MCP server.
- `typecheck_adapters.sh`: Runs Pyright strict typing checks for adapters and adapter scripts.

Examples:

- `scripts/adapters/adapter_tests.sh --all`
- `scripts/adapters/adapter_tests.sh --frameworks=langchain,crewai`
