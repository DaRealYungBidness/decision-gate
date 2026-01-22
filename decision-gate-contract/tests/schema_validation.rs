// decision-gate-contract/tests/schema_validation.rs
// ============================================================================
// Module: Contract Schema Validation Tests
// Description: Validate contract schemas and examples against JSON Schema rules.
// Purpose: Ensure contract artifacts are consistent with runtime types and samples.
// Dependencies: decision-gate-contract, decision-gate-core, jsonschema
// ============================================================================

//! Contract schema validation tests for generated artifacts and runtime data.

#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic_in_result_fn,
    clippy::unwrap_in_result,
    clippy::missing_docs_in_private_items,
    reason = "Test-only validation helpers use panic-based assertions for clarity."
)]

use std::collections::BTreeMap;
use std::error::Error;
use std::io;
use std::sync::Arc;

use decision_gate_contract::ContractBuilder;
use decision_gate_contract::examples;
use decision_gate_contract::schemas;
use decision_gate_contract::types::ProviderContract;
use decision_gate_contract::types::ToolContract;
use decision_gate_core::DecisionId;
use decision_gate_core::DecisionOutcome;
use decision_gate_core::DecisionRecord;
use decision_gate_core::DispatchReceipt;
use decision_gate_core::DispatchTarget;
use decision_gate_core::EvidenceAnchor;
use decision_gate_core::EvidenceRecord;
use decision_gate_core::EvidenceResult;
use decision_gate_core::EvidenceSignature;
use decision_gate_core::EvidenceValue;
use decision_gate_core::GateEvalRecord;
use decision_gate_core::GateEvaluation;
use decision_gate_core::GateId;
use decision_gate_core::GateTraceEntry;
use decision_gate_core::HashAlgorithm;
use decision_gate_core::HashDigest;
use decision_gate_core::PacketEnvelope;
use decision_gate_core::PacketId;
use decision_gate_core::PacketPayload;
use decision_gate_core::PacketRecord;
use decision_gate_core::PredicateKey;
use decision_gate_core::RunId;
use decision_gate_core::RunState;
use decision_gate_core::RunStatus;
use decision_gate_core::ScenarioId;
use decision_gate_core::SchemaId;
use decision_gate_core::StageId;
use decision_gate_core::SubmissionRecord;
use decision_gate_core::TenantId;
use decision_gate_core::Timestamp;
use decision_gate_core::ToolCallError;
use decision_gate_core::ToolCallErrorDetails;
use decision_gate_core::ToolCallRecord;
use decision_gate_core::TriggerEvent;
use decision_gate_core::TriggerId;
use decision_gate_core::TriggerKind;
use decision_gate_core::TriggerRecord;
use decision_gate_core::VisibilityPolicy;
use jsonschema::CompilationOptions;
use jsonschema::Draft;
use jsonschema::JSONSchema;
use jsonschema::SchemaResolver;
use jsonschema::SchemaResolverError;
use serde_json::Value;
use serde_json::json;
use url::Url;

#[derive(Clone)]
struct ContractSchemaResolver {
    registry: Arc<BTreeMap<String, Value>>,
}

impl ContractSchemaResolver {
    fn new(registry: BTreeMap<String, Value>) -> Self {
        Self {
            registry: Arc::new(registry),
        }
    }
}

impl SchemaResolver for ContractSchemaResolver {
    fn resolve(
        &self,
        _root_schema: &Value,
        url: &Url,
        _original_reference: &str,
    ) -> Result<Arc<Value>, SchemaResolverError> {
        let key = url.as_str();
        self.registry.get(key).map_or_else(
            || Err(io::Error::new(io::ErrorKind::NotFound, key.to_string()).into()),
            |schema| Ok(Arc::new(schema.clone())),
        )
    }
}

fn compile_schema(
    schema: &Value,
    resolver: &ContractSchemaResolver,
) -> Result<JSONSchema, Box<dyn Error>> {
    let mut options = CompilationOptions::default();
    options.with_draft(Draft::Draft202012);
    options.with_resolver(resolver.clone());
    let compiled = options.compile(schema).map_err(|err| io::Error::other(err.to_string()))?;
    Ok(compiled)
}

fn assert_valid(schema: &JSONSchema, instance: &Value, label: &str) -> Result<(), Box<dyn Error>> {
    match schema.validate(instance) {
        Ok(()) => Ok(()),
        Err(errors) => {
            let messages: Vec<String> = errors.map(|err| err.to_string()).collect();
            Err(format!("validation failed ({label}): {}", messages.join("; ")).into())
        }
    }
}

fn assert_invalid(
    schema: &JSONSchema,
    instance: &Value,
    label: &str,
) -> Result<(), Box<dyn Error>> {
    if schema.is_valid(instance) {
        Err(format!("expected invalid payload for {label}").into())
    } else {
        Ok(())
    }
}

fn artifact_json(
    bundle: &decision_gate_contract::ContractBundle,
    path: &str,
) -> Result<Value, Box<dyn Error>> {
    for artifact in &bundle.artifacts {
        if artifact.path == path {
            let value = serde_json::from_slice(&artifact.bytes)?;
            return Ok(value);
        }
    }
    Err(format!("artifact not found: {path}").into())
}

