// enterprise/decision-gate-enterprise/tests/config.rs
// ============================================================================
// Module: Enterprise Config Tests
// Description: Unit tests for enterprise config loading and wiring.
// Purpose: Validate config validation and wiring behavior without services.
// ============================================================================

//! Enterprise config unit tests.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    reason = "Test helpers use explicit panics for setup clarity."
)]

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use decision_gate_enterprise::config::EnterpriseConfig;
use decision_gate_enterprise::config::EnterpriseConfigError;
use decision_gate_enterprise::config::EnterpriseRunpackConfig;
use decision_gate_enterprise::config::EnterpriseStorageConfig;
use decision_gate_enterprise::config::EnterpriseUsageConfig;
use decision_gate_enterprise::config::UsageLedgerType;
use decision_gate_mcp::McpNoopAuditSink;
use decision_gate_mcp::NoopTenantAuthorizer;
use tempfile::NamedTempFile;

fn write_config(contents: &str) -> NamedTempFile {
    let mut file = NamedTempFile::new().expect("temp file");
    std::io::Write::write_all(&mut file, contents.as_bytes()).expect("write config");
    file
}

#[test]
fn config_rejects_missing_sqlite_path() {
    let toml = r#"
[usage.ledger]
ledger_type = "sqlite"
"#;
    let file = write_config(toml);
    let result = EnterpriseConfig::load(Some(file.path()));
    assert!(matches!(result, Err(EnterpriseConfigError::Invalid(_))));
}

#[test]
fn config_allows_memory_ledger() {
    let toml = r#"
[usage.ledger]
ledger_type = "memory"
"#;
    let file = write_config(toml);
    let config = EnterpriseConfig::load(Some(file.path())).expect("load config");
    let options = config
        .build_server_options(Arc::new(NoopTenantAuthorizer), Arc::new(McpNoopAuditSink))
        .expect("build options");
    assert!(options.run_state_store.is_none());
    assert!(options.schema_registry.is_none());
    assert!(options.runpack_storage.is_none());
}

#[test]
fn config_rejects_empty_postgres_connection() {
    let toml = r#"
[storage.postgres]
connection = ""
max_connections = 8
connect_timeout_ms = 1000
statement_timeout_ms = 1000
[usage.ledger]
ledger_type = "memory"
"#;
    let file = write_config(toml);
    let result = EnterpriseConfig::load(Some(file.path()));
    assert!(matches!(result, Err(EnterpriseConfigError::Invalid(_))));
}

#[test]
fn config_rejects_invalid_postgres_connection_on_build() {
    let toml = r#"
[storage.postgres]
connection = "not-a-url"
max_connections = 8
connect_timeout_ms = 1000
statement_timeout_ms = 1000
[usage.ledger]
ledger_type = "memory"
"#;
    let file = write_config(toml);
    let config = EnterpriseConfig::load(Some(file.path())).expect("load config");
    let result =
        config.build_server_options(Arc::new(NoopTenantAuthorizer), Arc::new(McpNoopAuditSink));
    assert!(matches!(result, Err(EnterpriseConfigError::Storage(_))));
}

#[test]
fn config_defaults_to_sqlite_ledger_type() {
    let config = EnterpriseConfig {
        usage: EnterpriseUsageConfig::default(),
        storage: EnterpriseStorageConfig::default(),
        runpacks: EnterpriseRunpackConfig::default(),
        source_modified_at: None,
    };
    assert_eq!(config.usage.ledger.ledger_type, UsageLedgerType::Sqlite);
}

#[test]
fn config_rejects_invalid_runpack_prefix() {
    let toml = r#"
[runpacks.s3]
bucket = "decision-gate-test"
prefix = "invalid/../prefix"
[usage.ledger]
ledger_type = "memory"
"#;
    let file = write_config(toml);
    let config = EnterpriseConfig::load(Some(file.path())).expect("load config");
    let result =
        config.build_server_options(Arc::new(NoopTenantAuthorizer), Arc::new(McpNoopAuditSink));
    assert!(matches!(result, Err(EnterpriseConfigError::Storage(_))));
}

#[test]
fn config_rejects_oversize_file() {
    let file = NamedTempFile::new().expect("temp file");
    // Write valid TOML header then pad with comments to exceed 512KB.
    let header = "[usage.ledger]\nledger_type = \"memory\"\n";
    let pad_line = "# padding comment line to fill up space\n";
    let target_size = 512 * 1024 + 1;
    let mut content = String::with_capacity(target_size + 100);
    content.push_str(header);
    while content.len() < target_size {
        content.push_str(pad_line);
    }
    std::io::Write::write_all(
        &mut std::fs::File::create(file.path()).expect("create"),
        content.as_bytes(),
    )
    .expect("write oversized file");

    let result = EnterpriseConfig::load(Some(file.path()));
    assert!(result.is_err(), "expected error for oversized file");
    match result.unwrap_err() {
        EnterpriseConfigError::Invalid(msg) => {
            assert!(
                msg.contains("exceeds size limit"),
                "expected 'exceeds size limit' in error, got: {msg}"
            );
        }
        other => panic!("expected Invalid error, got: {other:?}"),
    }
}

