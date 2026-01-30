<!--
Docs/guides/assetcore_interop_runbook.md
============================================================================
Document: AssetCore Interop Runbook
Description: Runbook for Decision Gate <-> AssetCore interoperability validation.
Purpose: Provide deterministic, reproducible steps for offline and live interop validation.
Dependencies:
  - system-tests/tests/fixtures/assetcore/interop/*
  - decision-gate-cli interop subcommand
  - AssetCore starter-pack docker image bundle
============================================================================
-->

# AssetCore Interop Runbook

## At a Glance

**What:** Validate Decision Gate <-> AssetCore interoperability
**Why:** Ensure AssetCore evidence integrates correctly with DG gates
**Who:** Integration engineers, test operators
**Prerequisites:** Docker (live mode), AssetCore repo access, Decision Gate CLI built

---

## Fixture vs Live Mode

### Offline Fixtures (Deterministic)
- Uses a provider stub with a fixture map.
- Fully deterministic and fast.

### Live Mode (Integration)
- Runs AssetCore services in Docker.
- Exercises real MCP calls and network paths.

---

## Anchor Policy (Exact)

Decision Gate enforces anchor rules **via config**, not the scenario:

```toml
[anchors]
[[anchors.providers]]
provider_id = "assetcore_read"
anchor_type = "assetcore.anchor_set"
required_fields = ["assetcore.namespace_id", "assetcore.commit_id", "assetcore.world_seq"]
```

Evidence anchors: `anchor_value` is a **string** containing canonical JSON.
Example EvidenceResult snippet:

```json
{
  "evidence_anchor": {
    "anchor_type": "assetcore.anchor_set",
    "anchor_value": "{\"assetcore.namespace_id\":1,\"assetcore.commit_id\":\"c123\",\"assetcore.world_seq\":42}"
  }
}
```

---

## Offline Fixture Validation (Recommended)

```bash
cargo test -p system-tests \
  --features system-tests \
  --test providers \
  -- \
  --exact assetcore_integration::assetcore_interop_fixtures
```

What happens:
- Provider stub loads `system-tests/tests/fixtures/assetcore/interop/fixture_map.json`.
- DG evaluates gates using fixture evidence.

---

## Live Mode (AssetCore Docker)

### Step 1: Load AssetCore Images

```bash
cd <ASSETCORE_REPO_ROOT>
starter-pack/scripts/load_images.sh --bundle starter-pack/docker-images
```

### Step 2: Start AssetCore Stack

```bash
docker compose \
  --env-file starter-pack/docker/images.env \
  -f starter-pack/docker/docker-compose.yml \
  up -d
```

### Step 3: Start Decision Gate MCP Server

```bash
cargo run -p decision-gate-cli -- \
  serve \
  --config system-tests/tests/fixtures/assetcore/decision-gate.toml
```

**Relevant fixture config (exact fields):**

```toml
[server]
transport = "http"
bind = "127.0.0.1:8088"

[[providers]]
name = "assetcore_read"
type = "mcp"
url = "http://127.0.0.1:9000/mcp"
allow_insecure_http = true
capabilities_path = "system-tests/tests/fixtures/assetcore/providers/assetcore_read.json"
```

### Step 4: Run Interop Evaluation

```bash
cargo run -p decision-gate-cli -- \
  interop eval \
  --mcp-url http://127.0.0.1:8088/rpc \
  --spec system-tests/tests/fixtures/assetcore/interop/scenarios/assetcore-interop-full.json \
  --run-config system-tests/tests/fixtures/assetcore/interop/run-configs/assetcore-interop-full.json \
  --trigger system-tests/tests/fixtures/assetcore/interop/triggers/assetcore-interop-full.json
```

### Step 5: Tear Down AssetCore Stack

```bash
docker compose \
  --env-file starter-pack/docker/images.env \
  -f starter-pack/docker/docker-compose.yml \
  down
```

---

## Refreshing Fixtures

When AssetCore contracts change, regenerate fixtures from the AssetCore repo and copy them into this repo:

```bash
cp <ASSETCORE_GENERATED_DIR>/decision-gate/interop/fixture_map.json \
  system-tests/tests/fixtures/assetcore/interop/fixture_map.json

cp <ASSETCORE_GENERATED_DIR>/decision-gate/providers/assetcore_read.json \
  system-tests/tests/fixtures/assetcore/providers/assetcore_read.json
```

---

## Troubleshooting

### AssetCore MCP connection refused
- Verify AssetCore MCP adapter is running.
- Check the provider URL in the Decision Gate config (`http://127.0.0.1:9000/mcp`).

### Anchor validation failures
- Confirm anchor policy under `[anchors]`.
- Ensure `anchor_value` is a string containing canonical JSON with required fields.

---

## Notes

- Interop fixtures use deterministic timestamps (no wall clock).
- Interop fixtures in this repo are ASCII-only; keep them that way for deterministic diffs.

---

## Glossary

**Anchor:** External reference proving evidence provenance.
**Fixture Map:** JSON mapping from queries to deterministic evidence results.
**Interop:** Interoperability validation between DG and AssetCore.
