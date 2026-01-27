//! Enterprise Postgres store system tests.
// enterprise-system-tests/tests/postgres_store.rs
// ============================================================================
// Module: Postgres Store Tests
// Description: Validate Postgres-backed run state + schema registry behavior.
// Purpose: Ensure enterprise storage is durable, deterministic, and tamper-safe.
// Dependencies: enterprise system-test helpers
// ============================================================================

mod helpers;

use std::sync::Arc;

use decision_gate_core::DataShapeId;
use decision_gate_core::DataShapeRecord;
use decision_gate_core::DataShapeRegistry;
use decision_gate_core::DataShapeSignature;
use decision_gate_core::DataShapeVersion;
use decision_gate_core::NamespaceId;
use decision_gate_core::RunState;
use decision_gate_core::RunStateStore;
use decision_gate_core::RunStatus;
use decision_gate_core::StoreError;
use decision_gate_core::TenantId;
use decision_gate_core::Timestamp;
use decision_gate_store_enterprise::postgres_store::PostgresStoreConfig;
use helpers::artifacts::TestReporter;
use helpers::infra::PostgresFixture;
use helpers::infra::build_postgres_store_blocking;
use helpers::infra::wait_for_postgres_blocking;
use helpers::scenarios::ScenarioFixture;
use serde_json::json;

