//! Storage config validation tests for decision-gate-config.
// decision-gate-config/tests/storage_validation.rs
// =============================================================================
// Module: Storage Config Validation Tests
// Description: Validate run-state store and runpack storage constraints.
// Purpose: Ensure storage configuration remains secure and deterministic.
// =============================================================================

use std::path::PathBuf;

use decision_gate_config::ConfigError;
use decision_gate_config::ObjectStoreConfig;
use decision_gate_config::ObjectStoreProvider;
use decision_gate_config::RunStateStoreType;
use decision_gate_config::RunpackStorageConfig;

mod common;

type TestResult = Result<(), String>;

fn assert_invalid(result: Result<(), ConfigError>, needle: &str) -> TestResult {
    match result {
        Err(error) => {
            let message = error.to_string();
            if message.contains(needle) {
                Ok(())
            } else {
                Err(format!("error {message} did not contain {needle}"))
            }
        }
        Ok(()) => Err("expected invalid config".to_string()),
    }
}

#[test]
fn run_state_store_memory_rejects_path() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.run_state_store.store_type = RunStateStoreType::Memory;
    config.run_state_store.path = Some(PathBuf::from("run.db"));
    assert_invalid(config.validate(), "memory run_state_store must not set path")?;
    Ok(())
}

#[test]
fn run_state_store_sqlite_requires_path() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.run_state_store.store_type = RunStateStoreType::Sqlite;
    config.run_state_store.path = None;
    assert_invalid(config.validate(), "sqlite run_state_store requires path")?;
    Ok(())
}

#[test]
fn run_state_store_sqlite_rejects_zero_max_versions() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.run_state_store.store_type = RunStateStoreType::Sqlite;
    config.run_state_store.path = Some(PathBuf::from("run.db"));
    config.run_state_store.max_versions = Some(0);
    assert_invalid(config.validate(), "run_state_store max_versions must be greater than zero")?;
    Ok(())
}

#[test]
fn runpack_storage_requires_bucket() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.runpack_storage = Some(RunpackStorageConfig::ObjectStore(ObjectStoreConfig {
        provider: ObjectStoreProvider::S3,
        bucket: "  ".to_string(),
        region: None,
        endpoint: None,
        prefix: None,
        force_path_style: false,
        allow_http: false,
    }));
    assert_invalid(config.validate(), "runpack_storage.bucket must be set")?;
    Ok(())
}

#[test]
fn runpack_storage_rejects_http_without_allow() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.runpack_storage = Some(RunpackStorageConfig::ObjectStore(ObjectStoreConfig {
        provider: ObjectStoreProvider::S3,
        bucket: "runpacks".to_string(),
        region: None,
        endpoint: Some("http://s3.example.com".to_string()),
        prefix: None,
        force_path_style: false,
        allow_http: false,
    }));
    assert_invalid(config.validate(), "runpack_storage.endpoint uses http:// without allow_http")?;
    Ok(())
}

#[test]
fn runpack_storage_rejects_prefix_traversal() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.runpack_storage = Some(RunpackStorageConfig::ObjectStore(ObjectStoreConfig {
        provider: ObjectStoreProvider::S3,
        bucket: "runpacks".to_string(),
        region: None,
        endpoint: None,
        prefix: Some("../runpacks".to_string()),
        force_path_style: false,
        allow_http: false,
    }));
    assert_invalid(config.validate(), "runpack_storage.prefix must be relative without traversal")?;
    Ok(())
}
