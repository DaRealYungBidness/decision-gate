<!--
Docs/configuration/decision-gate.toml.md
============================================================================
Document: Decision Gate MCP Configuration
Description: Reference for decision-gate.toml configuration fields.
Purpose: Document server, trust, evidence, and provider settings.
Dependencies:
  - decision-gate-mcp/src/config.rs
============================================================================
-->

# decision-gate.toml Configuration

## Overview
`decision-gate.toml` configures the MCP server, trust policies, evidence
disclosure defaults, and provider registry. All inputs are validated and
fail closed on errors.

## Top-Level Sections

### `[server]`
| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `transport` | `"stdio" | "http" | "sse"` | `stdio` | HTTP/SSE require `bind`; non-loopback requires auth. |
| `mode` | `"strict" | "dev_permissive"` | `strict` | `dev_permissive` is legacy; prefer `[dev]` for explicit opt-in. |
| `bind` | string | `null` | Required for HTTP/SSE; loopback-only unless auth enabled. |
| `max_body_bytes` | integer | `1048576` | Maximum JSON-RPC request size. |
| `auth` | table | `null` | Inbound authn/authz for MCP tool calls. |
| `audit` | table | `{ enabled = true }` | Structured audit logging configuration. |

### `[server.auth]`
| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `mode` | `"local_only" | "bearer_token" | "mtls"` | `local_only` | Local-only uses loopback/stdio. |
| `bearer_tokens` | array | `[]` | Required for `bearer_token` mode. |
| `mtls_subjects` | array | `[]` | Required for `mtls` mode (trusted proxy header). |
| `allowed_tools` | array | `[]` | Optional tool allowlist (per-tool authz). |
| `principals` | array | `[]` | Optional principal mappings for registry ACL (subject/roles). |

Bearer token example:
```toml
[server.auth]
mode = "bearer_token"
bearer_tokens = ["token-1", "token-2"]
allowed_tools = ["scenario_define", "scenario_start", "scenario_next"]
```

mTLS subject example (via trusted proxy header):
```toml
[server.auth]
mode = "mtls"
mtls_subjects = ["CN=decision-gate-client,O=Example Corp"]
```
When using `mtls` mode, the server expects the
`x-decision-gate-client-subject` header from a trusted TLS-terminating proxy.

Principal mapping example (registry ACL):
```toml
[[server.auth.principals]]
subject = "loopback"
policy_class = "prod"

[[server.auth.principals.roles]]
name = "TenantAdmin"
tenant_id = 1
namespace_id = 1
```
Built-in registry ACL expects `policy_class` values like `prod`, `project`,
or `scratch` (case-insensitive). Unknown values are treated as `prod`.

### `[server.audit]`
| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `enabled` | bool | `true` | Enable structured audit logging (JSON lines). |
| `path` | string | `null` | Optional audit log path; defaults to stderr. |
| `log_precheck_payloads` | bool | `false` | Explicit opt-in to log raw precheck payloads. |

### `[dev]`
| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `permissive` | bool | `false` | Explicit opt-in to allow asserted evidence in dev. |
| `permissive_scope` | `"asserted_evidence_only"` | `asserted_evidence_only` | Dev-permissive scope (fixed for v1). |
| `permissive_ttl_days` | integer | `null` | Optional TTL for warnings (days since config mtime). |
| `permissive_warn` | bool | `true` | Emit warnings on startup when dev-permissive enabled/expired. |
| `permissive_exempt_providers` | array | `["assetcore_read","assetcore"]` | Providers exempt from dev-permissive relaxations. |

Dev-permissive is rejected when `namespace.authority.mode = "assetcore_http"`.

### `[namespace]`
| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `allow_default` | bool | `false` | Allow the default namespace id (1) (opt-in). |
| `default_tenants` | array | `[]` | Tenant allowlist required when `allow_default = true`. |

### `[namespace.authority]`
| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `mode` | `"none" | "assetcore_http"` | `none` | Namespace authority backend selection. |
| `assetcore` | table | `null` | Asset Core authority settings (required for `assetcore_http`). |

### `[namespace.authority.assetcore]`
| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `base_url` | string | — | Asset Core write-daemon base URL. |
| `auth_token` | string | `null` | Optional bearer token for namespace lookup. |
| `connect_timeout_ms` | integer | `500` | HTTP connect timeout (ms). |
| `request_timeout_ms` | integer | `2000` | HTTP request timeout (ms). |

Asset Core authority example:
```toml
[namespace.authority]
mode = "assetcore_http"

[namespace.authority.assetcore]
base_url = "http://127.0.0.1:9001"
auth_token = "token"
connect_timeout_ms = 500
request_timeout_ms = 2000
```

