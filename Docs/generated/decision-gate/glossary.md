# Decision Gate Glossary

Short, canonical definitions for Decision Gate terms.

## `EvidenceAnchor`

Metadata linking evidence to its source for offline verification. Contains anchor_type and anchor_value set by the provider (e.g., 'receipt_id', 'log_offset'). Anchors enable audit trails: given an anchor, you can re-query the provider (if still available) or verify against archived snapshots. Anchors are included in runpacks.

## `EvidenceContext`

Runtime context passed to evidence providers during queries. Includes tenant_id, run_id, scenario_id, stage_id, trigger_id, trigger_time, and optional correlation_id for audit correlation. Context is metadata only and does not change predicate logic.

## `EvidenceQuery`

The request sent to an evidence provider. Contains provider_id (which provider to ask), predicate (which provider check to run), and params (provider-specific arguments). The query is deterministic: same query always returns the same result given the same external state. Queries are logged for audit.

## `EvidenceRef`

An opaque URI reference pointing to evidence content stored outside the runtime. The runtime records the ref but does not fetch or resolve it; external auditors can use the URI to retrieve evidence as needed.

## `EvidenceResult`

The response from an evidence provider. Contains the evidence value (or its hash if raw disclosure is blocked), the evidence_hash for integrity, an anchor linking to the source, and optional signature metadata. Comparators evaluate against the value field. Results are captured in runpacks for replay.

## `EvidenceValue`

The actual evidence payload returned by a provider. Can be JSON (objects, arrays, strings, numbers, booleans, null) or raw bytes. The comparator interprets the value type: numeric comparisons require numbers, in_set requires the expected to be an array. Type mismatches yield unknown outcomes.

## `GateOutcome`

The tri-state result of evaluating a gate: true, false, or unknown. True means the requirement is satisfied. False means evidence contradicts the requirement. Unknown means evidence is missing, the comparator cannot evaluate (type mismatch), or the provider failed. Branching can route on any outcome; only true advances linear/fixed stages.

## `GateSpec`

Defines a single gate within a stage. Each gate has a gate_id and a requirement tree (RET expression). A gate passes only when its requirement evaluates to true under Kleene tri-state logic. Gates fail-closed: false or unknown blocks advancement. Multiple gates in a stage are evaluated together.

## `Predicate`

A leaf node in a Requirement tree that references a predicate key defined in the ScenarioSpec. When evaluated, looks up the PredicateSpec, queries the evidence provider, applies the comparator, and returns a tri-state outcome. Predicate keys should be stable and descriptive (e.g., 'env_is_prod', 'after_freeze').

## `PredicateSpec`

Binds a predicate key to an evidence query, comparator, and expected value. The predicate key is referenced by Requirement leaves. When evaluated, the runtime queries the provider, applies the comparator to the evidence, and returns true/false/unknown. Missing expected (except for exists/not_exists) yields unknown.

## `RequireGroup`

An N-of-M quorum operator in a Requirement tree. Specifies a minimum count (min) of child requirements that must pass. Uses tri-state logic: returns true when at least min children are true; returns false when even all unknowns becoming true cannot reach min; otherwise returns unknown. Use for multi-party approval, threshold signatures, or redundant checks.

## `Requirement`

A Requirement Evaluation Tree (RET) is a boolean algebra over tri-state outcomes. It composes And, Or, Not, RequireGroup, and Predicate nodes into a tree. Evaluation uses strong Kleene logic: false dominates And, true dominates Or, and unknown propagates. Gates pass only when the root evaluates to true. RETs make gate logic explicit, auditable, and replayable.

## `ScenarioSpec`

The complete specification for a deterministic decision workflow. Contains an ordered list of stages, predicate definitions, and optional policies/schemas. A ScenarioSpec is immutable once registered: its canonical JSON form is hashed to produce the spec_hash. Same spec + same evidence = same decisions, always.

## `TimeoutPolicy`

Policy controlling what happens when a stage times out. Options: 'fail' marks the run failed with a timeout reason; 'advance_with_flag' advances and sets the decision timeout flag; 'alternate_branch' routes using unknown outcomes in branch rules. Choose based on workflow criticality and routing needs.

## `advance_to`

