# Bootstrap Scripts

Onboarding and smoke-test entry points.

- `bootstrap.sh`: Create a Python venv, install SDK/adapters, and optionally run a hello-flow smoke test.
- `bootstrap.ps1`: Windows version of `bootstrap.sh`.
- `quickstart.sh`: Start a local MCP server and run a quick define -> start -> next -> export -> verify flow.
- `quickstart.ps1`: Windows version of `quickstart.sh`.

Examples:

- `scripts/bootstrap/bootstrap.sh --adapters=all --validate`
- `scripts/bootstrap/quickstart.sh configs/presets/quickstart-dev.toml`
