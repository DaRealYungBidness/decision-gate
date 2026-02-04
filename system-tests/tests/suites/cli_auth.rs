// system-tests/tests/suites/cli_auth.rs
// ============================================================================
// Module: CLI Auth Matrix Tests
// Description: CLI MCP client authentication coverage.
// Purpose: Ensure CLI honors bearer token and mTLS subject auth paths.
// Dependencies: system-tests helpers, decision-gate-cli
// ============================================================================

//! CLI auth matrix tests.

use std::fs;
use std::path::Path;
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;
use tempfile::TempDir;

use crate::helpers::artifacts::TestReporter;
use crate::helpers::cli::cli_binary;
use crate::helpers::cli::run_cli;
use crate::helpers::harness::allocate_bind_addr;
use crate::helpers::harness::base_http_config_with_bearer;
use crate::helpers::harness::base_http_config_with_mtls;
use crate::helpers::harness::spawn_mcp_server;
use crate::helpers::readiness::wait_for_ready;

#[derive(Serialize)]
struct CliAuthEntry {
    scenario: String,
    status: i32,
    stdout: String,
    stderr: String,
}

fn record_cli_entry(
    transcript: &mut Vec<CliAuthEntry>,
    scenario: &str,
    output: &std::process::Output,
) {
    transcript.push(CliAuthEntry {
        scenario: scenario.to_string(),
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    });
}

async fn run_bearer_auth_checks(
    cli: &Path,
    transcript: &mut Vec<CliAuthEntry>,
) -> Result<(), Box<dyn std::error::Error>> {
    let bearer_token = "cli-bearer-token";
    let bearer_bind = allocate_bind_addr()?.to_string();
    let bearer_server =
        spawn_mcp_server(base_http_config_with_bearer(&bearer_bind, bearer_token)).await?;
    let bearer_client =
        bearer_server.client(Duration::from_secs(5))?.with_bearer_token(bearer_token.to_string());
    wait_for_ready(
        || async { bearer_client.list_tools().await.map(|_| ()) },
        Duration::from_secs(5),
        "bearer server",
    )
    .await?;
    let bearer_url = bearer_server.base_url().to_string();

    let unauthorized = run_cli(cli, &["mcp", "tools", "list", "--endpoint", &bearer_url])?;
    record_cli_entry(transcript, "bearer_unauthorized", &unauthorized);
    if unauthorized.status.success() {
        return Err("expected bearer auth failure without token".into());
    }

    let authorized = run_cli(
        cli,
        &["mcp", "tools", "list", "--endpoint", &bearer_url, "--bearer-token", bearer_token],
    )?;
    record_cli_entry(transcript, "bearer_authorized", &authorized);
    if !authorized.status.success() {
        return Err("expected bearer auth success with token".into());
    }

    bearer_server.shutdown().await;
    Ok(())
}