The policy controlling how a run progresses from the current stage. Four modes: 'linear' advances to the next stage in order; 'fixed' jumps to a named stage; 'branch' routes based on gate outcomes (true/false/unknown each map to a next_stage_id); 'terminal' ends the run. Branch mode enables conditional workflows.

## `allow_default`

Permit the literal 'default' namespace (opt-in). Requires namespace.default_tenants. Production deployments should use explicit namespaces to avoid cross-tenant collisions.

## `allow_http`

Per-provider setting permitting http:// URLs for the HTTP evidence provider. Defaults to false (HTTPS only). Enable for internal health endpoints that lack TLS. Prefer HTTPS for any network-accessible services.

## `allow_insecure_http`

Permits http:// (non-TLS) URLs for MCP providers globally. Disable in production. Use only for local development or air-gapped networks. HTTP traffic is unencrypted and vulnerable to interception.

## `allow_logical`

Permits logical timestamps in time predicates instead of requiring real unix_millis. Enable for testing and simulation where deterministic time control is needed. Disable in production for real-time constraints.

## `allow_raw`

Per-provider setting allowing raw evidence disclosure. Requires allow_raw_values to be true globally. Set on providers whose evidence is safe to expose (e.g., timestamps, health status). Omit for sensitive providers (secrets, credentials).

## `allow_raw_values`

Global setting permitting raw evidence values in results. When false, providers return hashes instead of values. Enable only when consuming systems need actual evidence content. Combine with require_provider_opt_in for layered control.

## `allow_yaml`

Permits YAML parsing in the JSON evidence provider. YAML is a superset of JSON with additional syntax. Enable when config files use YAML format. Disable to restrict to pure JSON for stricter validation.

## `allowed_comparators`

Allow-list of comparators valid for this check output.

## `allowed_hosts`

Hostname allowlist for HTTP provider outbound requests. Only URLs matching these hosts are permitted. Prevents SSRF: queries to unapproved hosts fail with a security error. Required for http providers in production.

## `allowlist`

List of environment variable keys the env provider may read. Queries for keys not in the allowlist fail with a policy error. Use allowlists to limit exposure: only permit the specific variables your predicates need.

## `anchor_types`

Anchor type strings that the provider check may emit.

## `audit_enabled`

Enable structured MCP audit logging. When false, audit events are discarded. Default is true to preserve security and compliance evidence.

## `audit_path`

Filesystem path for MCP audit logs (JSON lines). When unset, audit events are written to stderr.

## `bind`

Address the MCP server binds to for HTTP or SSE transports. Format: 'host:port' (e.g., '127.0.0.1:8080', '0.0.0.0:9000'). Required when the server transport is http/sse. Omit for stdio.

## `capabilities_path`

Filesystem path to a provider contract JSON (capability contract). The contract declares supported checks, param schemas, and comparator compatibility. The runtime validates queries against the contract before dispatch. Distribute contracts alongside provider binaries.

## `comparator`

The comparison operator applied to evidence. Supported comparators: equals, not_equals, greater_than, greater_than_or_equal, less_than, less_than_or_equal, contains, in_set, exists, not_exists. All comparators except exists/not_exists require an expected value and return unknown on type mismatch. Numeric comparators return unknown for non-numbers. exists/not_exists ignore expected.

## `config_schema`

JSON Schema validating provider configuration entries.

## `contains`

True when evidence (array or string) contains the expected value.

## `content_hash`

Hash metadata for any payload content. Includes the hash algorithm and hash value. Enables integrity verification of packets, submissions, and evidence without requiring access to the raw content. Used throughout runpacks.

## `content_types`

Allowed MIME content types for evidence values or policy rule checks. Used in provider contracts and policy rules to constrain payload formats.

## `correlation_id`

Identifier linking related requests and decisions across systems. Pass a correlation_id with triggers to trace decision flows through logs, metrics, and external services. Propagated in EvidenceContext for provider logging.

## `decision`

The recorded outcome of a trigger evaluation. Contains decision_id, seq, trigger_id, stage_id, decided_at, and a DecisionOutcome (start/advance/hold/fail/complete). Advance outcomes include a timeout flag when triggered by timeouts. Decisions form the audit trail for run progression.

## `decision_id`

Unique identifier for a recorded decision. Generated when a trigger evaluation produces an outcome and linked to the decision sequence, trigger_id, and stage_id. Decision IDs enable audit trails and debugging.

