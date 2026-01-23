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
| `transport` | `"stdio" | "http" | "sse"` | `stdio` | HTTP/SSE are loopback-only. |
| `bind` | string | `null` | Required for HTTP/SSE; must be a loopback address. |
| `max_body_bytes` | integer | `1048576` | Maximum JSON-RPC request size. |

### `[trust]`
| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `default_policy` | `"audit" | "require_signature"` | `audit` | Global provider trust policy. |

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

### `[[providers]]`
Provider entries register built-in or MCP providers.

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `name` | string | yes | Provider identifier referenced by `provider_id`. |
| `type` | `"builtin" | "mcp"` | yes | Provider kind. |
| `command` | array | no | Stdio command for MCP providers. |
| `url` | string | no | HTTP endpoint for MCP providers. |
| `allow_insecure_http` | bool | `false` | Allow `http://` for MCP providers. |
| `capabilities_path` | string | yes (MCP) | Path to the provider capability contract JSON. |
| `auth` | table | no | Bearer token for MCP providers. |
| `trust` | table | no | Per-provider trust override. |
| `allow_raw` | bool | `false` | Allow raw evidence disclosure for this provider. |
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
