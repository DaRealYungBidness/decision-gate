<!--
Docs/configuration/decision-gate.toml.md
============================================================================
Document: Decision Gate MCP Configuration
Description: Reference for decision-gate.toml configuration fields.
Purpose: Document server, trust, evidence, and provider settings.
Generated: This file is auto-generated; do not edit manually.
============================================================================
-->

# decision-gate.toml Configuration

## Overview

`decision-gate.toml` configures the MCP server, trust policies, evidence
disclosure defaults, and provider registry. All inputs are validated and
fail closed on errors.

## Top-Level Sections

### [server]

Server transport, auth, limits, and audit settings.

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `transport` | "stdio" \| "http" \| "sse" | stdio | Transport protocol for MCP. |
| `mode` | "strict" \| "dev_permissive" | strict | Operational mode for MCP (dev_permissive is legacy). |
| `bind` | string | null | Bind address for HTTP/SSE transport. |
| `max_body_bytes` | integer | 1048576 | Maximum JSON-RPC request size in bytes. |
| `limits` | table | { max_inflight = 256 } | Request limits for MCP server. |
| `auth` | table | null | Inbound authentication configuration for MCP tool calls. |
| `tls` | table | null | TLS configuration for HTTP/SSE transports. |
| `audit` | table | { enabled = true } | Structured audit logging configuration. |

HTTP/SSE require `bind`; non-loopback requires explicit CLI opt-in plus TLS + non-local auth.

### [server.auth]

Inbound authn/authz for MCP tool calls.

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `mode` | "local_only" \| "bearer_token" \| "mtls" | local_only | Inbound auth mode for MCP tool calls. |
| `bearer_tokens` | array | [] | Allowed bearer tokens. |
| `mtls_subjects` | array | [] | Allowed mTLS subjects (via trusted proxy header). |
| `allowed_tools` | array | [] | Optional tool allowlist for inbound calls. |
| `principals` | array | [] | Optional principal-to-role mappings. |

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

When using `mtls` mode, the server expects the `x-decision-gate-client-subject` header from a trusted TLS-terminating proxy.

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

Built-in registry ACL expects `policy_class` values like `prod`, `project`, or `scratch` (case-insensitive). Unknown values are treated as `prod`.

### [server.audit]

Structured audit logging configuration.

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `enabled` | bool | true | Enable structured audit logging (JSON lines). |
| `path` | string | null | Audit log path (JSON lines). |
| `log_precheck_payloads` | bool | false | Log raw precheck payloads (explicit opt-in). |

### [server.limits]

Request concurrency and rate limits.

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `max_inflight` | integer | 256 | Maximum concurrent MCP requests. |
| `rate_limit` | table | null | Optional rate limit configuration. |

### [server.limits.rate_limit]

Optional token-bucket style rate limit configuration.

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `max_requests` | integer | 1000 | Maximum requests per rate limit window. |
| `window_ms` | integer | 1000 | Rate limit window in milliseconds. |
| `max_entries` | integer | 4096 | Maximum distinct rate limit entries. |

### [server.tls]

TLS configuration for HTTP/SSE transports.

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `cert_path` | string | n/a | Server TLS certificate (PEM). |
| `key_path` | string | n/a | Server TLS private key (PEM). |
| `client_ca_path` | string | null | Optional client CA bundle for mTLS. |
| `require_client_cert` | bool | true | Require client certificate for mTLS. |

### [dev]

Explicit dev-permissive overrides (opt-in only).

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `permissive` | bool | false | Enable dev-permissive mode (explicit opt-in). |
| `permissive_scope` | "asserted_evidence_only" | asserted_evidence_only | Dev-permissive scope selection. |
| `permissive_ttl_days` | integer | null | Optional TTL for dev-permissive warnings (days). |
| `permissive_warn` | bool | true | Emit warnings when dev-permissive enabled/expired. |
| `permissive_exempt_providers` | array | ["assetcore_read", "assetcore"] | Providers exempt from dev-permissive relaxations. |

Dev-permissive is rejected when `namespace.authority.mode = "assetcore_http"`.

### [namespace]

Namespace allowlist and authority selection.

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `allow_default` | bool | false | Allow the default namespace ID (1). |
| `default_tenants` | array | [] | Tenant allowlist required when allow_default is true. |
| `authority` | table | { mode = "none" } | Namespace authority backend selection. |