## `deep_equals`

Deep structural equality for JSON objects and arrays.

## `deep_not_equals`

Deep structural inequality for JSON objects and arrays.

## `default`

Default policy effect applied when no rules match. Defaults to 'deny' for fail-closed behavior.

## `default_policy`

Default trust policy for evidence providers. Options: 'audit' or 'require_signature' (with key list). Individual providers can override. Start with 'audit' and tighten per-provider as needed.

## `default_tenants`

Allowlist of tenant IDs permitted to use the literal 'default' namespace. Required when allow_default is true; empty list is rejected.

## `denylist`

List of environment variable keys the env provider must never read. Queries for denied keys fail immediately. Use denylists for defense-in-depth: block known-sensitive keys (API_KEY, SECRET_*, etc.) even if accidentally queried.

## `description`

Short summary describing provider behavior and intent.

## `determinism`

Provider check output stability: deterministic, time_dependent, or external.

## `dispatch_targets`

Destinations where emitted packets are delivered. Configure targets in run_config: agent, session, external, or channel. Multiple targets enable fan-out to different systems.

## `effect`

Policy rule effect: 'permit', 'deny', or 'error' (fail closed with a policy error).

## `engine`

Dispatch policy engine selection. Options: 'permit_all', 'deny_all', or 'static'. Use 'static' to apply rule-based authorization. Additional engines can be added via adapters without changing core.

## `entry_packets`

Disclosure packets emitted when a run enters a stage. Use entry_packets to release information at specific workflow points: e.g., reveal configuration after approval, emit audit events, or trigger downstream systems. Packets include payload, schema_id, and visibility_labels for access control.

## `equals`

True when evidence equals expected (numbers, strings, booleans, or JSON values).

## `error_message`

Error message to report when effect is 'error'. Required for error rules.

## `evidence_hash`

SHA-256 hash of an evidence value for integrity verification. Computed over the canonical form of EvidenceValue. Evidence hashes enable verification without exposing raw values: auditors can confirm evidence matched expectations even when raw disclosure is blocked.

## `evidence_query`

Queries an evidence provider with the configured disclosure policy applied. Returns the EvidenceResult containing the value (or hash), anchor metadata, and optional signature. Use this for debugging predicates or building custom gates outside the standard scenario flow.

## `examples`

Example check invocations with params and results.

## `exists`

True when evidence value is present. Expected is ignored.

## `expected`

The target value compared against evidence output. Type must match the evidence type: JSON values for equals/in_set, numbers for greater_than, arrays for in_set (evidence matches any element). If expected is missing or mismatched, the comparator returns unknown (fail-closed). Not required for exists/not_exists.

## `forbid_labels`

Visibility labels that must not be present for the rule to match.

## `forbid_policy_tags`

Policy tags that must not be present for the rule to match.

## `gate_id`

Identifier for a gate within a stage. Must be unique within the stage. Appears in run logs, decisions, and branch routing. Use descriptive names reflecting what the gate checks: 'env_gate', 'time_gate', 'approval_gate'.

## `gates`

The list of GateSpecs evaluated when a run enters a stage. All gates are evaluated together (not short-circuited). The stage's advance_to policy determines how gate outcomes affect progression. Multiple gates enable parallel checks: e.g., verify both time constraints and approvals before advancing.

## `generated_at`

Timestamp recorded in the runpack manifest indicating when the export was created. Expressed as ISO 8601. Useful for audit logs and freshness checks. Does not affect verification outcome.

## `greater_than`

True when numeric evidence is greater than expected.

## `greater_than_or_equal`

True when numeric evidence is greater than or equal to expected.

## `hash_algorithm`

Algorithm used for hashing evidence and runpack artifacts. Currently SHA-256 exclusively. Recorded in manifests for forward compatibility. Do not assume other algorithms without checking this field.

## `in_set`

True when evidence is contained in the expected array.

## `include_verification`

Flag indicating whether to emit a verification report alongside the runpack export. When true, runpack_export also runs verification and includes the report. Useful for immediate validation after export.

## `issue_entry_packets`

Flag controlling whether scenario_start emits entry_packets for the initial stage. Set false to defer packet emission until the first trigger or next. Useful when runs need setup before disclosures begin.

