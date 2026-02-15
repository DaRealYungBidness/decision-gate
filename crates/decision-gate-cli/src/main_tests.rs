// crates/decision-gate-cli/src/main_tests.rs
// ============================================================================
// Module: CLI Main Helpers Tests
// Description: Unit tests for file read size enforcement in the CLI entry point.
// Purpose: Ensure bounded reads fail closed on oversized inputs.
// Dependencies: decision-gate-cli main helpers
// ============================================================================

//! ## Overview
//! Validates `read_bytes_with_limit` enforces size limits for CLI inputs.
//!
//! Security posture: CLI inputs are untrusted; size limits must fail closed.

#![allow(
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::use_debug,
    clippy::dbg_macro,
    clippy::panic_in_result_fn,
    clippy::unwrap_in_result,
    reason = "Test-only output and panic-based assertions are permitted."
)]

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use super::DecisionGateConfig;
use super::McpClientArgs;
use super::McpTransportArg;
use super::ReadLimitError;
use super::apply_json_root_override;
use super::load_auth_profiles;
use super::parse_namespace_id;
use super::parse_stdio_env;
use super::parse_tenant_id;
use super::read_bytes_with_limit;
use super::resolve_auth;
use super::resolve_auth_config_path;

// ============================================================================
// SECTION: Helpers
// ============================================================================

fn temp_file(label: &str) -> PathBuf {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock drift").as_nanos();
    let mut path = std::env::temp_dir();
    path.push(format!("decision-gate-cli-{label}-{nanos}.bin"));
    path
}

fn cleanup(path: &PathBuf) {
    let _ = fs::remove_file(path);
}

fn temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock drift").as_nanos();
    let mut path = std::env::temp_dir();
    path.push(format!("decision-gate-cli-{label}-{nanos}"));
    fs::create_dir_all(&path).expect("create temp directory");
    path
}

fn cleanup_dir(path: &PathBuf) {
    let _ = fs::remove_dir_all(path);
}

fn base_mcp_args() -> McpClientArgs {
    McpClientArgs {
        transport: McpTransportArg::Http,
        endpoint: Some("http://127.0.0.1:8080/rpc".to_string()),
        stdio_command: None,
        stdio_args: Vec::new(),
        stdio_env: Vec::new(),
        stdio_config: None,
        timeout_ms: 5_000,
        bearer_token: None,
        client_subject: None,
        auth_profile: None,
        auth_config: None,
    }
}

// ============================================================================
// SECTION: Tests
// ============================================================================

#[test]
fn read_bytes_with_limit_allows_small_file() {
    let path = temp_file("io-small");
    fs::write(&path, b"ok").expect("write small file");

    let bytes = read_bytes_with_limit(&path, 16).expect("read small file");
    assert_eq!(bytes, b"ok");

    cleanup(&path);
}

#[test]
fn read_bytes_with_limit_rejects_large_file() {
    let path = temp_file("io-large");
    let limit = 8_usize;
    let payload = vec![0_u8; limit + 1];
    fs::write(&path, payload).expect("write large file");

    let err = read_bytes_with_limit(&path, limit).expect_err("expected size limit failure");
    match err {
        ReadLimitError::TooLarge {
            size,
            limit: reported,
        } => {
            let limit_u64 = u64::try_from(limit).expect("limit fits");
            assert!(size > limit_u64);
            assert_eq!(reported, limit);
        }
        ReadLimitError::Io(err) => panic!("unexpected IO error: {err}"),
    }

    cleanup(&path);
}

#[test]
fn parse_stdio_env_accepts_key_value() {
    let env = parse_stdio_env(&[String::from("KEY=VALUE")]).expect("parse env");
    assert_eq!(env, vec![("KEY".to_string(), "VALUE".to_string())]);
}

#[test]
fn parse_stdio_env_rejects_invalid_entry() {
    let err = parse_stdio_env(&[String::from("INVALID")]).expect_err("expected error");
    assert!(err.to_string().contains("Invalid stdio env var"));
}

#[test]
fn parse_stdio_env_rejects_empty_key() {
    let err = parse_stdio_env(&[String::from("=VALUE")]).expect_err("expected error");
    assert!(err.to_string().contains("Invalid stdio env var"));
}

#[test]
fn parse_stdio_env_rejects_nul() {
    let err = parse_stdio_env(&[String::from("KEY\0BAD=VALUE")]).expect_err("expected error");
    assert!(err.to_string().contains("Invalid stdio env var"));
}

