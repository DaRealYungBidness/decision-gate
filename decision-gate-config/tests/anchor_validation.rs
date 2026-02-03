//! Anchor policy config validation tests for decision-gate-config.
// decision-gate-config/tests/anchor_validation.rs
// =============================================================================
// Module: Anchor Policy Validation Tests
// Description: Validate anchor policy provider constraints.
// Purpose: Ensure anchor provider entries are deterministic and unambiguous.
// =============================================================================

use decision_gate_config::AnchorProviderConfig;
use decision_gate_config::ConfigError;

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
fn anchors_provider_id_must_be_trimmed() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.anchors.providers = vec![AnchorProviderConfig {
        provider_id: " time".to_string(),
        anchor_type: "env".to_string(),
        required_fields: vec!["tenant_id".to_string()],
    }];
    assert_invalid(config.validate(), "anchors.providers.provider_id must be trimmed")?;
    Ok(())
}

#[test]
fn anchors_provider_ids_must_be_unique() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.anchors.providers = vec![
        AnchorProviderConfig {
            provider_id: "time".to_string(),
            anchor_type: "env".to_string(),
            required_fields: vec!["tenant_id".to_string()],
        },
        AnchorProviderConfig {
            provider_id: "time".to_string(),
            anchor_type: "env".to_string(),
            required_fields: vec!["tenant_id".to_string()],
        },
    ];
    assert_invalid(config.validate(), "duplicate anchors.providers.provider_id: time")?;
    Ok(())
}