fn artifact_contracts<T: serde::de::DeserializeOwned>(
    bundle: &decision_gate_contract::ContractBundle,
    path: &str,
) -> Result<Vec<T>, Box<dyn Error>> {
    for artifact in &bundle.artifacts {
        if artifact.path == path {
            let value = serde_json::from_slice(&artifact.bytes)?;
            return Ok(value);
        }
    }
    Err(format!("artifact not found: {path}").into())
}

fn build_registry(schemas: &[Value]) -> Result<ContractSchemaResolver, Box<dyn Error>> {
    let mut registry = BTreeMap::new();
    for schema in schemas {
        let Some(id) = schema.get("$id").and_then(Value::as_str) else {
            return Err("schema missing $id".into());
        };
        registry.insert(id.to_string(), schema.clone());
    }
    Ok(ContractSchemaResolver::new(registry))
}

#[test]
fn contract_schemas_validate_examples_and_reject_invalid() -> Result<(), Box<dyn Error>> {
    let bundle = ContractBuilder::default().build()?;
    let scenario_schema = artifact_json(&bundle, "schemas/scenario.schema.json")?;
    let config_schema = artifact_json(&bundle, "schemas/config.schema.json")?;
    let resolver = build_registry(&[scenario_schema.clone(), config_schema.clone()])?;

    let scenario_validator = compile_schema(&scenario_schema, &resolver)?;
    let config_validator = compile_schema(&config_schema, &resolver)?;
    let run_config_validator = compile_schema(&schemas::run_config_schema(), &resolver)?;

    let scenario_example = artifact_json(&bundle, "examples/scenario.json")?;
    let run_config_example = artifact_json(&bundle, "examples/run-config.json")?;
    assert_valid(&scenario_validator, &scenario_example, "scenario example")?;
    assert_valid(&run_config_validator, &run_config_example, "run config example")?;

    let minimal_config = json!({});
    assert_valid(&config_validator, &minimal_config, "minimal config")?;

    let invalid_config = json!({
        "server": {
            "transport": "http"
        }
    });
    assert_invalid(&config_validator, &invalid_config, "http server missing bind")?;

    let invalid_scenario = json!({
        "scenario_id": "scenario-invalid",
        "spec_version": "v1",
        "stages": [],
        "predicates": [],
        "policies": [],
        "schemas": []
    });
    assert_invalid(&scenario_validator, &invalid_scenario, "scenario missing stages")?;

    Ok(())
}

#[test]
fn tooling_and_provider_schemas_compile_and_examples_validate() -> Result<(), Box<dyn Error>> {
    let bundle = ContractBuilder::default().build()?;
    let scenario_schema = artifact_json(&bundle, "schemas/scenario.schema.json")?;
    let config_schema = artifact_json(&bundle, "schemas/config.schema.json")?;
    let resolver = build_registry(&[scenario_schema, config_schema])?;

    let tool_contracts: Vec<ToolContract> = artifact_contracts(&bundle, "tooling.json")?;
    for contract in tool_contracts {
        let input_schema = compile_schema(&contract.input_schema, &resolver)?;
        let output_schema = compile_schema(&contract.output_schema, &resolver)?;
        let example = examples::scenario_example();
        let example_value = serde_json::to_value(example)?;
        if contract.name == "scenario_define" {
            let input = json!({ "spec": example_value });
            assert_valid(&input_schema, &input, "scenario_define input")?;
        }
        let output = json!({
            "scenario_id": "example-scenario",
            "spec_hash": {
                "algorithm": "sha256",
                "value": "5c3a5b6bce0f4a2c9e22c4fa6a1e6d8d90b0f2dfed1b7f1e9b3d3b3d1f0c9b21"
            }
        });
        if contract.name == "scenario_define" {
            assert_valid(&output_schema, &output, "scenario_define output")?;
        }
    }

    let provider_contracts: Vec<ProviderContract> = artifact_contracts(&bundle, "providers.json")?;
    for provider in provider_contracts {
        let _ = compile_schema(&provider.config_schema, &resolver)?;
        for predicate in provider.predicates {
            let params_schema = compile_schema(&predicate.params_schema, &resolver)?;
            let result_schema = compile_schema(&predicate.result_schema, &resolver)?;
            for example in predicate.examples {
                assert_valid(&params_schema, &example.params, "provider example params")?;
                assert_valid(&result_schema, &example.result, "provider example result")?;
            }
        }
    }

    Ok(())
}

#[test]
fn run_state_schema_accepts_core_structs() -> Result<(), Box<dyn Error>> {
    let resolver = build_registry(&[schemas::scenario_schema(), schemas::config_schema()])?;
    let run_state_schema = compile_schema(&schemas::run_state_schema(), &resolver)?;
    let run_state_value = serde_json::to_value(sample_run_state())?;
    assert_valid(&run_state_schema, &run_state_value, "run state")?;
    Ok(())
}

fn sample_hash_digest() -> HashDigest {
    HashDigest::new(HashAlgorithm::Sha256, b"decision-gate")
}