## `jsonpath`

JSONPath selector used by the JSON provider to extract values from documents. Syntax follows RFC 9535. Examples: '$.version', '$.config.features[*].name'. The extracted value becomes the evidence for comparator evaluation.

## `less_than`

True when numeric evidence is less than expected.

## `less_than_or_equal`

True when numeric evidence is less than or equal to expected.

## `lex_greater_than`

Lexicographic string compare: true when evidence sorts after expected.

## `lex_greater_than_or_equal`

Lexicographic string compare: true when evidence sorts after or equals expected.

## `lex_less_than`

Lexicographic string compare: true when evidence sorts before expected.

## `lex_less_than_or_equal`

Lexicographic string compare: true when evidence sorts before or equals expected.

## `log_precheck_payloads`

Explicit opt-in to log raw precheck request/response payloads. Default false; hash-only auditing is always emitted when audit is enabled.

## `logical`

A logical timestamp value used for deterministic ordering when wall-clock time is unavailable. Caller-supplied integers (>= 0) are accepted only when allow_logical is enabled. Useful for testing and simulation.

## `manifest`

The runpack manifest containing metadata and hashes for all artifacts. Lists every file in the runpack with its SHA-256 hash, plus generation timestamp and spec_hash reference. Verification compares computed hashes against the manifest to detect tampering or missing files.

## `manifest_name`

Override filename for the runpack manifest. Defaults to 'manifest.json'. Customize when exporting multiple runpacks to the same directory or when integrating with systems that expect specific filenames.

## `manifest_path`

Path to the manifest file inside the runpack directory. Used by runpack_verify to locate the manifest. Typically 'manifest.json' at the runpack root. The verifier reads this file first to discover all other artifacts.

## `max_body_bytes`

Maximum request body size in bytes for JSON-RPC requests. Prevents oversized payloads from exhausting server resources. Requests exceeding this are rejected before processing. Configure based on expected payload sizes.

## `max_bytes`

Maximum file size in bytes the JSON provider will read. Prevents resource exhaustion from oversized files. Queries for files exceeding this limit fail with a validation error. Size appropriately for your config files.

## `max_key_bytes`

Maximum byte length for environment variable keys queried by the env provider. Prevents resource exhaustion from pathological key names. Defaults to a reasonable limit. Queries exceeding this fail with a validation error.

## `max_response_bytes`

Maximum HTTP response body size the HTTP provider will read. Prevents memory exhaustion from unbounded responses. Responses exceeding this are truncated or rejected. Size for typical health check or API responses.

## `max_value_bytes`

Maximum byte length for environment variable values returned by the env provider. Prevents oversized values from bloating evidence results. Values exceeding this are truncated or rejected per provider config.

## `mode`

Server operating mode: 'strict' (default) or legacy 'dev_permissive'. Prefer the explicit dev.permissive toggle. Dev-permissive relaxes asserted evidence only and does not auto-allow the default namespace.

## `name`

Human-readable provider name shown in docs and UIs.

## `not_equals`

True when evidence does not equal expected.

## `not_exists`

True when evidence value is missing. Expected is ignored.

## `notes`

Optional notes about provider behavior or determinism.

## `on_timeout`

Stage timeout policy (TimeoutPolicy). Always present in StageSpec and used only when timeout is set; ignored when timeout is null. Supports 'fail', 'advance_with_flag', or 'alternate_branch' behaviors.

## `output_dir`

Directory where runpack_export writes the audit bundle for filesystem storage. Optional when managed runpack storage is configured. Ensure write permissions and sufficient disk space. Existing files may be overwritten.

## `overrides`

Deterministic override map for environment values during testing. Keys in overrides replace real environment lookups, ensuring reproducible evidence. Use for CI/CD where environment varies across runners.

## `packet_id`

Identifier for a disclosure packet. Must be unique within the scenario. Used to track emissions, filter by packet type, and correlate with dispatch_targets. Referenced in entry_packets and disclosure logs.

## `packet_ids`

Packet identifiers allowed by the rule.

## `params`

Provider-specific parameters passed to a check. Structure varies by provider: env.get needs {key}, time.after needs {timestamp}, http.status needs {url}. Invalid or missing required params cause the provider to fail, yielding an unknown outcome.

