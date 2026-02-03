<!--
Docs/guides/container_deployment.md
============================================================================
Document: Decision Gate Container Deployment Guide
Description: Container contract, build/run steps, and operator guidance.
Purpose: Provide an implementable deployment guide for the OSS server image.
Dependencies:
  - Docs/configuration/decision-gate.toml.md
  - Docs/guides/preset_configs.md
  - Docs/guides/security_guide.md
============================================================================
-->

# Container Deployment

This guide defines the Decision Gate OSS container contract and provides
operator-ready steps for building and running the MCP server image.

The container image is a **server artifact**. It runs `decision-gate serve`
and is intended for production-style deployment.

## Contract Summary (Authoritative)

- **Entrypoint:** `decision-gate`
- **Default command:** `serve --config /etc/decision-gate/decision-gate.toml --allow-non-loopback`
- **Config mount:** `/etc/decision-gate/decision-gate.toml`
- **Transport:** HTTP (SSE optional)
- **Auth:** required (bearer token or mTLS proxy header)
- **TLS:** terminated upstream by default (`server.tls_termination = "upstream"`)
- **Persistence:** stateless by default; SQLite is explicit
- **Runtime:** non-root, minimal privileges, stdout/stderr logs
- **Writable paths:** `/var/lib/decision-gate` (only when SQLite enabled)

## Build the Image

Local build:

```bash dg-run dg-level=manual
docker build -t decision-gate:dev .
```

Multi-arch build (amd64 + arm64):

```bash dg-run dg-level=manual
IMAGE_REPO=ghcr.io/your-org/decision-gate IMAGE_TAG=dev \
  scripts/container/build_container.sh
```

Push multi-arch:

```bash dg-run dg-level=manual
IMAGE_REPO=ghcr.io/your-org/decision-gate IMAGE_TAG=dev PUSH=1 \
  scripts/container/build_container.sh
```

Notes:
- `IMAGE_REPO=ghcr.io/your-org/decision-gate` is a placeholder. Replace `your-org`
  with your GitHub org or user (for example, `ghcr.io/decision-gate/decision-gate`).
- `IMAGE_TAG=dev` is a local/dev example. For releases, use a version tag
  (for example, `vX.Y.Z`) and optionally publish `latest`.

## Tags and Release Policy

Local/dev:
- `decision-gate:dev` for ad-hoc testing.
- Do not sign or publish SBOM/provenance for these tags.

Release:
- `ghcr.io/<org>/decision-gate:vX.Y.Z` (immutable release tag).
- `ghcr.io/<org>/decision-gate:latest` (points at most recent release).
- A dependency SBOM (Rust deps) is published for release tags.
- Container SBOMs, signatures, and provenance are not yet emitted in OSS.

## Configuration

The container expects a config file at:
`/etc/decision-gate/decision-gate.toml`.

Use the container preset as a baseline:
`configs/presets/container-prod.toml`.

Key requirements:
- `server.bind` must be non-loopback (e.g., `0.0.0.0:8080`).
- `server.auth.mode` must be `bearer_token` or `mtls`.
- `server.tls_termination = "upstream"` when TLS is terminated outside the container.

## Run the Container

Minimal run (bearer token auth, upstream TLS termination):

```bash dg-run dg-level=manual
docker run --rm -p 8080:8080 \
  -v "$(pwd)/configs/presets/container-prod.toml:/etc/decision-gate/decision-gate.toml:ro" \
  decision-gate:dev
```

Notes:
- Replace the demo token in the preset before production use.
- `--allow-non-loopback` is part of the default container command.
  If you override the command, include `--allow-non-loopback` or set
  `DECISION_GATE_ALLOW_NON_LOOPBACK=1`.

### In-Container TLS (Optional)

If you need TLS inside the container, set:

```toml dg-parse dg-level=fast
[server]
tls_termination = "server"

[server.tls]
cert_path = "/etc/decision-gate/tls/server.crt"
key_path = "/etc/decision-gate/tls/server.key"
```

