// decision-gate-config/src/examples.rs
// ============================================================================
// Module: Config Examples
// Description: Canonical example configuration payloads.
// Purpose: Deterministic examples for docs and tooling.
// Dependencies: std
// ============================================================================

//! ## Overview
//! Canonical examples for Decision Gate configuration. Outputs are
//! deterministic and kept in sync with schema and docs.
//!
//! Security posture: examples are static templates; see
//! `Docs/security/threat_model.md`.

/// Returns a canonical example `decision-gate.toml` configuration.
#[must_use]
pub fn config_toml_example() -> String {
    String::from(
        r#"[server]
transport = "stdio"
mode = "strict"
max_body_bytes = 1048576

[namespace]
allow_default = false

[trust]
default_policy = "audit"
min_lane = "verified"

[evidence]
allow_raw_values = false
require_provider_opt_in = true

[policy]
engine = "permit_all"

[runpack_storage]
type = "object_store"
provider = "s3"
bucket = "decision-gate-runpacks"
prefix = "decision-gate/runpacks"
# endpoint = "https://s3.example.com"
# force_path_style = false
# allow_http = false

[run_state_store]
type = "sqlite"
path = "decision-gate.db"
journal_mode = "wal"
sync_mode = "full"
busy_timeout_ms = 5000
max_versions = 1000

[[providers]]
name = "time"
type = "builtin"

[[providers]]
name = "env"
type = "builtin"

[[providers]]
name = "json"
type = "builtin"
config = { root = "/etc/decision-gate", max_bytes = 1048576, allow_yaml = true }

[[providers]]
name = "http"
type = "builtin"
config = { allow_http = false, timeout_ms = 5000, max_response_bytes = 1048576, allowed_hosts = ["api.example.com"], user_agent = "decision-gate/0.1", hash_algorithm = "sha256" }
"#,
    )
}
