<!--
Docs/guides/preset_configs.md
============================================================================
Document: Decision Gate Preset Configurations
Description: Curated server presets for onboarding and security posture.
Purpose: Provide runnable, labeled presets with explicit tradeoffs.
Dependencies:
  - decision-gate-mcp configuration
  - Docs/verification/README.md
============================================================================
-->

# Preset Configurations

## At a Glance

Decision Gate ships **three curated presets** to balance onboarding speed with
security posture. Pick one, run it, then graduate to the next.

| Preset              | Intent                                                | Path                                       |
| ------------------- | ----------------------------------------------------- | ------------------------------------------ |
| Quickstart-Dev      | Lowest friction local onboarding                      | `configs/presets/quickstart-dev.toml`      |
| Default-Recommended | Safe-by-default local usage                           | `configs/presets/default-recommended.toml` |
| Hardened            | Strong local security posture (bearer auth + signing) | `configs/presets/hardened.toml`            |

**Important:** Every preset is runnable. For production exposure, add TLS and
mTLS as described in `Docs/guides/security_guide.md`.

---

## Quickstart-Dev (Lowest Friction)

Run it:

```bash dg-run dg-level=manual
cargo run -p decision-gate-cli -- serve --config configs/presets/quickstart-dev.toml
```

Config:

```toml dg-validate=config dg-level=fast
# Decision Gate preset: Quickstart-Dev
# Lowest-friction local setup for first-time users.
# NOT for shared machines or production.

[server]
transport = "http"
bind = "127.0.0.1:4000"
mode = "strict"

[server.auth]
mode = "local_only"

[dev]
permissive = true
permissive_warn = true

[namespace]
allow_default = true
default_tenants = [1]

[trust]
# Audit mode (no signature enforcement).
default_policy = "audit"
min_lane = "verified"

[evidence]
allow_raw_values = false
require_provider_opt_in = true

[schema_registry]
type = "sqlite"
path = "decision-gate-registry.db"

[schema_registry.acl]
# Allow local-only registry access without explicit principal mappings.
allow_local_only = true
require_signing = false

[run_state_store]
# Use SQLite for local durability.
type = "sqlite"
path = "decision-gate.db"
journal_mode = "wal"
sync_mode = "full"
busy_timeout_ms = 5000

[[providers]]
name = "time"
type = "builtin"

[[providers]]
name = "env"
type = "builtin"

[[providers]]
name = "json"
type = "builtin"

[[providers]]
name = "http"
type = "builtin"
```

**Risk posture:** local-only bypass is enabled for registry access. This is
fine for a single-user machine but not for shared hosts.

---

## Default-Recommended (Safe Local Default)

Run it:

```bash dg-run dg-level=manual
cargo run -p decision-gate-cli -- serve --config configs/presets/default-recommended.toml
```

Config:

```toml dg-validate=config dg-level=fast
# Decision Gate preset: Default-Recommended
# Safe-by-default local configuration with explicit principal mapping.

[server]
transport = "http"
bind = "127.0.0.1:4000"
mode = "strict"

[server.auth]
mode = "local_only"

[[server.auth.principals]]
subject = "loopback"
policy_class = "prod"

[[server.auth.principals.roles]]
name = "TenantAdmin"
tenant_id = 1
namespace_id = 1

[namespace]
allow_default = true
default_tenants = [1]

[trust]
# Audit mode (no signature enforcement).
default_policy = "audit"
min_lane = "verified"

[evidence]
allow_raw_values = false
require_provider_opt_in = true

[schema_registry]
type = "sqlite"
path = "decision-gate-registry.db"

[schema_registry.acl]
# Enforce principal mapping even for local-only requests.
allow_local_only = false
require_signing = false

[run_state_store]
# Use SQLite for local durability.
type = "sqlite"
path = "decision-gate.db"
journal_mode = "wal"
sync_mode = "full"
busy_timeout_ms = 5000

[[providers]]
name = "time"
type = "builtin"

[[providers]]
name = "env"
type = "builtin"

[[providers]]
name = "json"
type = "builtin"

[[providers]]
name = "http"
type = "builtin"
```

---

## Hardened (Bearer Auth + Signing)

Run it:

```bash dg-run dg-level=manual
cargo run -p decision-gate-cli -- serve --config configs/presets/hardened.toml
```

Config:

```toml dg-validate=config dg-level=fast
# Decision Gate preset: Hardened
# Strong local security posture. For production, add TLS + mTLS.

[server]
transport = "http"
bind = "127.0.0.1:4000"
mode = "strict"

[server.auth]
mode = "bearer_token"
# Replace this token and update the principal subject hash below.
bearer_tokens = ["dg-hardened-demo-token"]

[[server.auth.principals]]
# Subject is token:sha256(bearer_token). Update when rotating the token.
subject = "token:73a7ceabc74caaa14553ad02540165ba8ad8b709f15e6503b8879552f22042a1"
policy_class = "prod"

[[server.auth.principals.roles]]
name = "TenantAdmin"
tenant_id = 1

[namespace]
# Disable the default namespace id=1; use a non-default namespace (e.g., 2).
allow_default = false

[trust]
# Audit mode (no signature enforcement).
default_policy = "audit"
min_lane = "verified"

[evidence]
allow_raw_values = false
require_provider_opt_in = true

[schema_registry]
type = "sqlite"
path = "decision-gate-registry.db"

[schema_registry.acl]
# Require signatures for schema registry writes.
allow_local_only = false
require_signing = true

[run_state_store]
# Use SQLite for local durability.
type = "sqlite"
path = "decision-gate.db"
journal_mode = "wal"
sync_mode = "full"
busy_timeout_ms = 5000

[[providers]]
name = "time"
type = "builtin"

[[providers]]
name = "env"
type = "builtin"

[[providers]]
name = "json"
type = "builtin"

[[providers]]
name = "http"
type = "builtin"
```

### Updating Bearer Token Principal Mapping

Generate the `token:<sha256>` subject string when you change the token:

```bash dg-run dg-level=manual
python3 - <<'PY'
import hashlib

token = "your-token"
print("token:" + hashlib.sha256(token.encode()).hexdigest())
PY
```

---

## Preset Behavior Mapping

Each preset intentionally sets the same core sections so behavior is explicit:

- **Auth:** `server.auth` (local-only vs bearer token)
- **Registry ACL:** `schema_registry.acl` (local-only bypass vs enforced)
- **Namespace policy:** `namespace.allow_default`
- **Trust posture:** `trust` and `dev.permissive`
- **Durability:** `run_state_store` and `schema_registry` SQLite backends

If you change one of these sections, update the preset and the corresponding
expectations in `system-tests/tests/suites/presets.rs`.
