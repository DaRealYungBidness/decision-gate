<!--
Decision Gate Container Plan
============================================================================
Document: decision_gate_container_plan.md
Description: World-class container contract + phased implementation plan.
Purpose: Provide a complete, implementable roadmap for OSS container delivery.
============================================================================
-->

# Decision Gate OSS Container Plan (World-Class Baseline)

This document defines the authoritative container contract and a phased plan
to deliver an enterprise-grade OSS container image for Decision Gate.

The goal is not "have Docker" but "ship an operator-ready server artifact"
with a clear security posture and supply-chain story.

## Scope

- One OSS container image for the MCP server (`decision-gate serve`).
- Portable builds per architecture (no `target-cpu=native` in distributed images).
- Explicit security posture, deterministic-ish build pipeline, and supply-chain
  artifacts (SBOM/signing/provenance) in later phases.

## Non-Goals

- No enterprise-only features in OSS crates.
- No containerization of SDKs, examples, or developer tooling.
- No implicit exposure of network services.

## World-Class Decisions (Authoritative)

These are the decisions to implement and preserve unless intentionally revised.

1) Transport: HTTP is the container default.
   - Why: Containers are deployed as network services. HTTP is the standard
     service interface for enterprise/DoD deployments.

2) Explicit bind required (no implicit public exposure).
   - Why: Fail-closed security posture. Operators must explicitly opt in to
     non-loopback bind via config.

3) Auth required by default for container deployments.
   - Why: Enterprise and DoD environments require authenticated service access.

4) TLS terminates upstream by default (in-container TLS supported).
   - Why: Most production environments terminate TLS at ingress/service mesh.
     In-container TLS remains supported via config for environments that require
     it, but is not the default.

5) Stateless by default; durability is explicit.
   - Why: Stateless defaults avoid accidental persistence and make deployments
     predictable. Durable mode is available by configuring SQLite + volume.

6) Non-root runtime and minimal privileges.
   - Why: Standard enterprise hardening. Reduce blast radius if compromised.

7) Read-only filesystem by default; explicit writable paths only.
   - Why: Enforces immutability, simplifies compliance, and reduces risk.

8) Portable builds only for distributed artifacts.
   - Why: Prevent illegal-instruction failures and ensure broad compatibility.

## Container Contract (Authoritative)

### Entrypoint and Command

- Entrypoint: `decision-gate`
- Default command: `serve --config /etc/decision-gate/decision-gate.toml`
- The image runs a long-lived MCP server process.

### Configuration Injection

- Primary: mount a config file at
  `/etc/decision-gate/decision-gate.toml`.
- The container contract does not require environment-variable overrides.
  (Optional future enhancement.)

### Network Binding

- The server must not bind to a non-loopback address unless explicitly set in
  the mounted config (e.g., `bind = "0.0.0.0:8080"`).
- HTTP/SSE require `bind` per current config validation.

### Auth Defaults

- Container deployments require an explicit auth mode (e.g. `bearer_token` or
  `mtls` proxy-header mode).
- `local_only` is permitted only for development/testing.

### TLS Posture

- Default: TLS terminates upstream (ingress/proxy).
- In-container TLS is supported via `[server.tls]` in config when required.

### Persistence

- Default: in-memory store / stateless runtime.
- Durable mode: SQLite configured explicitly, with data dir mounted
  at `/var/lib/decision-gate` (or a path defined in config).

### Runtime User and Filesystem

- Non-root user (e.g. UID/GID 10001).
- Root filesystem read-only by default.
- Writable paths (only when needed):
  - `/var/lib/decision-gate` (SQLite data dir)
  - `/tmp` (if required by runtime)

### Logging

- Logs to stdout/stderr. No in-container log files by default.

### Health

- If/when added: HTTP health endpoint or explicit readiness policy documented.

## Phased Implementation Plan

Each phase is implementable on its own and preserves the OSS/enterprise boundary.

### Phase 0: Contract and Documentation (Now)

