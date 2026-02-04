<!--
Docs/guides/security_guide.md
============================================================================
Document: Decision Gate Security Guide
Description: Security posture and trust policy guidance.
Purpose: Explain evidence trust, disclosure policy, auth, and anchor validation.
Dependencies:
  - Docs/security/threat_model.md
  - decision-gate-mcp configuration
============================================================================
-->

# Security Guide

## At a Glance

**What:** Security controls for trust, signatures, disclosure, and access
**Why:** Prevent evidence tampering and limit data exposure
**Who:** Security engineers, compliance teams, production operators
**Prerequisites:** [evidence_flow_and_execution_model.md](evidence_flow_and_execution_model.md)

---

## Preset Postures

Decision Gate ships four presets (see [preset_configs.md](preset_configs.md)):

- **Quickstart-Dev:** local-only auth, registry local-only bypass, dev-permissive enabled.
- **Default-Recommended:** local-only auth with explicit principal mappings; registry bypass off.
- **Hardened:** bearer auth, default namespace disabled, schema signing required.
- **Container-Prod:** bearer auth, upstream TLS termination, stateless defaults.

These presets are intentionally explicit. If you change the posture (auth mode,
namespace defaults, registry ACL, trust lanes), update the preset and re-run
system-tests.

---

## Fail-Closed Architecture

Decision Gate **fails closed**: missing or invalid evidence produces `unknown`, which holds gates.

Security controls applied (in order):
1. **Trust lane minimum** (`min_lane`)
2. **Anchor validation** (if configured)
3. **Signature verification** (if required)
4. **Comparator evaluation** (tri-state)

---

## Access Control

### Tool Access (MCP API)
Tool access is controlled by `[server.auth]`.

```toml dg-parse dg-level=fast
[server.auth]
mode = "bearer_token"
bearer_tokens = ["token-1", "token-2"]
allowed_tools = ["scenario_define", "scenario_start", "scenario_next"]
```

Modes:
- `local_only` (default): loopback + stdio only
- `bearer_token`: HTTP `Authorization: Bearer <token>` required
- `mtls`: uses the `x-decision-gate-client-subject` header from a trusted TLS-terminating proxy

Registry note:
- `schema_registry.acl.allow_local_only = true` bypasses principal mapping for local-only callers. Use only for dev/local onboarding.

### Non-Loopback Binding
Binding HTTP/SSE to non-loopback requires **all** of:
1. `--allow-non-loopback` or `DECISION_GATE_ALLOW_NON_LOOPBACK=1`
2. `[server.tls]` configured **or** `server.tls_termination = "upstream"`
3. Non-local auth (`bearer_token` or `mtls`)

For in-container `mtls`, `server.tls.client_ca_path` must be set and
`require_client_cert = true`. For upstream TLS termination, enforce mTLS at
the proxy and forward `x-decision-gate-client-subject`.

---

## Trust Lanes

Evidence is classified into lanes:
- **Verified**: provider-fetched evidence
- **Asserted**: precheck payloads

Minimum lane is enforced by:
```toml dg-parse dg-level=fast
[trust]
min_lane = "verified"   # or "asserted"
```

If evidence lane is below the minimum, the condition becomes `unknown` and a `trust_lane` error is recorded in the runpack.

### Dev-Permissive Mode

```toml dg-parse dg-level=fast
[dev]
permissive = true
```

Effects:
- Effective `min_lane` becomes `asserted` for most providers.
- Providers listed in `dev.permissive_exempt_providers` remain strict (default: `assetcore`, `assetcore_read`).
- Not allowed when `namespace.authority.mode = "assetcore_http"`.

---

## Signature Verification

Configured with `trust.default_policy`:

```toml dg-parse dg-level=fast
[trust]
# Require Ed25519 signatures from these public key files.
default_policy = { require_signature = { keys = ["/etc/decision-gate/keys/provider.pub"] } }
```

