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

Decision Gate ships **four curated presets** to balance onboarding speed with
security posture. Pick one, run it, then graduate to the next.

| Preset              | Intent                                                | Path                                       |
| ------------------- | ----------------------------------------------------- | ------------------------------------------ |
| Quickstart-Dev      | Lowest friction local onboarding                      | `configs/presets/quickstart-dev.toml`      |
| Default-Recommended | Safe-by-default local usage                           | `configs/presets/default-recommended.toml` |
| Container-Prod      | Containerized server baseline (bearer auth, upstream TLS) | `configs/presets/container-prod.toml`  |
| Hardened            | Strong local security posture (bearer auth + signing) | `configs/presets/hardened.toml`            |

**Important:** Every preset is runnable. For production exposure, use TLS
(in-container or upstream termination) and mTLS as described in
`Docs/guides/security_guide.md`.

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
config = { root = "./evidence", root_id = "evidence-root", max_bytes = 1048576, allow_yaml = true }

[[providers]]
name = "http"
type = "builtin"
```

**Risk posture:** local-only bypass is enabled for registry access. This is
fine for a single-user machine but not for shared hosts.

**Note:** These presets assume an `./evidence` directory exists for the `json`
provider root. Create it (or change `root`) before starting the server.

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
config = { root = "./evidence", root_id = "evidence-root", max_bytes = 1048576, allow_yaml = true }

[[providers]]
name = "http"
type = "builtin"
```

---

## Container-Prod (Container Baseline)

Run it:

```bash dg-run dg-level=manual
DECISION_GATE_ALLOW_NON_LOOPBACK=1 cargo run -p decision-gate-cli -- serve \
  --config configs/presets/container-prod.toml
```

Config:

```toml dg-validate=config dg-level=fast
# Decision Gate preset: Container-Prod
# Production-oriented container configuration.
# Requires explicit auth and upstream TLS termination.

[server]
transport = "http"
bind = "0.0.0.0:8080"
mode = "strict"
tls_termination = "upstream"

[server.auth]
mode = "bearer_token"
# Replace this token and update the principal subject hash below.
bearer_tokens = ["dg-container-demo-token"]

[[server.auth.principals]]
# Subject is token:sha256(bearer_token). Update when rotating the token.
subject = "token:5e268e45a49c26207274917a880f33eafbf6e98563170d0bfe1504408d33d18c"
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
type = "memory"

[schema_registry.acl]
allow_local_only = false
require_signing = false

[run_state_store]
type = "memory"

[[providers]]
name = "time"
type = "builtin"

[[providers]]
name = "env"
type = "builtin"

[[providers]]
name = "json"
type = "builtin"
config = { root = "./evidence", root_id = "evidence-root", max_bytes = 1048576, allow_yaml = true }

[[providers]]
name = "http"
type = "builtin"
```

**Risk posture:** explicit auth required; upstream TLS termination assumed.
See `Docs/guides/container_deployment.md` for deployment guidance.

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
config = { root = "./evidence", root_id = "evidence-root", max_bytes = 1048576, allow_yaml = true }

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
- **TLS termination:** `server.tls_termination` (server vs upstream)

If you change one of these sections, update the preset and the corresponding
expectations in `system-tests/tests/suites/presets.rs`.
