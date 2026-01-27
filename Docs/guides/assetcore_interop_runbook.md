<!--
Docs/guides/assetcore_interop_runbook.md
============================================================================
Document: AssetCore Interop Runbook
Description: Runbook for Decision Gate <-> AssetCore interoperability validation.
Purpose: Provide deterministic, reproducible steps for offline and live interop validation.
Dependencies:
  - system-tests/tests/fixtures/assetcore/interop/*
  - decision-gate-cli interop subcommand
  - Asset-Core starter-pack docker image bundle
============================================================================
-->

# AssetCore Interop Runbook

## Overview
This runbook validates Decision Gate interoperability with AssetCore using two
paths:
- **Offline fixtures:** deterministic provider stub driven by the AssetCore
  fixture map.
- **Live mode:** Decision Gate evaluates predicates against a running AssetCore
  Docker stack using the interop runner.

The fixture map and provider contract are generated in Asset-Core and synced
into this repository under `system-tests/tests/fixtures/assetcore`.
Anchor policy enforcement is enabled in the AssetCore test config; evidence
must include the canonical ASC anchor set.

For integration framing and architecture context, see
`Docs/integrations/assetcore/`.

## Prerequisites
- Docker installed and running.
- Asset-Core repository available for live mode and fixture refresh.
- Decision Gate CLI built (`cargo build -p decision-gate-cli`) for interop runs.

## Offline Fixture Validation (Recommended)
Run the deterministic fixture suite against the provider stub:

```bash
cargo test -p system-tests --features system-tests --test providers -- --exact assetcore_interop_fixtures
```

The provider stub emits `assetcore.anchor_set` anchors derived from the fixture
map and must satisfy the configured anchor policy.

Artifacts are written under the system-tests run root:
- `interop_spec.json`
- `interop_run_config.json`
- `interop_trigger.json`
- `interop_fixture_map.json`
- `interop_status.json`
- `interop_decision.json`

## Live Mode (AssetCore Docker)
1) Load the Asset-Core image bundle (pinned by digest):

```bash
cd /path/to/Asset-Core
starter-pack/scripts/load_images.sh --bundle starter-pack/docker-images
```

2) Start Asset-Core using the starter-pack compose file:

```bash
docker compose --env-file starter-pack/docker/images.env -f starter-pack/docker/docker-compose.yml up -d
```

3) Start the Decision Gate MCP server (in a separate terminal):

```bash
cargo run -p decision-gate-cli -- serve --config system-tests/tests/fixtures/assetcore/decision-gate.toml
```

The config includes anchor policy requirements for AssetCore evidence and can
optionally enable `namespace.authority` if the ASC write daemon is available.

4) Run the Decision Gate interop evaluation:

```bash
cargo run -p decision-gate-cli -- interop eval \
  --mcp-url http://127.0.0.1:8088/rpc \
  --spec system-tests/tests/fixtures/assetcore/interop/scenarios/assetcore-interop-full.json \
  --run-config system-tests/tests/fixtures/assetcore/interop/run-configs/assetcore-interop-full.json \
  --trigger system-tests/tests/fixtures/assetcore/interop/triggers/assetcore-interop-full.json
```

5) Tear down the Asset-Core stack:

```bash
docker compose --env-file starter-pack/docker/images.env -f starter-pack/docker/docker-compose.yml down
```

## Refreshing Fixtures from Asset-Core
In Asset-Core, regenerate the Decision Gate artifacts and copy the outputs into
this repository:

```bash
# In Asset-Core
cargo run --bin generate-decision-gate

# From the decision-gate repo root
cp /path/to/Asset-Core/Docs/generated/decision-gate/interop/fixture_map.json \
  system-tests/tests/fixtures/assetcore/interop/fixture_map.json
cp /path/to/Asset-Core/Docs/generated/decision-gate/interop/seed_plan.json \
  system-tests/tests/fixtures/assetcore/interop/seed_plan.json
cp /path/to/Asset-Core/Docs/generated/decision-gate/interop/scenarios/assetcore-interop-full.json \
  system-tests/tests/fixtures/assetcore/interop/scenarios/assetcore-interop-full.json
cp /path/to/Asset-Core/Docs/generated/decision-gate/interop/run-configs/assetcore-interop-full.json \
  system-tests/tests/fixtures/assetcore/interop/run-configs/assetcore-interop-full.json
cp /path/to/Asset-Core/Docs/generated/decision-gate/interop/triggers/assetcore-interop-full.json \
  system-tests/tests/fixtures/assetcore/interop/triggers/assetcore-interop-full.json
cp /path/to/Asset-Core/Docs/generated/decision-gate/providers/assetcore_read.json \
  system-tests/tests/fixtures/assetcore/providers/assetcore_read.json
```

## Notes
- All interop inputs are deterministic and must remain ASCII-only.
- Live mode assumes the Asset-Core MCP adapter is exposed on `http://127.0.0.1:8088/rpc`.
- If any predicate output diverges from the fixture map, the test must fail closed.