#[test]
fn config_rejects_non_utf8_content() {
    let file = NamedTempFile::new().expect("temp file");
    std::fs::write(file.path(), [0xFF, 0xFE]).expect("write non-utf8");

    let result = EnterpriseConfig::load(Some(file.path()));
    assert!(result.is_err(), "expected error for non-utf8 content");
    match result.unwrap_err() {
        EnterpriseConfigError::Invalid(msg) => {
            assert!(msg.to_lowercase().contains("utf-8"), "expected 'utf-8' in error, got: {msg}");
        }
        other => panic!("expected Invalid error, got: {other:?}"),
    }
}

#[test]
fn config_rejects_overlong_path() {
    // Build a path with total length > 4096 characters.
    let long_component = "a".repeat(200);
    let mut path = PathBuf::new();
    while path.as_os_str().len() <= 4096 {
        path.push(&long_component);
    }
    let result = EnterpriseConfig::load(Some(path.as_path()));
    assert!(result.is_err(), "expected error for overlong path");
    match result.unwrap_err() {
        EnterpriseConfigError::Invalid(msg) => {
            assert!(
                msg.contains("exceeds max length"),
                "expected 'exceeds max length' in error, got: {msg}"
            );
        }
        other => panic!("expected Invalid error, got: {other:?}"),
    }
}

#[test]
fn config_rejects_overlong_path_component() {
    // A single path component of 256 chars exceeds the 255-char limit.
    let long_component = "b".repeat(256);
    let path = PathBuf::from(&long_component);
    let result = EnterpriseConfig::load(Some(path.as_path()));
    assert!(result.is_err(), "expected error for overlong path component");
    match result.unwrap_err() {
        EnterpriseConfigError::Invalid(msg) => {
            assert!(
                msg.contains("component too long"),
                "expected 'component too long' in error, got: {msg}"
            );
        }
        other => panic!("expected Invalid error, got: {other:?}"),
    }
}

#[test]
fn config_store_path_rejects_overlong_total() {
    // Write TOML with sqlite_path set to a string > 4096 chars.
    let long_path = "a".repeat(4097);
    let toml = format!("[usage.ledger]\nledger_type = \"sqlite\"\nsqlite_path = \"{long_path}\"\n");
    let file = write_config(&toml);
    let result = EnterpriseConfig::load(Some(file.path()));
    assert!(result.is_err(), "expected error for overlong store path");
    match result.unwrap_err() {
        EnterpriseConfigError::Invalid(msg) => {
            assert!(
                msg.contains("exceeds max length"),
                "expected 'exceeds max length' in error, got: {msg}"
            );
        }
        other => panic!("expected Invalid error, got: {other:?}"),
    }
}

#[test]
fn config_store_path_rejects_overlong_component() {
    // Write TOML with sqlite_path containing a 256-char directory component.
    let long_dir = "c".repeat(256);
    let store_path = format!("{long_dir}/usage.db");
    let toml =
        format!("[usage.ledger]\nledger_type = \"sqlite\"\nsqlite_path = \"{store_path}\"\n");
    let file = write_config(&toml);
    let result = EnterpriseConfig::load(Some(file.path()));
    assert!(result.is_err(), "expected error for overlong store path component");
    match result.unwrap_err() {
        EnterpriseConfigError::Invalid(msg) => {
            assert!(
                msg.contains("component too long"),
                "expected 'component too long' in error, got: {msg}"
            );
        }
        other => panic!("expected Invalid error, got: {other:?}"),
    }
}

#[test]
fn config_rejects_malformed_toml() {
    let file = write_config("[broken");
    let result = EnterpriseConfig::load(Some(file.path()));
    assert!(result.is_err(), "expected error for malformed TOML");
    match result.unwrap_err() {
        EnterpriseConfigError::Parse(_) => {}
        other => panic!("expected Parse error, got: {other:?}"),
    }
}

#[allow(unsafe_code, reason = "Test harness mutates process env for configuration.")]
#[test]
fn config_env_var_overrides_default_path() {
    // Write valid config to a unique temp file.
    let file = write_config("[usage.ledger]\nledger_type = \"memory\"\n");
    let path_str = file.path().to_string_lossy().to_string();

    // Set the env var. This is process-global so we must be careful.
    // SAFETY: Test controls the process env in a single-threaded section.
    unsafe {
        std::env::set_var("DECISION_GATE_ENTERPRISE_CONFIG", &path_str);
    }
    let result = EnterpriseConfig::load(None);
    // SAFETY: Resets the env var set above to avoid cross-test leakage.
    unsafe {
        std::env::remove_var("DECISION_GATE_ENTERPRISE_CONFIG");
    }

    let config = result.expect("load config via env var");
    assert_eq!(config.usage.ledger.ledger_type, UsageLedgerType::Memory);
}

#[test]
fn config_missing_file_returns_io_error() {
    let result = EnterpriseConfig::load(Some(Path::new("/nonexistent/path/config.toml")));
    assert!(result.is_err(), "expected error for missing file");
    match result.unwrap_err() {
        EnterpriseConfigError::Io(_) => {}
        other => panic!("expected Io error, got: {other:?}"),
    }
}

#[test]
fn config_source_modified_at_is_populated() {
    let file = write_config("[usage.ledger]\nledger_type = \"memory\"\n");
    let config = EnterpriseConfig::load(Some(file.path())).expect("load config");
    assert!(
        config.source_modified_at.is_some(),
        "source_modified_at should be populated after loading from disk"
    );
}