#[test]
fn load_auth_profiles_parses_config() {
    let path = temp_file("auth-profiles");
    let payload = r#"[client.auth_profiles.demo]
bearer_token = "token-1"
client_subject = "subject-1"
"#;
    fs::write(&path, payload).expect("write config");

    let profiles = load_auth_profiles(&path).expect("load profiles");
    let profile = profiles.get("demo").expect("demo profile");
    assert_eq!(profile.bearer_token.as_deref(), Some("token-1"));
    assert_eq!(profile.client_subject.as_deref(), Some("subject-1"));

    cleanup(&path);
}

#[test]
fn resolve_auth_prefers_cli_over_profile() {
    let path = temp_file("auth-profile-override");
    let payload = r#"[client.auth_profiles.demo]
bearer_token = "profile-token"
client_subject = "profile-subject"
"#;
    fs::write(&path, payload).expect("write config");

    let mut args = base_mcp_args();
    args.auth_profile = Some("demo".to_string());
    args.auth_config = Some(path.clone());
    args.bearer_token = Some("cli-token".to_string());

    let resolved = resolve_auth(&args).expect("resolve auth");
    assert_eq!(resolved.bearer_token.as_deref(), Some("cli-token"));
    assert_eq!(resolved.client_subject.as_deref(), Some("profile-subject"));

    cleanup(&path);
}

#[test]
fn resolve_auth_config_path_defaults_to_decision_gate_toml() {
    let path = resolve_auth_config_path(None);
    assert_eq!(path, PathBuf::from("decision-gate.toml"));
}

#[test]
fn resolve_auth_config_path_prefers_explicit_path() {
    let path = PathBuf::from("custom-config.toml");
    let resolved = resolve_auth_config_path(Some(&path));
    assert_eq!(resolved, path);
}

#[test]
fn load_auth_profiles_missing_file_errors() {
    let missing = PathBuf::from("/nonexistent/auth-config.toml");
    let err = load_auth_profiles(&missing).expect_err("expected missing file error");
    assert!(
        err.to_string().contains(missing.to_string_lossy().as_ref()),
        "error should include path"
    );
}

#[test]
fn load_auth_profiles_invalid_toml_errors() {
    let path = temp_file("auth-invalid");
    fs::write(&path, "not valid toml ==").expect("write invalid toml");
    let err = load_auth_profiles(&path).expect_err("expected parse error");
    assert!(err.to_string().contains("Failed to parse auth config"));
    cleanup(&path);
}

#[test]
fn parse_tenant_id_rejects_zero() {
    let err = parse_tenant_id(0).expect_err("expected tenant id error");
    assert!(err.to_string().contains("tenant_id"));
}

#[test]
fn parse_namespace_id_rejects_zero() {
    let err = parse_namespace_id(0).expect_err("expected namespace id error");
    assert!(err.to_string().contains("namespace_id"));
}

#[test]
fn parse_tenant_id_accepts_max() {
    let tenant = parse_tenant_id(u64::MAX).expect("tenant id max");
    assert!(tenant.get() > 0);
}

#[test]
fn parse_namespace_id_accepts_max() {
    let namespace = parse_namespace_id(u64::MAX).expect("namespace id max");
    assert!(namespace.get() > 0);
}

// ============================================================================
// SECTION: Size Limit Enforcement Tests
// ============================================================================

#[test]
fn auth_config_size_limit_enforced() {
    use super::MAX_AUTH_CONFIG_BYTES;

    // Create a file exactly 1 byte over the limit
    let path = temp_file("auth-oversized");
    let data = vec![b'x'; MAX_AUTH_CONFIG_BYTES + 1];
    fs::write(&path, data).expect("write oversized file");

    // Attempt to load should fail
    let result = load_auth_profiles(&path);

    cleanup(&path);

    assert!(result.is_err(), "Auth config over size limit should be rejected");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("too large") || err_msg.contains("size") || err_msg.contains("large"),
        "Error should mention size limit: {err_msg}"
    );
}