### `[trust]`
| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `default_policy` | `"audit" | "require_signature"` | `audit` | Global provider trust policy. |
| `min_lane` | `"verified" | "asserted"` | `verified` | Minimum evidence trust lane accepted. |

`require_signature` form:
```toml
[trust]
default_policy = { require_signature = { keys = ["key1.pub"] } }
```

### `[evidence]`
| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `allow_raw_values` | bool | `false` | Enables raw evidence disclosure. |
| `require_provider_opt_in` | bool | `true` | Providers must opt in via `allow_raw`. |

### `[provider_discovery]`
| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `allowlist` | array | `[]` | Optional allowlist for provider contract/schema disclosure. Empty means allow all. |
| `denylist` | array | `[]` | Provider IDs that must not be disclosed. |
| `max_response_bytes` | integer | `1048576` | Maximum response size for discovery tools. |

### `[anchors]`
| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `providers` | array | `[]` | Provider-specific anchor requirements. |

### `[[anchors.providers]]`
| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `provider_id` | string | yes | Provider identifier requiring anchors. |
| `anchor_type` | string | yes | Anchor type identifier expected in results. |
| `required_fields` | array | yes | Required fields in `anchor_value`. |

Anchor policy example (Asset Core):
```toml
[anchors]
[[anchors.providers]]
provider_id = "assetcore_read"
anchor_type = "assetcore.anchor_set"
required_fields = ["assetcore.namespace_id", "assetcore.commit_id", "assetcore.world_seq"]
```

### `[policy]`
| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `engine` | `"permit_all" | "deny_all" | "static"` | `permit_all` | Dispatch policy engine selection. |
| `static` | table | `null` | Static rule config (required when `engine = "static"`). |

Static policy example:
```toml
[policy]
engine = "static"

[policy.static]
default = "deny"

[[policy.static.rules]]
effect = "permit"
target_kinds = ["agent"]
require_labels = ["public"]
```

Static policy rule fields:
| Field | Type | Notes |
| --- | --- | --- |
| `effect` | `"permit" | "deny" | "error"` | Rule effect; `error` fails closed. |
| `error_message` | string | Required when `effect = "error"`. |
| `target_kinds` | array | Any of `agent`, `session`, `external`, `channel`. |
| `targets` | array | Explicit target selectors (see below). |
| `require_labels` | array | Visibility labels required to match. |
| `forbid_labels` | array | Visibility labels that block the match. |
| `require_policy_tags` | array | Policy tags required to match. |
| `forbid_policy_tags` | array | Policy tags that block the match. |
| `content_types` | array | Allowed content types. |
| `schema_ids` | array | Allowed schema IDs. |
| `packet_ids` | array | Allowed packet IDs. |
| `stage_ids` | array | Allowed stage IDs. |
| `scenario_ids` | array | Allowed scenario IDs. |

Target selector fields (`policy.static.rules.targets`):
| Field | Type | Notes |
| --- | --- | --- |
| `target_kind` | `"agent" | "session" | "external" | "channel"` | Target kind. |
| `target_id` | string | Agent/session/channel identifier. |
| `system` | string | External system name (external only). |
| `target` | string | External target identifier (external only). |

### `[validation]`
| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `strict` | bool | `true` | Reject invalid comparator/type combos at scenario definition and precheck. |
| `profile` | `"strict_core_v1"` | `strict_core_v1` | Strict comparator matrix/profile identifier. |
| `allow_permissive` | bool | `false` | Required when `strict = false` (explicit footgun opt-in). |
| `enable_lexicographic` | bool | `false` | Enables lexicographic comparator family (opt-in per schema). |
| `enable_deep_equals` | bool | `false` | Enables deep equality comparator family (opt-in per schema). |

Strict validation (default):
```toml
[validation]
strict = true
profile = "strict_core_v1"
```

Permissive validation (explicit opt-in):
```toml
[validation]
strict = false
allow_permissive = true
```

Optional comparator families:
```toml
[validation]
enable_lexicographic = true
enable_deep_equals = true
```

### `[run_state_store]`
| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `type` | `"memory" | "sqlite"` | `memory` | Store backend selection. |
| `path` | string | `null` | SQLite database path (required for `sqlite`). |
| `busy_timeout_ms` | integer | `5000` | SQLite busy timeout. |
| `journal_mode` | `"wal" | "delete"` | `wal` | SQLite journal mode. |
| `sync_mode` | `"full" | "normal"` | `full` | SQLite sync mode. |
| `max_versions` | integer | `null` | Optional max versions retained per run. |