## `params_required`

Whether EvidenceQuery.params must be supplied for this check.

## `params_schema`

JSON Schema for provider check params payloads.

## `payload`

The content body of a packet, submission, or trigger payload. Encoded as PacketPayload: json, bytes, or external content_ref (uri + content_hash, optional encryption). Payloads are hashed for integrity and may be schema-validated before emission.

## `permissive`

Explicit dev-only toggle for allowing asserted evidence. Use only in local development or controlled test environments; emits warnings and audit metadata.

## `permissive_exempt_providers`

Provider IDs exempt from dev-permissive relaxations (e.g., Asset Core providers).

## `permissive_scope`

Dev-permissive scope selector. Currently fixed to asserted_evidence_only for v1.

## `permissive_ttl_days`

Optional TTL (days) for dev-permissive warnings. Uses config mtime to emit expiry warnings when the TTL is exceeded.

## `permissive_warn`

Emit warnings and security audit events when dev-permissive is enabled or expired.

## `policy_tags`

Labels applied to runs, predicates, or disclosures for policy routing. Tags enable conditional behavior: different disclosure rules per environment, tenant-specific rate limits, or audit categories. Define tags in the ScenarioSpec and apply them to runs, predicates, packets, or timeouts as needed.

## `precheck`

Evaluates a scenario against asserted data without mutating run state. Validates asserted data against a registered shape and returns the decision result for simulation.

## `predicate`

The provider check name to evaluate within a provider. Each provider exposes named checks (e.g., 'get' for env, 'after' for time, 'status' for http). The check determines what the provider returns and what params it accepts. See providers.json for the complete check catalog per provider.

## `predicates`

List of provider checks exposed by the provider contract.

## `provider_contract_get`

Fetches the canonical provider contract JSON and its hash for a provider. Use this to discover predicate schemas, comparator allow-lists, and examples. Disclosure is controlled by authz and provider contract visibility policy.

## `provider_id`

Identifier for an evidence provider registered in decision-gate.toml. Providers supply predicates: 'time' for timestamps, 'env' for environment variables, 'http' for health checks, 'json' for file queries. Custom providers can be registered via MCP configuration.

## `provider_schema_get`

Fetches predicate-level schema details for a provider (params schema, result schema, comparator allow-lists, and examples). Use this for authoring forms or LLM guidance without loading the full provider contract.

## `providers_list`

Lists registered evidence providers and their capabilities summary. Returns provider identifiers, transport metadata, and policy-scoped visibility. Use this to discover available providers and supported checks.

## `record`

Wrapper for submission responses. Contains submission_id, run_id, payload, content_type, content_hash, submitted_at, and correlation_id. Records prove artifacts were submitted and hashed for audit.

## `request`

Wrapper for tool request payloads. Used in MCP protocol messages. Contains the tool name, parameters, and request metadata. Internal structure; most users interact via higher-level scenario_* tools.

## `require_labels`

Visibility labels that must be present for the rule to match.

## `require_policy_tags`

Policy tags that must be present for the rule to match.

## `require_provider_opt_in`

Requires providers to explicitly opt into raw disclosure via allow_raw in their config. Even if allow_raw_values is true globally, providers without opt-in return hashes. Defense-in-depth for sensitive evidence.

## `requirement`

The RET expression that a gate must satisfy. This field contains the root of a Requirement tree (And/Or/Not/RequireGroup/Predicate). The gate passes only when the entire tree evaluates to true. Design requirements to handle unknown outcomes explicitly via branching or RequireGroup thresholds.

## `result`

Example output value for a check invocation.

## `result_schema`

JSON Schema for provider check output values.

## `root`

Base directory for JSON provider file resolution. File paths in queries are resolved relative to root. Prevents directory traversal: paths escaping root fail with a security error. Set to the config directory or a dedicated data folder.

## `rules`

Ordered list of policy rules. The first rule that matches the dispatch request wins.

## `run_config`

Configuration provided when starting a run via scenario_start. Includes tenant_id, run_id, scenario_id, dispatch_targets, and policy_tags. Run config is immutable after start and recorded in runpacks for reproducibility.

## `run_id`

Unique identifier scoping a single execution of a scenario. Generated at scenario_start and used for all subsequent operations. Run IDs appear in runpacks, decisions, and audit logs. Keep run_ids for correlation and incident response.

