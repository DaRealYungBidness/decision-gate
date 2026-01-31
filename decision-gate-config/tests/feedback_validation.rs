//! Feedback configuration validation tests for decision-gate-config.

mod common;

type TestResult = Result<(), String>;

#[test]
fn feedback_default_exceeds_max_rejected() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.server.feedback.scenario_next.default = decision_gate_config::FeedbackLevel::Evidence;
    config.server.feedback.scenario_next.max = decision_gate_config::FeedbackLevel::Trace;
    let Err(err) = config.validate() else {
        return Err("expected validation error".to_string());
    };
    if !err.to_string().contains("server.feedback.scenario_next.default exceeds max") {
        return Err(format!("unexpected error: {err}"));
    }
    Ok(())
}

#[test]
fn feedback_local_only_exceeds_max_rejected() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.server.feedback.scenario_next.local_only_default =
        decision_gate_config::FeedbackLevel::Evidence;
    config.server.feedback.scenario_next.max = decision_gate_config::FeedbackLevel::Trace;
    let Err(err) = config.validate() else {
        return Err("expected validation error".to_string());
    };
    if !err.to_string().contains("server.feedback.scenario_next.local_only_default exceeds max") {
        return Err(format!("unexpected error: {err}"));
    }
    Ok(())
}