Deliverables:
- This document (authoritative contract + plan).
- A short "Container Deployment" guide:
  - Path: `Docs/guides/container_deployment.md`
  - Content:
    - Quickstart `docker run` with config mount.
    - Explicit bind + auth example.
    - Durable mode example with SQLite volume.
    - TLS upstream vs in-container explanation.
- Add a "Container Support" section to `README.md` that links to the guide.

Verification:
- Doc text matches current configuration rules (explicit bind required, auth
  modes, TLS config).

### Phase 1: Minimal OSS Server Image

Goal: one image, one responsibility, non-root, minimal base.

Deliverables:
- Root Dockerfile (multi-stage) to build `decision-gate`:
  - Builder stage:
    - `cargo build -p decision-gate-cli --release --locked`
  - Runtime stage:
    - Minimal base (debian-slim or distroless)
    - Non-root user
    - Copy binary to `/usr/local/bin/decision-gate`
    - Create `/etc/decision-gate` and `/var/lib/decision-gate`
    - `ENTRYPOINT ["decision-gate"]`
    - `CMD ["serve", "--config", "/etc/decision-gate/decision-gate.toml"]`
- Example config file:
  - Path: `configs/presets/container-prod.toml`
  - Includes:
    - `transport = "http"`
    - explicit `bind = "0.0.0.0:8080"`
    - auth mode set (bearer token or mtls proxy header)
    - audit enabled
    - limits set to sane defaults

Verification:
- `docker build` succeeds and runs.
- `docker run` with mounted config and auth token responds over HTTP.

### Phase 2: Multi-Arch Portable Images (No Hosted CI Required)

Goal: publish `linux/amd64` and `linux/arm64` portable images.

Deliverables:
- Local build script (no hosted CI dependency):
  - Path: `scripts/build_container.sh`
  - Accepts `IMAGE_REPO` and `IMAGE_TAG` env vars.
  - Uses `docker buildx build --platform linux/amd64,linux/arm64`.
- Document build steps in `Docs/guides/container_deployment.md`.
- Tagging policy documented:
  - `latest` for most recent release
  - `vX.Y.Z` for versioned releases

Verification:
- `docker buildx imagetools inspect` confirms both platforms.

### Phase 3: Supply-Chain Hardening

Goal: enterprise-grade integrity and auditability.

Deliverables:
- SBOM generation (SPDX or CycloneDX):
  - Document tooling (e.g., `syft`) and commands.
- Image signing:
  - `cosign sign` (keyless or key-based) with verification instructions.
- Provenance attestation:
  - `cosign attest` (SLSA/in-toto format).
- Publish verification steps in `Docs/guides/container_deployment.md`.

Verification:
- SBOM artifact produced and stored alongside release.
- Image signature verifiable via published command.
- Provenance attestation attached and verifiable.

### Phase 4: Operator Extras (Optional)

Goal: reduce friction for enterprise/DoD adopters.

Deliverables:
- Health endpoint or documented readiness semantics.
- Kubernetes manifest example with:
  - Non-root security context
  - Read-only filesystem
  - Explicit volume mounts
  - Resource limits
- Compatibility matrix of transports and auth modes.

Verification:
- K8s example works with the published image.

## Implementation Notes (Explicit Guidance for LLMs)

1) Do not weaken OSS security defaults or add enterprise dependencies.
2) Keep the container image strictly scoped to `decision-gate serve`.
3) Do not add implicit bind defaults to the binary itself. If defaults are
   provided, they must live in the container example config only.
4) Do not enable `target-cpu=native` for distributed artifacts.
5) Always document what is guaranteed vs optional in the supply-chain posture.

## Open Questions (If Reconsidered Later)

- If a default bind/port is desired, should it be set in the container entrypoint
  or in the example config? Current decision: config only.
- If a default auth mode is desired, should the example config enforce bearer
  tokens or mTLS proxy headers? Current decision: auth required.
- If durability is expected by default, should the container require a volume?
  Current decision: stateless by default.