### [namespace.authority]

Namespace authority backend configuration.

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `mode` | "none" \| "assetcore_http" | none | Namespace authority backend selection. |
| `assetcore` | table | null | Asset Core namespace authority settings. |

### [namespace.authority.assetcore]

Asset Core namespace authority settings.

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `base_url` | string | n/a | Asset Core write-daemon base URL. |
| `auth_token` | string | null | Optional bearer token for namespace lookup. |
| `connect_timeout_ms` | integer | 500 | HTTP connect timeout (ms). |
| `request_timeout_ms` | integer | 2000 | HTTP request timeout (ms). |

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

### [trust]

Trust lane defaults and provider signature enforcement.

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `default_policy` | "audit" | audit | Default trust policy for providers. |
| `min_lane` | "verified" \| "asserted" | verified | Minimum evidence trust lane accepted. |

`require_signature` form:

```toml
[trust]
default_policy = { require_signature = { keys = ["key1.pub"] } }
```

### [evidence]

Evidence disclosure policy defaults.

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `allow_raw_values` | bool | false | Allow raw evidence values to be disclosed. |
| `require_provider_opt_in` | bool | true | Require provider opt-in for raw disclosure. |

### [provider_discovery]

Provider contract/schema disclosure controls.

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `allowlist` | array | [] | Optional allowlist for provider disclosure. |
| `denylist` | array | [] | Provider identifiers denied for disclosure. |
| `max_response_bytes` | integer | 1048576 | Maximum response size for provider discovery tools. |

### [anchors]

Evidence anchor policy configuration.

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `providers` | array | [] | Provider-specific anchor requirements. |

### [[anchors.providers]]

Provider-specific anchor requirements.

| Field | Type | Required | Default | Notes |
| --- | --- | --- | --- | --- |
| `provider_id` | string | yes | n/a | Provider identifier requiring anchors. |
| `anchor_type` | string | yes | n/a | Anchor type identifier expected in results. |
| `required_fields` | array | yes | n/a | Required fields in anchor_value. |

Anchor policy example (Asset Core):

```toml
[anchors]
[[anchors.providers]]
provider_id = "assetcore_read"
anchor_type = "assetcore.anchor_set"
required_fields = ["assetcore.namespace_id", "assetcore.commit_id", "assetcore.world_seq"]
```

### [policy]

Dispatch policy engine selection.

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `engine` | "permit_all" \| "deny_all" \| "static" | permit_all | Dispatch policy engine selection. |
| `static` | table | null | Static dispatch policy rules. |

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

### [policy.static]

Static dispatch policy rules.

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `default` | "permit" \| "deny" | deny | Default decision when no rules match. |
| `rules` | array | [] | Ordered list of static policy rules. |

### [[policy.static.rules]]

Static policy rule fields.

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `effect` | "permit" \| "deny" \| "error" | n/a | Rule effect. |
| `error_message` | string | null | Error message when effect is 'error'. |
| `target_kinds` | array | [] | Target kinds that may receive the packet. |
| `targets` | array | [] | Specific target selectors. |
| `require_labels` | array | [] | Visibility labels required to match. |
| `forbid_labels` | array | [] | Visibility labels that block a match. |
| `require_policy_tags` | array | [] | Policy tags required to match. |
| `forbid_policy_tags` | array | [] | Policy tags that block a match. |
| `content_types` | array | [] | Allowed content types. |
| `schema_ids` | array | [] | Allowed schema identifiers. |
| `packet_ids` | array | [] | Allowed packet identifiers. |
| `stage_ids` | array | [] | Allowed stage identifiers. |
| `scenario_ids` | array | [] | Allowed scenario identifiers. |

Target selector fields (`policy.static.rules.targets`):

| Field | Type | Notes |
| --- | --- | --- |
| `target_kind` | "agent" \| "session" \| "external" \| "channel" | Target kind. |
| `target_id` | string | Agent/session/channel identifier. |
| `system` | string | External system name (external only). |
| `target` | string | External target identifier (external only). |

### [validation]

Comparator validation policy for scenarios and prechecks.

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `strict` | bool | true | Enforce strict comparator validation. |
| `profile` | "strict_core_v1" | strict_core_v1 | Strict comparator profile identifier. |
| `allow_permissive` | bool | false | Explicit opt-in for permissive validation. |
| `enable_lexicographic` | bool | false | Enable lexicographic comparators (opt-in per schema). |
| `enable_deep_equals` | bool | false | Enable deep equality comparators (opt-in per schema). |

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