## `run_state_store`

Backend configuration for persisting run state. Options include in-memory (for testing) or SQLite. The store holds run progress, decision history, and pending triggers. Configure durability and retention based on compliance needs.

## `runpack_dir`

Root directory of a runpack for verification. runpack_verify reads the manifest from this location and checks all referenced artifacts. Point to the directory containing manifest.json.

## `runpack_export`

Exports a deterministic audit bundle (runpack) containing the scenario spec, all triggers, gate evaluations, decisions, and disclosure packets. The manifest includes SHA-256 hashes of every artifact. Runpacks enable offline verification: anyone can replay the decision logic and confirm the same outcomes.

## `runpack_verify`

Verifies a runpack's manifest and artifacts offline. Checks that all hashes match, the decision sequence is internally consistent, and no artifacts are missing or tampered. Returns a verification report. Use this for compliance audits, incident review, or CI/CD gate validation.

## `scenario_define`

Registers a ScenarioSpec with the runtime and returns its canonical spec_hash. The runtime validates the spec structure, checks that all referenced predicates and providers exist, and computes a SHA-256 hash of the canonical JSON form. Store the spec_hash for audit: it proves which exact spec governed a run.

## `scenario_id`

Stable identifier for a scenario across its lifecycle: registration, runs, and audits. Choose descriptive, versioned IDs (e.g., 'deployment-gate-v2'). The scenario_id plus spec_hash together identify exactly which workflow definition was used.

## `scenario_ids`

Scenario identifiers allowed by the rule.

## `scenario_next`

Evaluates gates for the current stage and advances or holds the run. This is the primary driver for agent-controlled workflows. All gates must be true to advance; otherwise the run holds. Branch stages use gate outcomes to select the next_stage_id once gates pass. Timeout policies may synthesize outcomes for alternate_branch routing. Returns the decision and new stage.

## `scenario_start`

Creates a new run state for a registered scenario. Initializes the run at the first stage and optionally emits entry_packets as disclosures. Returns a run_id that scopes all subsequent operations. The run starts in the active state with stage_entered_at set from started_at.

## `scenario_status`

Fetches a read-only snapshot of a run without modifying it. Returns current_stage_id, status, last_decision, issued_packet_ids, and an optional safe_summary for UI displays. Use this for dashboards, polling, and debugging. The response omits raw evidence values.

## `scenario_submit`

Submits external artifacts to a run's audit trail for later review. Use this to attach documents, signatures, or receipts for audit and runpack export. Payloads are hashed into content_hash and recorded in the submission log. Submissions are idempotent by submission_id; conflicting payloads return a conflict error.

## `scenario_trigger`

Submits a trigger event with an explicit timestamp and evaluates the run. Unlike scenario_next, triggers carry kind, source_id, and optional payload metadata for time-based predicates and auditing. The trigger_id ensures idempotent processing: repeated calls with the same trigger_id return the cached decision.

## `scenarios_list`

Lists registered scenarios for a tenant and namespace. Returns scenario identifiers and spec hashes to support inventory and audit.

## `schema_id`

Identifier for a schema attached to packets. Schemas validate payload structure before emission. Register schemas in the ScenarioSpec's schemas array. Packets reference schemas by schema_id for type safety and documentation.

## `schema_ids`

Schema identifiers allowed by the rule.

## `schemas_get`

Fetches a specific data shape by schema_id and version for a tenant and namespace. Fails closed when the schema is missing.

## `schemas_list`

Lists registered data shapes for a tenant and namespace. Supports pagination via cursor and limit. Use this to discover available schema versions.

## `schemas_register`

Registers a data shape schema for a tenant and namespace. Schemas are immutable; re-registering the same version fails. Include created_at to capture when the schema was authored.

## `signature`

Cryptographic signature metadata attached to evidence. Contains the signature scheme (e.g., ed25519), the public key identifier, and the signature bytes. Enables providers to prove evidence authenticity. Verifiers can check signatures offline using the anchored key reference.

## `spec_hash`

A canonical SHA-256 hash of the ScenarioSpec in deterministic JSON form. Two specs with identical content always produce the same hash regardless of field ordering. Store the spec_hash when starting a run: it proves exactly which spec version governed the decisions. Essential for audit and compliance.