async fn run_mtls_auth_checks(
    cli: &Path,
    transcript: &mut Vec<CliAuthEntry>,
) -> Result<(), Box<dyn std::error::Error>> {
    let subject = "CN=decision-gate-cli,O=Example";
    let mtls_bind = allocate_bind_addr()?.to_string();
    let mtls_server = spawn_mcp_server(base_http_config_with_mtls(&mtls_bind, subject)).await?;
    let mtls_client =
        mtls_server.client(Duration::from_secs(5))?.with_client_subject(subject.to_string());
    wait_for_ready(
        || async { mtls_client.list_tools().await.map(|_| ()) },
        Duration::from_secs(5),
        "mtls server",
    )
    .await?;
    let mtls_url = mtls_server.base_url().to_string();

    let unauthorized = run_cli(cli, &["mcp", "tools", "list", "--endpoint", &mtls_url])?;
    record_cli_entry(transcript, "mtls_unauthorized", &unauthorized);
    if unauthorized.status.success() {
        return Err("expected mTLS subject failure without client subject".into());
    }

    let authorized = run_cli(
        cli,
        &["mcp", "tools", "list", "--endpoint", &mtls_url, "--client-subject", subject],
    )?;
    record_cli_entry(transcript, "mtls_authorized", &authorized);
    if !authorized.status.success() {
        return Err("expected mTLS subject success with client subject".into());
    }

    mtls_server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn cli_auth_matrix() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("cli_auth_matrix")?;
    let Some(cli) = cli_binary() else {
        reporter.artifacts().write_json("tool_transcript.json", &Vec::<Value>::new())?;
        reporter.finish(
            "skip",
            vec!["decision-gate CLI binary unavailable".to_string()],
            vec![
                "summary.json".to_string(),
                "summary.md".to_string(),
                "tool_transcript.json".to_string(),
            ],
        )?;
        drop(reporter);
        return Ok(());
    };

    let mut transcript: Vec<CliAuthEntry> = Vec::new();
    run_bearer_auth_checks(&cli, &mut transcript).await?;
    run_mtls_auth_checks(&cli, &mut transcript).await?;

    reporter.artifacts().write_json("tool_transcript.json", &transcript)?;
    reporter.finish(
        "pass",
        vec!["CLI bearer and mTLS subject auth verified".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn cli_auth_profile_bearer_token() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("cli_auth_profile_bearer_token")?;
    let Some(cli) = cli_binary() else {
        reporter.artifacts().write_json("tool_transcript.json", &Vec::<Value>::new())?;
        reporter.finish(
            "skip",
            vec!["decision-gate CLI binary unavailable".to_string()],
            vec![
                "summary.json".to_string(),
                "summary.md".to_string(),
                "tool_transcript.json".to_string(),
            ],
        )?;
        drop(reporter);
        return Ok(());
    };

    let bearer_token = "cli-auth-profile-token";
    let bind = allocate_bind_addr()?.to_string();
    let server = spawn_mcp_server(base_http_config_with_bearer(&bind, bearer_token)).await?;
    let client = server.client(Duration::from_secs(5))?.with_bearer_token(bearer_token.to_string());
    wait_for_ready(
        || async { client.list_tools().await.map(|_| ()) },
        Duration::from_secs(5),
        "bearer server",
    )
    .await?;
    let base_url = server.base_url().to_string();

    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("decision-gate.toml");
    let config_contents = format!(
        r#"[client.auth_profiles.default]
bearer_token = "{bearer_token}"
"#
    );
    fs::write(&config_path, config_contents)?;

    let output = run_cli(
        &cli,
        &[
            "mcp",
            "tools",
            "list",
            "--endpoint",
            &base_url,
            "--auth-profile",
            "default",
            "--auth-config",
            config_path.to_str().unwrap_or_default(),
        ],
    )?;
    let transcript = vec![CliAuthEntry {
        scenario: "auth_profile_bearer_token".to_string(),
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    }];
    if !output.status.success() {
        return Err("expected auth profile bearer token to succeed".into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &transcript)?;
    reporter.finish(
        "pass",
        vec!["CLI auth profile bearer token honored".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    server.shutdown().await;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn cli_auth_profile_cli_override() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("cli_auth_profile_cli_override")?;
    let Some(cli) = cli_binary() else {
        reporter.artifacts().write_json("tool_transcript.json", &Vec::<Value>::new())?;
        reporter.finish(
            "skip",
            vec!["decision-gate CLI binary unavailable".to_string()],
            vec![
                "summary.json".to_string(),
                "summary.md".to_string(),
                "tool_transcript.json".to_string(),
            ],
        )?;
        drop(reporter);
        return Ok(());
    };

    let correct_token = "cli-auth-profile-correct";
    let bind = allocate_bind_addr()?.to_string();
    let server = spawn_mcp_server(base_http_config_with_bearer(&bind, correct_token)).await?;
    let client =
        server.client(Duration::from_secs(5))?.with_bearer_token(correct_token.to_string());
    wait_for_ready(
        || async { client.list_tools().await.map(|_| ()) },
        Duration::from_secs(5),
        "bearer server",
    )
    .await?;
    let base_url = server.base_url().to_string();

    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("decision-gate.toml");
    let config_contents = r#"[client.auth_profiles.default]
bearer_token = "wrong-token"
"#;
    fs::write(&config_path, config_contents)?;

    let mut transcript = Vec::new();

    let unauthorized = run_cli(
        &cli,
        &[
            "mcp",
            "tools",
            "list",
            "--endpoint",
            &base_url,
            "--auth-profile",
            "default",
            "--auth-config",
            config_path.to_str().unwrap_or_default(),
        ],
    )?;
    transcript.push(CliAuthEntry {
        scenario: "auth_profile_wrong_token".to_string(),
        status: unauthorized.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&unauthorized.stdout).to_string(),
        stderr: String::from_utf8_lossy(&unauthorized.stderr).to_string(),
    });
    if unauthorized.status.success() {
        return Err("expected auth profile wrong token to fail".into());
    }

    let authorized = run_cli(
        &cli,
        &[
            "mcp",
            "tools",
            "list",
            "--endpoint",
            &base_url,
            "--auth-profile",
            "default",
            "--auth-config",
            config_path.to_str().unwrap_or_default(),
            "--bearer-token",
            correct_token,
        ],
    )?;
    transcript.push(CliAuthEntry {
        scenario: "auth_profile_cli_override".to_string(),
        status: authorized.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&authorized.stdout).to_string(),
        stderr: String::from_utf8_lossy(&authorized.stderr).to_string(),
    });
    if !authorized.status.success() {
        return Err("expected CLI bearer token to override profile".into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &transcript)?;
    reporter.finish(
        "pass",
        vec!["CLI bearer token overrides auth profile".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    server.shutdown().await;
    drop(reporter);
    Ok(())
}
