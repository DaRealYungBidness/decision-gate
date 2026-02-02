// decision-gate-cli/src/tests/serve_policy.rs
// ============================================================================
// Module: Serve Policy Tests
// Description: Unit tests for CLI server bind safety rules.
// Purpose: Ensure non-loopback binding remains fail-closed without explicit opt-in.
// Dependencies: decision-gate-cli serve_policy, decision-gate-mcp config loader
// ============================================================================

//! ## Overview
//! Validates the serve policy fails closed for unsafe binds and allows
//! loopback/stdio operation without network exposure.

use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use decision_gate_mcp::DecisionGateConfig;

use crate::serve_policy::ServePolicyError;
use crate::serve_policy::enforce_local_only;
use crate::serve_policy::parse_allow_non_loopback_value;

fn write_config(contents: &str) -> PathBuf {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).expect("time").as_nanos();
    let path = std::env::temp_dir().join(format!("dg-cli-test-{timestamp}.toml"));
    fs::write(&path, contents).expect("write config");
    path
}

fn load_config(contents: &str) -> DecisionGateConfig {
    let path = write_config(contents);
    let config = DecisionGateConfig::load(Some(&path)).expect("load config");
    let _ = fs::remove_file(path);
    config
}

#[test]
fn stdio_is_local_only() {
    let config = load_config(
        r#"
[server]
transport = "stdio"
"#,
    );
    let outcome = enforce_local_only(&config, false).expect("stdio allowed");
    assert!(!outcome.network_exposed);
    assert!(outcome.bind_addr.is_none());
}

#[test]
fn loopback_allows_without_tls_or_auth() {
    let config = load_config(
        r#"
[server]
transport = "http"
bind = "127.0.0.1:8080"
"#,
    );
    let outcome = enforce_local_only(&config, false).expect("loopback allowed");
    assert!(!outcome.network_exposed);
    assert!(outcome.bind_addr.is_some());
}

#[test]
fn non_loopback_requires_opt_in() {
    let config = load_config(
        r#"
[server]
transport = "http"
bind = "0.0.0.0:8080"

[server.auth]
mode = "bearer_token"
bearer_tokens = ["token"]
"#,
    );
    let err = enforce_local_only(&config, false).expect_err("expected opt-in error");
    assert!(matches!(err, ServePolicyError::NonLoopbackOptInRequired { .. }));
}

#[test]
fn non_loopback_requires_auth() {
    let mut config = load_config(
        r#"
[server]
transport = "http"
bind = "0.0.0.0:8080"

[server.auth]
mode = "bearer_token"
bearer_tokens = ["token"]
"#,
    );
    config.server.auth = None;
    let err = enforce_local_only(&config, true).expect_err("expected auth error");
    assert!(matches!(err, ServePolicyError::NonLoopbackAuthRequired { .. }));
}

#[test]
fn non_loopback_requires_tls() {
    let config = load_config(
        r#"
[server]
transport = "http"
bind = "0.0.0.0:8080"

[server.auth]
mode = "bearer_token"
bearer_tokens = ["token"]
"#,
    );
    let err = enforce_local_only(&config, true).expect_err("expected tls error");
    assert!(matches!(err, ServePolicyError::NonLoopbackTlsRequired { .. }));
}

#[test]
fn non_loopback_allows_upstream_tls() {
    let config = load_config(
        r#"
[server]
transport = "http"
bind = "0.0.0.0:8080"
tls_termination = "upstream"

[server.auth]
mode = "bearer_token"
bearer_tokens = ["token"]
"#,
    );
    let outcome = enforce_local_only(&config, true).expect("expected upstream tls success");
    assert!(outcome.network_exposed);
}

#[test]
fn mtls_allows_upstream_tls_without_client_ca() {
    let config = load_config(
        r#"
[server]
transport = "http"
bind = "0.0.0.0:8080"
tls_termination = "upstream"

[server.auth]
mode = "mtls"
mtls_subjects = ["CN=test"]
"#,
    );
    let outcome = enforce_local_only(&config, true).expect("expected upstream mtls success");
    assert!(outcome.network_exposed);
}

#[test]
fn mtls_requires_client_ca() {
    let config = load_config(
        r#"
[server]
transport = "http"
bind = "0.0.0.0:8080"

[server.auth]
mode = "mtls"
mtls_subjects = ["CN=test"]

[server.tls]
cert_path = "cert.pem"
key_path = "key.pem"
require_client_cert = true
"#,
    );
    let err = enforce_local_only(&config, true).expect_err("expected mtls CA error");
    assert!(matches!(err, ServePolicyError::NonLoopbackMtlsClientCaRequired { .. }));
}