#[test]
fn postgres_store_run_state_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("postgres_store_run_state_roundtrip")?;

    let postgres = PostgresFixture::start()?;
    wait_for_postgres_blocking(&postgres.url)?;
    let store = build_postgres_store_blocking(postgres_config(&postgres.url))?;

    let fixture = ScenarioFixture::time_after("pg-roundtrip", "run-1", 0);
    let state_v1 = build_state(&fixture, RunStatus::Active, 0);
    store.save(&state_v1)?;

    let state_v2 = build_state(&fixture, RunStatus::Completed, 1);
    store.save(&state_v2)?;

    let loaded = store
        .load(&state_v2.tenant_id, &state_v2.namespace_id, &state_v2.run_id)?
        .ok_or("missing run state")?;

    if loaded.status != RunStatus::Completed {
        return Err("expected latest run state to be completed".into());
    }
    if loaded.spec_hash != state_v2.spec_hash {
        return Err("spec hash mismatch after roundtrip".into());
    }

    let mut client = postgres::Client::connect(&postgres.url, postgres::NoTls)?;
    let latest: i64 = client
        .query_one(
            "SELECT latest_version FROM runs WHERE tenant_id = $1 AND namespace_id = $2 AND \
             run_id = $3",
            &[
                &state_v2.tenant_id.as_str(),
                &state_v2.namespace_id.as_str(),
                &state_v2.run_id.as_str(),
            ],
        )?
        .get(0);
    if latest != 2 {
        return Err(format!("expected latest_version=2, got {latest}").into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &Vec::<serde_json::Value>::new())?;
    reporter.finish(
        "pass",
        vec!["postgres run state roundtrip verified".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}

#[test]
fn postgres_store_corruption_detection() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("postgres_store_corruption_detection")?;

    let postgres = PostgresFixture::start()?;
    wait_for_postgres_blocking(&postgres.url)?;
    let store = build_postgres_store_blocking(postgres_config(&postgres.url))?;

    let fixture = ScenarioFixture::time_after("pg-corrupt", "run-1", 0);
    let state = build_state(&fixture, RunStatus::Active, 0);
    store.save(&state)?;

    let mut client = postgres::Client::connect(&postgres.url, postgres::NoTls)?;
    client.execute(
        "UPDATE run_state_versions SET state_hash = $1 WHERE tenant_id = $2 AND namespace_id = $3 \
         AND run_id = $4",
        &[
            &"deadbeef",
            &state.tenant_id.as_str(),
            &state.namespace_id.as_str(),
            &state.run_id.as_str(),
        ],
    )?;

    let result = store.load(&state.tenant_id, &state.namespace_id, &state.run_id);
    match result {
        Err(StoreError::Corrupt(_)) => {}
        Err(err) => return Err(format!("expected corruption error, got {err:?}").into()),
        Ok(_) => return Err("expected corruption error, got ok".into()),
    }

    reporter.artifacts().write_json("tool_transcript.json", &Vec::<serde_json::Value>::new())?;
    reporter.finish(
        "pass",
        vec!["postgres corruption detection enforced".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}

#[test]
fn postgres_store_concurrent_writes() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("postgres_store_concurrent_writes")?;

    let postgres = PostgresFixture::start()?;
    wait_for_postgres_blocking(&postgres.url)?;
    let store = Arc::new(build_postgres_store_blocking(postgres_config(&postgres.url))?);

    let fixture = ScenarioFixture::time_after("pg-concurrent", "run-1", 0);
    let mut handles = Vec::new();
    let writes = 6u64;
    for idx in 0 .. writes {
        let store = Arc::clone(&store);
        let state = build_state(&fixture, RunStatus::Active, idx);
        handles.push(std::thread::spawn(move || store.save(&state)));
    }
    for handle in handles {
        handle
            .join()
            .map_err(|_| "join failed".to_string())?
            .map_err(|err| format!("save failed: {err}"))?;
    }

    let mut client = postgres::Client::connect(&postgres.url, postgres::NoTls)?;
    let latest: i64 = client
        .query_one(
            "SELECT latest_version FROM runs WHERE tenant_id = $1 AND namespace_id = $2 AND \
             run_id = $3",
            &[
                &fixture.tenant_id.as_str(),
                &fixture.namespace_id.as_str(),
                &fixture.run_id.as_str(),
            ],
        )?
        .get(0);
    let writes_i64 = i64::try_from(writes).map_err(|_| "writes out of i64 range")?;
    if latest != writes_i64 {
        return Err(format!("expected latest_version={writes}, got {latest}").into());
    }
    let count: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM run_state_versions WHERE tenant_id = $1 AND namespace_id = $2 \
             AND run_id = $3",
            &[
                &fixture.tenant_id.as_str(),
                &fixture.namespace_id.as_str(),
                &fixture.run_id.as_str(),
            ],
        )?
        .get(0);
    if count != writes_i64 {
        return Err(format!("expected {writes} run_state_versions, got {count}").into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &Vec::<serde_json::Value>::new())?;
    reporter.finish(
        "pass",
        vec!["postgres concurrent writes are atomic".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}

#[test]
fn postgres_registry_pagination_stability() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("postgres_registry_pagination_stability")?;

    let postgres = PostgresFixture::start()?;
    wait_for_postgres_blocking(&postgres.url)?;
    let store = build_postgres_store_blocking(postgres_config(&postgres.url))?;

    let tenant_id = TenantId::new("tenant-1");
    let namespace_id = NamespaceId::new("default");
    let records = vec![
        schema_record(&tenant_id, &namespace_id, "alpha", "v1", None),
        schema_record(&tenant_id, &namespace_id, "alpha", "v2", None),
        schema_record(&tenant_id, &namespace_id, "beta", "v1", None),
        schema_record(&tenant_id, &namespace_id, "beta", "v2", None),
        schema_record(&tenant_id, &namespace_id, "gamma", "v1", None),
    ];
    for record in records.clone() {
        store.register(record)?;
    }

    let mut cursor = None;
    let mut listed = Vec::new();
    loop {
        let page = store.list(&tenant_id, &namespace_id, cursor.clone(), 2)?;
        for item in &page.items {
            listed.push((item.schema_id.as_str().to_string(), item.version.as_str().to_string()));
        }
        cursor = page.next_token;
        if cursor.is_none() {
            break;
        }
    }

    let expected: Vec<(String, String)> = records
        .iter()
        .map(|record| (record.schema_id.as_str().to_string(), record.version.as_str().to_string()))
        .collect();
    if listed != expected {
        return Err(format!("pagination order mismatch: {listed:?} != {expected:?}").into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &Vec::<serde_json::Value>::new())?;
    reporter.finish(
        "pass",
        vec!["postgres registry pagination stable".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}

#[test]
fn postgres_registry_signing_metadata() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("postgres_registry_signing_metadata")?;

    let postgres = PostgresFixture::start()?;
    wait_for_postgres_blocking(&postgres.url)?;
    let store = build_postgres_store_blocking(postgres_config(&postgres.url))?;

    let tenant_id = TenantId::new("tenant-1");
    let namespace_id = NamespaceId::new("default");
    let signing = DataShapeSignature {
        key_id: "key-1".to_string(),
        signature: "sig-1".to_string(),
        algorithm: Some("ed25519".to_string()),
    };
    let record =
        schema_record(&tenant_id, &namespace_id, "signed-schema", "v1", Some(signing.clone()));
    store.register(record.clone())?;

    let loaded = store
        .get(&tenant_id, &namespace_id, &record.schema_id, &record.version)?
        .ok_or("missing signed schema")?;

    let loaded_signing = loaded.signing.ok_or("missing signing metadata")?;
    if loaded_signing.key_id != signing.key_id
        || loaded_signing.signature != signing.signature
        || loaded_signing.algorithm != signing.algorithm
    {
        return Err("signing metadata mismatch".into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &Vec::<serde_json::Value>::new())?;
    reporter.finish(
        "pass",
        vec!["postgres registry signing metadata preserved".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}

fn postgres_config(url: &str) -> PostgresStoreConfig {
    PostgresStoreConfig {
        connection: url.to_string(),
        max_connections: 8,
        connect_timeout_ms: 5_000,
        statement_timeout_ms: 30_000,
    }
}

fn build_state(fixture: &ScenarioFixture, status: RunStatus, stage_tick: u64) -> RunState {
    let spec_hash = fixture.spec.canonical_hash().expect("spec hash");
    RunState {
        tenant_id: fixture.tenant_id.clone(),
        namespace_id: fixture.namespace_id.clone(),
        run_id: fixture.run_id.clone(),
        scenario_id: fixture.scenario_id.clone(),
        spec_hash,
        current_stage_id: fixture.stage_id.clone(),
        stage_entered_at: Timestamp::Logical(stage_tick),
        status,
        dispatch_targets: Vec::new(),
        triggers: Vec::new(),
        gate_evals: Vec::new(),
        decisions: Vec::new(),
        packets: Vec::new(),
        submissions: Vec::new(),
        tool_calls: Vec::new(),
    }
}

fn schema_record(
    tenant_id: &TenantId,
    namespace_id: &NamespaceId,
    schema_id: &str,
    version: &str,
    signing: Option<DataShapeSignature>,
) -> DataShapeRecord {
    DataShapeRecord {
        tenant_id: tenant_id.clone(),
        namespace_id: namespace_id.clone(),
        schema_id: DataShapeId::new(schema_id),
        version: DataShapeVersion::new(version),
        schema: json!({
            "type": "object",
            "properties": {"after": {"type": "boolean"}},
            "required": ["after"],
        }),
        description: Some(format!("schema {schema_id} {version}")),
        created_at: Timestamp::Logical(1),
        signing,
    }
}