Notes:
- Each `keys` entry is a **file path**. The key file may contain raw 32-byte public key bytes or base64 text.
- `EvidenceResult.signature.key_id` must match the configured key path string.
- The signature is verified over **canonical JSON of the HashDigest**.
- If `evidence_hash` is present, it must match the canonical hash of the evidence value.

If a signature check fails, the provider call fails and the condition becomes `unknown` with `provider_error` recorded.

---

## Anchor Validation

Anchors are configured via **server config**, not the scenario:

```toml dg-parse dg-level=fast
[anchors]
[[anchors.providers]]
provider_id = "assetcore_read"
anchor_type = "assetcore.anchor_set"
required_fields = ["assetcore.namespace_id", "assetcore.commit_id", "assetcore.world_seq"]
```

Anchor rules (exact):
- `EvidenceResult.evidence_anchor.anchor_type` must match.
- `anchor_value` must be a **string** containing canonical JSON.
- That JSON must parse to an **object**.
- Required fields must exist and be scalar **string or number** (no booleans, arrays, objects, or nulls).

Violations produce `anchor_invalid` and the condition becomes `unknown`.

---

## Evidence Disclosure

`evidence_query` can return raw values, but disclosure is policy-controlled:

```toml dg-parse dg-level=fast
[evidence]
allow_raw_values = false
require_provider_opt_in = true

[[providers]]
name = "json"
type = "builtin"
config = { root = "/var/lib/decision-gate/evidence", root_id = "evidence-root", max_bytes = 1048576, allow_yaml = false }
allow_raw = true
```

Behavior:
- If raw disclosure is blocked, Decision Gate **redacts** `value` and `content_type` but still returns hashes and anchors.
- This is **not** a JSON-RPC error.
- Provider names are unique; built-in identifiers (`time`, `env`, `json`, `http`) are reserved.

---

## Secure Production Configuration

```toml dg-parse dg-level=fast
[server]
transport = "http"
bind = "0.0.0.0:4000"
mode = "strict"

[server.auth]
mode = "bearer_token"
bearer_tokens = ["token-1"]

[server.tls]
cert_path = "/etc/decision-gate/tls/cert.pem"
key_path = "/etc/decision-gate/tls/key.pem"

[trust]
min_lane = "verified"
default_policy = { require_signature = { keys = ["/etc/decision-gate/keys/provider.pub"] } }

[evidence]
allow_raw_values = false
require_provider_opt_in = true

[anchors]
[[anchors.providers]]
provider_id = "json"
anchor_type = "file_path_rooted"
required_fields = ["root_id", "path"]

[namespace]
allow_default = false

[policy]
engine = "static"

[policy.static]
default = "deny"

[run_state_store]
type = "sqlite"
path = "/var/lib/decision-gate/decision-gate.db"
journal_mode = "wal"
sync_mode = "full"
```

**Important:** For non-loopback binds, add `--allow-non-loopback` or set `DECISION_GATE_ALLOW_NON_LOOPBACK=1`.

---

## SQLite Integrity

The SQLite run state store saves canonical JSON snapshots and **verifies hashes on load**. Corruption or hash mismatches fail closed.

Best practices:
- Use a durable volume (avoid `/tmp`).
- Backup the `.db`, `-wal`, and `-shm` files together.

---

## Common Pitfalls (Corrected)

- **"Signature errors return signature_invalid."** -> No. Signature failures surface as `provider_error` during provider calls.
- **"Anchor policy is in the scenario."** -> No. It is configured under `[anchors]`.
- **"policy engine controls tool access."** -> No. `[policy]` controls **packet dispatch** only. Tool access is `[server.auth]`.
- **"evidence_query returns an error when raw values are blocked."** -> No. Values are redacted, not rejected.

---

## Cross-Reference

- [evidence_flow_and_execution_model.md](evidence_flow_and_execution_model.md)
- [provider_protocol.md](provider_protocol.md)
- [provider_development.md](provider_development.md)
