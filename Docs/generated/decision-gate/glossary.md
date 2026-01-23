# Decision Gate Glossary

Short, canonical definitions for Decision Gate terms.

## `EvidenceAnchor`

Stable anchor metadata for offline verification.

## `EvidenceContext`

Run context passed to evidence providers during queries.

## `EvidenceQuery`

Evidence query containing provider_id, predicate, and params.

## `EvidenceRef`

External reference URI for evidence content.

## `EvidenceResult`

Provider response containing evidence value and metadata.

## `EvidenceValue`

Evidence payload (json or bytes) returned by a provider.

## `GateOutcome`

Outcome selector for branching (true, false, unknown).

## `GateSpec`

Gate definition referencing a requirement inside a stage.

## `Predicate`

RET leaf that references a predicate key.

## `PredicateSpec`

Predicate definition inside a ScenarioSpec.

## `RequireGroup`

RET operator requiring at least min of reqs to pass.

## `Requirement`

RET definition composed of And/Or/Not/RequireGroup/Predicate nodes.

## `ScenarioSpec`

Scenario specification defining stages, predicates, and policies.

## `TimeoutPolicy`

Timeout handling policy for a stage.

## `advance_to`

Stage advancement policy: linear, fixed, branch, or terminal.

## `allow_http`

Allow cleartext http:// URLs for the HTTP provider.

## `allow_insecure_http`

Allow http:// URLs for MCP providers.

## `allow_logical`

Allow logical trigger timestamps for time predicates.

## `allow_raw`

Allow raw evidence disclosure for this provider.

## `allow_raw_values`

Allow raw evidence values to be returned.

## `allow_yaml`

Allow YAML parsing in the JSON provider.

## `allowed_hosts`

Allowed HTTP hostnames for outbound checks.

## `allowlist`

Allowed keys for environment lookup.

## `bind`

Bind address required for HTTP/SSE transports.

## `capabilities_path`

Path to the provider capability contract JSON.

## `comparator`

Comparison operator applied to the evidence result.

## `content_hash`

Hash metadata for payload content.

## `content_type`

MIME type for evidence or packet content.

## `correlation_id`

Correlation identifier used to link requests and decisions.

## `decision`

Decision record returned by evaluation tools.

## `decision_id`

Decision identifier recorded in run history.

## `default_policy`

Default trust policy for evidence providers.

## `denylist`

Blocked keys for environment lookup.

## `dispatch_targets`

Dispatch destinations for emitted packets.

## `entry_packets`

Disclosures emitted when a stage starts.

## `evidence_anchor`

Anchor metadata linking evidence to a receipt or source.

## `evidence_hash`

Hash of the evidence value for audit and integrity checks.

## `evidence_query`

Query evidence providers with disclosure policy applied.

## `evidence_ref`

External URI reference for evidence content.

## `expected`

Expected value compared against evidence output.

## `gate_id`

Gate identifier used in run logs and audits.

## `gates`

Gate list evaluated at a stage.

## `generated_at`

Timestamp recorded in runpack manifests.

## `hash_algorithm`

Hash algorithm used for evidence or runpack hashing.

## `include_verification`

Whether to emit a runpack verification report.

## `issue_entry_packets`

Whether to issue entry packets during scenario start.

## `jsonpath`

JSONPath selector used to extract values from JSON/YAML.

## `logical`

Logical timestamp value used for deterministic ordering.

## `manifest`

Runpack manifest metadata and hashes.

## `manifest_name`

Override for the runpack manifest file name.

## `manifest_path`

Path to the runpack manifest inside the runpack directory.

## `max_body_bytes`

Maximum JSON-RPC request size in bytes.

## `max_bytes`

Maximum file size in bytes for the JSON provider.

## `max_key_bytes`

Maximum bytes allowed for an environment key.

## `max_response_bytes`

Maximum HTTP response size in bytes.

## `max_value_bytes`

Maximum bytes allowed for an environment value.

## `on_timeout`

Timeout policy for a stage (fail or hold).

## `output_dir`

Output directory for runpack exports.

## `overrides`

Deterministic override map for environment values.

## `packet_id`

Packet identifier for disclosures.

## `params`

Provider-specific parameters for the predicate.

## `payload`

Payload content provided to tools or packets.

## `payload_ref`

Optional reference to external payload content.

## `policy_tags`

Policy labels applied to disclosures or runs.

## `predicate`

Provider predicate name to evaluate.

## `provider_id`

Evidence provider identifier registered in the MCP config.

## `record`

Record wrapper for submission responses.

## `request`

Tool request payload wrapper.

## `require_provider_opt_in`

Require provider opt-in before returning raw values.

## `requirement`

Requirement Evaluation Tree (RET) that gates must satisfy.

## `root`

Root directory for JSON/YAML file resolution.

## `run_config`

Run configuration for scenario start.

## `run_id`

Run identifier scoped to a scenario and used for state and runpacks.

## `run_state_store`

Run state store backend configuration.

## `runpack_dir`

Runpack root directory for verification.

## `runpack_export`

Export deterministic runpack artifacts for offline verification.

## `runpack_verify`

Verify runpack manifest and artifacts offline.

## `scenario_define`

Register a ScenarioSpec, validate it, and return the canonical spec hash.

## `scenario_id`

Stable scenario identifier used to register, start, and audit runs.

## `scenario_next`

Evaluate gates for an agent-driven step and advance or hold the run.

## `scenario_start`

Create a run state for a scenario and optionally issue entry packets.

## `scenario_status`

Fetch a read-only run snapshot and safe summary.

## `scenario_submit`

Submit external artifacts for audit and later checks.

## `scenario_trigger`

Submit a trigger event and evaluate the run.

## `schema_id`

Schema identifier attached to packets.

## `signature`

Evidence signature metadata (scheme, key, signature).

## `spec_hash`

Canonical SHA-256 hash of the ScenarioSpec.

## `stage_id`

Stage identifier used to evaluate gates and record decisions.

## `stages`

Ordered stages defining gate evaluation flow.

## `started_at`

Caller-supplied run start timestamp.

## `status`

Status indicator for run or verification results.

## `submission_id`

Submission identifier for external artifacts.

## `tenant_id`

Tenant identifier for isolation and policy scoping.

## `timeout`

Stage timeout duration; null means no timeout.

## `timeout_ms`

Timeout in milliseconds for HTTP checks or stores.

## `transport`

MCP transport: stdio, http, or sse.

## `trigger`

Trigger payload used to advance a run.

## `trigger_id`

Trigger identifier used for idempotent evaluation.

## `trigger_time`

Caller-supplied timestamp used by time predicates.

## `unix_millis`

Unix timestamp in milliseconds.

## `user_agent`

User agent string used for HTTP provider requests.

## `visibility_labels`

Visibility labels attached to emitted packets.