### [runpack_storage]

Runpack storage configuration.

| Field | Type | Required | Default | Notes |
| --- | --- | --- | --- | --- |
| `type` | "object_store" | yes | n/a | Runpack storage backend selection. |
| `provider` | "s3" | yes | n/a | Object-store provider. |
| `bucket` | string | yes | n/a | Bucket name for runpack storage. |
| `region` | string | no | null | Optional S3 region override. |
| `endpoint` | string | no | null | Optional S3-compatible endpoint. |
| `prefix` | string | no | null | Optional key prefix inside the bucket. |
| `force_path_style` | bool | no | false | Force path-style addressing (S3-compatible). |
| `allow_http` | bool | no | false | Allow non-TLS endpoints (explicit opt-in). |

### [run_state_store]

Run state persistence settings.

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `type` | "memory" \| "sqlite" | memory | Run state store backend selection. |
| `path` | string | null | SQLite database path. |
| `busy_timeout_ms` | integer | 5000 | SQLite busy timeout (ms). |
| `journal_mode` | "wal" \| "delete" | wal | SQLite journal mode. |
| `sync_mode` | "full" \| "normal" | full | SQLite sync mode. |
| `max_versions` | integer | null | Optional max versions retained per run. |

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

### [schema_registry]

Schema registry persistence and limits.

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `type` | "memory" \| "sqlite" | memory | Schema registry backend selection. |
| `path` | string | null | SQLite database path. |
| `busy_timeout_ms` | integer | 5000 | SQLite busy timeout (ms). |
| `journal_mode` | "wal" \| "delete" | wal | SQLite journal mode. |
| `sync_mode` | "full" \| "normal" | full | SQLite sync mode. |
| `max_schema_bytes` | integer | 1048576 | Maximum schema payload size in bytes. |
| `max_entries` | integer | null | Optional max schemas per tenant + namespace. |
| `acl` | table | { mode = "builtin" } | Schema registry ACL configuration. |

### [schema_registry.acl]

Schema registry ACL configuration.

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `mode` | "builtin" \| "custom" | builtin | Built-in role rules or custom ACL rules. |
| `default` | "deny" \| "allow" | deny | Default decision when no rules match (custom only). |
| `require_signing` | bool | false | Require schema signing metadata on writes. |
| `rules` | array | [] | Custom ACL rules (mode = custom). |

Built-in ACL relies on `server.auth.principals` for role and policy_class resolution. Without principals, registry access defaults to deny.

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

### [[schema_registry.acl.rules]]

Custom ACL rule fields.

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `effect` | "allow" \| "deny" | n/a | Rule effect. |
| `actions` | array | [] | Registry actions covered by the rule. |
| `tenants` | array | [] | Tenant identifier scope. |
| `namespaces` | array | [] | Namespace identifier scope. |
| `subjects` | array | [] | Principal subjects in scope. |
| `roles` | array | [] | Role names in scope. |
| `policy_classes` | array | [] | Policy class labels in scope. |

### [[providers]]

Provider entries register built-in or MCP providers.

| Field | Type | Required | Default | Notes |
| --- | --- | --- | --- | --- |
| `name` | string | yes | n/a | Provider identifier. |
| `type` | "builtin" \| "mcp" | yes | n/a | Provider kind. |
| `command` | array | no | [] |  |
| `url` | string | no | null | Provider HTTP URL. |
| `allow_insecure_http` | bool | no | false | Allow http:// URLs for MCP providers. |
| `capabilities_path` | string | no | null | Path to provider capability contract JSON. |
| `auth` | table | no | null |  |
| `trust` | unknown | no | null | Default trust policy for providers. |
| `allow_raw` | bool | no | false | Allow raw evidence disclosure for this provider. |
| `timeouts` | table | no | { connect_timeout_ms = 2000, request_timeout_ms = 10000 } | HTTP timeout overrides for MCP providers. |
| `config` | json | no | null | Provider-specific config blob. |

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

### [providers.timeouts]

Timeout overrides for HTTP MCP providers.

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `connect_timeout_ms` | integer | 2000 | TCP/TLS connect timeout (ms). |
| `request_timeout_ms` | integer | 10000 | Total request timeout (ms). |

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