Mount the certs and update your container runtime accordingly.

## Durable Mode (SQLite)

By default, the container preset uses in-memory stores.

To enable SQLite durability, update the config:

```toml dg-parse dg-level=fast
[schema_registry]
type = "sqlite"
path = "/var/lib/decision-gate/schema-registry.db"

[run_state_store]
type = "sqlite"
path = "/var/lib/decision-gate/decision-gate.db"
journal_mode = "wal"
sync_mode = "full"
busy_timeout_ms = 5000
```

Run with a writable volume:

```bash dg-run dg-level=manual
docker run --rm -p 8080:8080 \
  -v "$(pwd)/configs/presets/container-prod.toml:/etc/decision-gate/decision-gate.toml:ro" \
  -v decision-gate-data:/var/lib/decision-gate \
  decision-gate:dev
```

## Auth Expectations

Bearer token example:

```bash dg-run dg-level=manual
curl -sS -X POST http://127.0.0.1:8080/rpc \
  -H "Authorization: Bearer dg-container-demo-token" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'
```

For mTLS proxy mode, set:

```toml dg-parse dg-level=fast
[server.auth]
mode = "mtls"
mtls_subjects = ["CN=decision-gate-client,O=Example Corp"]
```

Then have your proxy inject `x-decision-gate-client-subject`.

## Health Endpoints

Decision Gate exposes standard Kubernetes probes:
- `GET /healthz` for liveness
- `GET /readyz` for readiness

These endpoints are intentionally unauthenticated and return minimal status
only. `/readyz` performs lightweight readiness checks (state store + schema
registry) and returns HTTP 503 with `{"status":"not_ready"}` if dependencies
are unavailable.

```bash dg-run dg-level=manual
curl -sS http://127.0.0.1:8080/healthz
curl -sS http://127.0.0.1:8080/readyz
```

Both endpoints return HTTP 200 with a JSON payload.

## Kubernetes Example

```yaml dg-parse dg-level=fast
apiVersion: apps/v1
kind: Deployment
metadata:
  name: decision-gate
spec:
  replicas: 1
  selector:
    matchLabels:
      app: decision-gate
  template:
    metadata:
      labels:
        app: decision-gate
    spec:
      containers:
        - name: decision-gate
          image: ghcr.io/your-org/decision-gate:latest
          ports:
            - containerPort: 8080
          securityContext:
            runAsNonRoot: true
            runAsUser: 10001
            readOnlyRootFilesystem: true
          livenessProbe:
            httpGet:
              path: /healthz
              port: 8080
            initialDelaySeconds: 5
            periodSeconds: 10
          readinessProbe:
            httpGet:
              path: /readyz
              port: 8080
            initialDelaySeconds: 2
            periodSeconds: 5
          volumeMounts:
            - name: config
              mountPath: /etc/decision-gate/decision-gate.toml
              subPath: decision-gate.toml
              readOnly: true
            - name: data
              mountPath: /var/lib/decision-gate
      volumes:
        - name: config
          configMap:
            name: decision-gate-config
        - name: data
          emptyDir: {}
```

For SQLite durability, replace `emptyDir` with a persistent volume claim.

## Supply-Chain Artifacts (Optional but Recommended)

Decision Gate OSS currently publishes a dependency SBOM (Rust deps) for release
tags. Container SBOMs, signatures, and provenance are not yet emitted in OSS.
The commands below are optional examples if you want to add those artifacts in
your own release pipeline.

SBOM (example using `syft`):

```bash dg-run dg-level=manual
syft packages decision-gate:dev -o spdx-json > decision-gate.sbom.spdx.json
```

Image signing (cosign):

```bash dg-run dg-level=manual
cosign sign decision-gate:dev
```

Provenance attestation:

```bash dg-run dg-level=manual
cosign attest --predicate decision-gate.sbom.spdx.json --type spdx decision-gate:dev
```

Document whether images are signed and provide verification commands.