const fn sample_timestamp() -> Timestamp {
    Timestamp::UnixMillis(1_710_000_000_000)
}

#[allow(
    clippy::too_many_lines,
    reason = "Full run state fixture is intentionally verbose for schema coverage."
)]
fn sample_run_state() -> RunState {
    let tenant_id = TenantId::new("tenant-1");
    let run_id = RunId::new("run-1");
    let scenario_id = ScenarioId::new("scenario-1");
    let stage_id = StageId::new("stage-1");
    let trigger_id = TriggerId::new("trigger-1");
    let gate_id = GateId::new("gate-1");
    let decision_id = DecisionId::new("decision-1");
    let packet_id = PacketId::new("packet-1");
    let schema_id = SchemaId::new("schema-1");
    let predicate = PredicateKey::from("predicate-1");
    let timestamp = sample_timestamp();
    let hash = sample_hash_digest();

    let trigger_event = TriggerEvent {
        trigger_id: trigger_id.clone(),
        run_id: run_id.clone(),
        kind: TriggerKind::Tick,
        time: timestamp,
        source_id: "scheduler".to_string(),
        payload_ref: None,
        correlation_id: None,
    };

    let evidence_result = EvidenceResult {
        value: Some(EvidenceValue::Json(json!({"status": "ok"}))),
        evidence_hash: Some(hash.clone()),
        evidence_ref: None,
        evidence_anchor: Some(EvidenceAnchor {
            anchor_type: "env".to_string(),
            anchor_value: "DEPLOY_ENV".to_string(),
        }),
        signature: Some(EvidenceSignature {
            scheme: "ed25519".to_string(),
            key_id: "key-1".to_string(),
            signature: vec![1, 2, 3],
        }),
        content_type: Some("text/plain".to_string()),
    };

    let evidence_record = EvidenceRecord {
        predicate: predicate.clone(),
        status: ret_logic::TriState::True,
        result: evidence_result,
    };

    let evaluation = GateEvaluation {
        gate_id,
        status: ret_logic::TriState::True,
        trace: vec![GateTraceEntry {
            predicate,
            status: ret_logic::TriState::True,
        }],
    };

    let gate_eval = GateEvalRecord {
        trigger_id: trigger_id.clone(),
        stage_id: stage_id.clone(),
        evaluation,
        evidence: vec![evidence_record],
    };

    let decision = DecisionRecord {
        decision_id: decision_id.clone(),
        seq: 0,
        trigger_id,
        stage_id: stage_id.clone(),
        decided_at: timestamp,
        outcome: DecisionOutcome::Complete {
            stage_id: stage_id.clone(),
        },
        correlation_id: None,
    };

    let packet_envelope = PacketEnvelope {
        scenario_id: scenario_id.clone(),
        run_id: run_id.clone(),
        stage_id: stage_id.clone(),
        packet_id,
        schema_id,
        content_type: "application/json".to_string(),
        content_hash: hash.clone(),
        visibility: VisibilityPolicy::new(vec!["public".to_string()], Vec::new()),
        expiry: None,
        correlation_id: None,
        issued_at: timestamp,
    };

    let receipt = DispatchReceipt {
        dispatch_id: "dispatch-1".to_string(),
        target: DispatchTarget::Agent {
            agent_id: "agent-1".to_string(),
        },
        receipt_hash: hash.clone(),
        dispatched_at: timestamp,
        dispatcher: "dispatcher-1".to_string(),
    };

    let packet_record = PacketRecord {
        envelope: packet_envelope,
        payload: PacketPayload::Json {
            value: json!({"message": "hello"}),
        },
        receipts: vec![receipt],
        decision_id,
    };

    let submission = SubmissionRecord {
        submission_id: "submission-1".to_string(),
        run_id: run_id.clone(),
        payload: PacketPayload::Json {
            value: json!({"artifact": "attestation"}),
        },
        content_type: "application/json".to_string(),
        content_hash: hash.clone(),
        submitted_at: timestamp,
        correlation_id: None,
    };

    let tool_call = ToolCallRecord {
        call_id: "call-1".to_string(),
        method: "scenario_next".to_string(),
        request_hash: hash.clone(),
        response_hash: hash,
        called_at: timestamp,
        correlation_id: None,
        error: Some(ToolCallError {
            code: "provider_missing".to_string(),
            message: "provider missing".to_string(),
            details: Some(ToolCallErrorDetails::Message {
                info: "missing provider".to_string(),
            }),
        }),
    };

    RunState {
        tenant_id,
        run_id,
        scenario_id,
        spec_hash: sample_hash_digest(),
        current_stage_id: stage_id,
        status: RunStatus::Active,
        dispatch_targets: vec![DispatchTarget::Agent {
            agent_id: "agent-1".to_string(),
        }],
        triggers: vec![TriggerRecord {
            seq: 0,
            event: trigger_event,
        }],
        gate_evals: vec![gate_eval],
        decisions: vec![decision],
        packets: vec![packet_record],
        submissions: vec![submission],
        tool_calls: vec![tool_call],
    }
}