SQLite example:
```toml
[run_state_store]
type = "sqlite"
path = "decision-gate.db"
journal_mode = "wal"
sync_mode = "full"
busy_timeout_ms = 5000
max_versions = 1000
```

### `[schema_registry]`
| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `type` | `"memory" | "sqlite"` | `memory` | Registry backend selection. |
| `path` | string | `null` | SQLite database path (required for `sqlite`). |
| `busy_timeout_ms` | integer | `5000` | SQLite busy timeout. |
| `journal_mode` | `"wal" | "delete"` | `wal` | SQLite journal mode. |
| `sync_mode` | `"full" | "normal"` | `full` | SQLite sync mode. |
| `max_schema_bytes` | integer | `1048576` | Maximum schema payload size. |
| `max_entries` | integer | `null` | Optional max schemas per tenant+namespace. |
| `acl` | table | `{ mode = "builtin" }` | Registry ACL configuration. |

### `[schema_registry.acl]`
| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `mode` | `"builtin" | "custom"` | `builtin` | Built-in role rules or custom ACL rules. |
| `default` | `"deny" | "allow"` | `deny` | Default decision when no rules match (custom only). |
| `require_signing` | bool | `false` | Require schema signing metadata on writes. |
| `rules` | array | `[]` | Custom ACL rules (only when `mode = "custom"`). |

Built-in ACL relies on `server.auth.principals` for role and policy_class
resolution. Without principals, registry access defaults to deny.

Custom ACL example:
```toml
[schema_registry.acl]
mode = "custom"
default = "deny"

[[schema_registry.acl.rules]]
effect = "allow"
actions = ["register", "list", "get"]
tenants = [1]
namespaces = [1]
roles = ["TenantAdmin", "NamespaceAdmin"]
```

### `[[providers]]`
Provider entries register built-in or MCP providers.

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `name` | string | yes | Provider identifier referenced by `provider_id`. |
| `type` | `"builtin" | "mcp"` | yes | Provider kind. |
| `command` | array | no | Stdio command for MCP providers. |
| `url` | string | no | HTTP endpoint for MCP providers. |
| `allow_insecure_http` | bool | `false` | Allow `http://` for MCP providers. |
| `capabilities_path` | string | yes (MCP) | Path to the provider contract JSON (capability contract). |
| `auth` | table | no | Bearer token for MCP providers. |
| `trust` | table | no | Per-provider trust override. |
| `allow_raw` | bool | `false` | Allow raw evidence disclosure for this provider. |
| `timeouts` | table | no | HTTP timeout overrides for MCP providers. |
| `config` | table | no | Built-in provider configuration blob. |

`auth` form:
```toml
auth = { bearer_token = "token" }
```

`trust` override form:
```toml
trust = { require_signature = { keys = ["provider.pub"] } }
```

`capabilities_path` example for MCP providers:
```toml
[[providers]]
name = "mongo"
type = "mcp"
command = ["mongo-provider", "--stdio"]
capabilities_path = "contracts/mongo_provider.json"
```

`timeouts` form (HTTP MCP providers):
```toml
timeouts = { connect_timeout_ms = 2000, request_timeout_ms = 10000 }
```

`timeouts` fields:
| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `connect_timeout_ms` | integer | `2000` | TCP/TLS connect timeout (100-10000). |
| `request_timeout_ms` | integer | `10000` | Total request timeout (500-30000, >= connect). |

HTTP provider example with timeouts:
```toml
[[providers]]
name = "ci"
type = "mcp"
url = "https://ci.example.com/rpc"
capabilities_path = "contracts/ci_provider.json"
timeouts = { connect_timeout_ms = 2000, request_timeout_ms = 10000 }
```

Timeout constraints:
- `connect_timeout_ms` must be between 100 and 10000.
- `request_timeout_ms` must be between 500 and 30000 and >= `connect_timeout_ms`.

## Built-In Provider Config
Built-in providers accept optional `config` blocks:

- `time`:
  - `allow_logical` (bool, default true)
- `env`:
  - `allowlist` (array)
  - `denylist` (array)
  - `max_value_bytes` (integer)
  - `max_key_bytes` (integer)
  - `overrides` (table)
- `json`:
  - `root` (string)
  - `max_bytes` (integer)
  - `allow_yaml` (bool)
- `http`:
  - `allow_http` (bool)
  - `timeout_ms` (integer)
  - `max_response_bytes` (integer)
  - `allowed_hosts` (array)
  - `user_agent` (string)
  - `hash_algorithm` (string)