#[test]
fn auth_config_at_exact_limit_accepted() {
    use super::MAX_AUTH_CONFIG_BYTES;

    // Create a file exactly at the limit (valid TOML)
    let path = temp_file("auth-at-limit");

    // Create a valid TOML that's close to the limit
    let mut data = b"[client.auth_profiles.test]\nbearer_token = \"".to_vec();
    // Pad to get close to MAX_AUTH_CONFIG_BYTES
    let remaining = MAX_AUTH_CONFIG_BYTES - data.len() - 2; // -2 for closing quote and newline
    data.extend(vec![b'x'; remaining]);
    data.extend_from_slice(b"\"\n");

    fs::write(&path, &data).expect("write at-limit file");

    // Should succeed (though TOML parsing might fail for other reasons)
    let result = load_auth_profiles(&path);

    cleanup(&path);

    // The file should be read successfully (may fail TOML parsing, but not size check)
    match result {
        Ok(_) => { /* Size limit check passed */ }
        Err(e) => {
            let err_msg = e.to_string();
            // Should not be a size error
            assert!(
                !err_msg.contains("too large") && !err_msg.contains("size limit"),
                "Should not fail on size limit at exactly MAX_AUTH_CONFIG_BYTES"
            );
        }
    }
}

#[test]
fn apply_json_root_override_requires_root_for_root_id() {
    let config_text = r#"
[server]
transport = "stdio"

[[providers]]
name = "json"
type = "builtin"
config = { root = ".", root_id = "evidence-root", max_bytes = 1048576, allow_yaml = true }
"#;
    let mut config: DecisionGateConfig = toml::from_str(config_text).expect("parse config");
    config.validate().expect("validate config");

    let err = apply_json_root_override(&mut config, None, Some("override-root"))
        .expect_err("expected override validation error");
    assert!(err.contains("--json-root-id requires --json-root"));
}

#[test]
fn apply_json_root_override_rejects_missing_json_provider() {
    let root = temp_dir("json-root-missing-provider");
    let config_text = r#"
[server]
transport = "stdio"

[[providers]]
name = "env"
type = "builtin"
"#;
    let mut config: DecisionGateConfig = toml::from_str(config_text).expect("parse config");
    config.validate().expect("validate config");

    let err = apply_json_root_override(&mut config, Some(root.as_path()), None)
        .expect_err("expected missing provider error");
    assert!(err.contains("built-in provider 'json'"));

    cleanup_dir(&root);
}

#[test]
fn apply_json_root_override_updates_builtin_json_root_and_id() {
    let configured_root = temp_dir("json-root-configured");
    let override_root = temp_dir("json-root-override");
    let escaped = configured_root.to_string_lossy().replace('\\', "\\\\");
    let config_text = format!(
        r#"
[server]
transport = "stdio"

[[providers]]
name = "json"
type = "builtin"
config = {{ root = "{escaped}", root_id = "evidence-root", max_bytes = 1048576, allow_yaml = true }}
"#
    );
    let mut config: DecisionGateConfig = toml::from_str(&config_text).expect("parse config");
    config.validate().expect("validate config");

    apply_json_root_override(
        &mut config,
        Some(override_root.as_path()),
        Some("arxi-evidence-root"),
    )
    .expect("apply override");

    let provider =
        config.providers.iter().find(|provider| provider.name == "json").expect("json provider");
    let table =
        provider.config.as_ref().and_then(toml::Value::as_table).expect("json config table");
    let actual_root = table.get("root").and_then(toml::Value::as_str).expect("json root string");
    let actual_root_id =
        table.get("root_id").and_then(toml::Value::as_str).expect("json root id string");
    let canonical_override = fs::canonicalize(&override_root).expect("canonical override root");
    assert_eq!(actual_root, canonical_override.to_string_lossy());
    assert_eq!(actual_root_id, "arxi-evidence-root");

    cleanup_dir(&configured_root);
    cleanup_dir(&override_root);
}

#[test]
fn apply_json_root_override_rejects_invalid_root_id() {
    let root = temp_dir("json-root-invalid-id");
    let config_text = r#"
[server]
transport = "stdio"

[[providers]]
name = "json"
type = "builtin"
config = { root = ".", root_id = "evidence-root", max_bytes = 1048576, allow_yaml = true }
"#;
    let mut config: DecisionGateConfig = toml::from_str(config_text).expect("parse config");
    config.validate().expect("validate config");

    let err = apply_json_root_override(&mut config, Some(root.as_path()), Some("INVALID"))
        .expect_err("expected invalid root id");
    assert!(err.contains("--json-root-id"));

    cleanup_dir(&root);
}