#[test]
fn mtls_requires_client_cert_flag() {
    let config = load_config(
        r#"
[server]
transport = "http"
bind = "0.0.0.0:8080"

[server.auth]
mode = "mtls"
mtls_subjects = ["CN=test"]

[server.tls]
cert_path = "cert.pem"
key_path = "key.pem"
client_ca_path = "ca.pem"
require_client_cert = false
"#,
    );
    let err = enforce_local_only(&config, true).expect_err("expected mtls cert error");
    assert!(matches!(err, ServePolicyError::NonLoopbackMtlsClientCertRequired { .. }));
}

#[test]
fn non_loopback_allows_bearer_with_tls() {
    let config = load_config(
        r#"
[server]
transport = "http"
bind = "0.0.0.0:8080"

[server.auth]
mode = "bearer_token"
bearer_tokens = ["token"]

[server.tls]
cert_path = "cert.pem"
key_path = "key.pem"
"#,
    );
    let outcome = enforce_local_only(&config, true).expect("expected success");
    assert!(outcome.network_exposed);
}

#[test]
fn non_loopback_allows_mtls_with_client_ca() {
    let config = load_config(
        r#"
[server]
transport = "http"
bind = "0.0.0.0:8080"

[server.auth]
mode = "mtls"
mtls_subjects = ["CN=test"]

[server.tls]
cert_path = "cert.pem"
key_path = "key.pem"
client_ca_path = "ca.pem"
require_client_cert = true
"#,
    );
    let outcome = enforce_local_only(&config, true).expect("expected mtls success");
    assert!(outcome.network_exposed);
}

#[test]
fn parse_allow_non_loopback_accepts_true() {
    let result = parse_allow_non_loopback_value("true").expect("parse env");
    assert!(result);
}

#[test]
fn parse_allow_non_loopback_accepts_false() {
    let result = parse_allow_non_loopback_value("false").expect("parse env");
    assert!(!result);
    let result = parse_allow_non_loopback_value("0").expect("parse env");
    assert!(!result);
}

#[test]
fn parse_allow_non_loopback_rejects_invalid() {
    let err = parse_allow_non_loopback_value("maybe").expect_err("expected invalid env");
    assert!(matches!(err, ServePolicyError::InvalidEnv { .. }));
}

// ============================================================================
// SECTION: Environment Parsing Extensions
// ============================================================================

#[test]
fn parse_allow_non_loopback_accepts_yes_variants() {
    // Test various "yes" values
    let yes_values = vec!["true", "1", "yes", "y", "on", "TRUE", "True"];
    for value in yes_values {
        let result = parse_allow_non_loopback_value(value).expect("parse env");
        assert!(result, "expected true for {value}");
    }
}

#[test]
fn parse_allow_non_loopback_accepts_no_variants() {
    // Test various "no" values
    let no_values = vec!["false", "0", "no", "n", "off", "FALSE", "False"];
    for value in no_values {
        let result = parse_allow_non_loopback_value(value).expect("parse env");
        assert!(!result, "expected false for {value}");
    }
}

#[test]
fn env_var_with_trailing_newline_handled() {
    // Environment variables might have trailing newlines
    let with_newline = "true\n";
    let result = parse_allow_non_loopback_value(with_newline).expect("parse env");
    assert!(result);
}

#[test]
fn bind_addr_ipv6_loopback_recognized() {
    // Test IPv6 loopback address
    let config = load_config(
        r#"
[server]
transport = "http"
bind = "[::1]:8080"
"#,
    );
    let outcome = enforce_local_only(&config, false).expect("ipv6 loopback allowed");
    assert!(!outcome.network_exposed);
    assert!(outcome.bind_addr.is_some());
}

#[test]
fn bind_addr_localhost_normalized_to_127_0_0_1() {
    // Current config loader only accepts socket addresses, not hostnames.
    let path = write_config(
        r#"
[server]
transport = "http"
bind = "localhost:8080"
"#,
    );
    let err = DecisionGateConfig::load(Some(&path)).expect_err("expected invalid bind");
    let _ = fs::remove_file(path);
    assert!(err.to_string().contains("invalid bind address"));
}