## `stage_entered_at`

Timestamp recorded when the run entered the current stage. Used to evaluate stage timeouts and for audit replay. Set at scenario_start and updated on advances.

## `stage_id`

Identifier for a stage within a scenario. Must be unique within the ScenarioSpec. Referenced by advance_to policies and branch targets. Stage IDs appear in run status, decisions, and entry_packet emissions. Use descriptive names: 'approval', 'verification', 'release'.

## `stage_ids`

Stage identifiers allowed by the rule.

## `stages`

An ordered list of decision phases in a ScenarioSpec. Each stage contains gates to evaluate and an advance_to policy. Runs progress through stages sequentially unless branching redirects them. Stages isolate concerns: early stages might check prerequisites, middle stages verify conditions, final stages authorize actions.

## `started_at`

Caller-supplied timestamp marking when the run began. Required at scenario_start and used for timing calculations, stage_entered_at, and audit records. Prefer explicit timestamps for deterministic replay.

## `static`

Static dispatch policy configuration. Applies when policy.engine = 'static'.

## `status`

Current state indicator for a run or verification. Run statuses: 'active', 'completed', 'failed'. Verification statuses: 'pass', 'fail'. Check status to determine next actions or surface issues.

## `storage_uri`

Optional storage location returned by managed runpack storage backends (for example, s3://bucket/tenant/namespace/run/runpack.tar). Present only when the server is configured to export runpacks to object storage.

## `submission_id`

Identifier for an external artifact submitted via scenario_submit. Must be unique within the run. Enables idempotent submissions: repeated calls with the same payload return the existing record, while conflicting payloads return an error.

## `system`

External system name for dispatch targets.

## `target`

External target identifier for dispatch targets.

## `target_id`

Target identifier for agent/session/channel selectors.

## `target_kind`

Target kind for an explicit selector. Options: 'agent', 'session', 'external', 'channel'.

## `target_kinds`

Target kinds allowed by the rule. Options: 'agent', 'session', 'external', 'channel'.

## `targets`

Explicit target selectors for dispatch authorization.

## `tenant_id`

Identifier for tenant isolation in multi-tenant deployments. Scopes runs, state stores, and policies. Each tenant's data is logically separated. Defaults to a single tenant if not specified. Required for SaaS and shared-infrastructure patterns.

## `timeout`

Stage timeout configuration. Set to a TimeoutSpec containing timeout_ms and policy_tags, or null for no timeout. Timeouts prevent runs from stalling indefinitely and are handled by on_timeout.

## `timeout_ms`

Timeout in milliseconds. Used for stage timeouts (TimeoutSpec) and for HTTP provider requests. Operations exceeding this duration fail per policy.

## `transport`

MCP transport protocol for provider communication: 'stdio' for subprocess pipes, 'http' for JSON-RPC over HTTP, 'sse' for server-sent events. Choose based on deployment: stdio for local providers, http/sse for remote services. Configure in decision-gate.toml [[providers]].

## `trigger`

Event payload submitted via scenario_trigger to advance a run. Contains trigger_id, run_id, kind, time, source_id, optional payload, and correlation_id. Triggers are recorded for audit replay.

## `trigger_id`

Identifier ensuring idempotent trigger processing. Repeated calls with the same trigger_id return the cached decision without re-evaluation. Use UUIDs or event-derived IDs. Critical for safe retries in distributed systems.

## `trigger_time`

Caller-supplied timestamp from the trigger event used by time predicates and EvidenceContext. Uses the Timestamp type (unix_millis or logical when allowed) to avoid wall-clock reads. Auditors can replay runs with the recorded trigger_time.

## `unix_millis`

Unix timestamp expressed in milliseconds since epoch (1970-01-01 UTC). Standard format for trigger_time and time predicates. Millisecond precision enables sub-second scheduling. Convert from ISO 8601: parse to Date, call getTime().

## `user_agent`

User-Agent header string for HTTP provider requests. Defaults to a Decision Gate identifier. Customize for API requirements, rate-limit identification, or debugging. Appears in external service logs.

## `visibility_labels`

Access control labels attached to emitted packets. Labels are available to policy deciders and downstream consumers to implement disclosure rules. Use for graduated disclosure: some systems see summaries, others see full details.

