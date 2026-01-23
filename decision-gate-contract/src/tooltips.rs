// decision-gate-contract/src/tooltips.rs
// ============================================================================
// Module: Tooltip Catalog
// Description: Canonical tooltip strings for Decision Gate docs and UI.
// Purpose: Provide a stable tooltip manifest for documentation rendering.
// Dependencies: decision-gate-contract::types
// ============================================================================

//! ## Overview
//! Tooltips provide short, reusable explanations for UI and documentation
//! surfaces. Terms are kept stable and ASCII-only to support localization.
//! Security posture: tooltips are static content; see
//! `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use crate::types::TooltipEntry;
use crate::types::TooltipsManifest;

// ============================================================================
// SECTION: Tooltip Catalog
// ============================================================================

/// Tooltip schema version included in the manifest payload.
const TOOLTIP_VERSION: &str = "1.0.0";

/// Builds a tooltip entry from a term and description.
fn entry(term: &str, description: &str) -> TooltipEntry {
    TooltipEntry {
        term: term.to_string(),
        title: term.to_string(),
        description: description.to_string(),
    }
}

/// Returns the canonical tooltip manifest.
#[allow(clippy::too_many_lines, reason = "flat tooltip catalog is naturally long")]
#[must_use]
pub fn tooltips_manifest() -> TooltipsManifest {
    let mut entries = vec![
        // =====================================================================
        // MCP TOOLS - Scenario Lifecycle
        // =====================================================================
        entry(
            "scenario_define",
            "Registers a ScenarioSpec with the runtime and returns its canonical spec_hash. The \
             runtime validates the spec structure, checks that all referenced predicates and \
             providers exist, and computes a SHA-256 hash of the canonical JSON form. Store the \
             spec_hash for audit: it proves which exact spec governed a run.",
        ),
        entry(
            "scenario_start",
            "Creates a new run state for a registered scenario. Initializes the run at the first \
             stage and optionally emits entry_packets as disclosures. Returns a run_id that \
             scopes all subsequent operations. The run starts in the active state with \
             stage_entered_at set from started_at.",
        ),
        entry(
            "scenario_status",
            "Fetches a read-only snapshot of a run without modifying it. Returns \
             current_stage_id, status, last_decision, issued_packet_ids, and an optional \
             safe_summary for UI displays. Use this for dashboards, polling, and debugging. The \
             response omits raw evidence values.",
        ),
        entry(
            "scenario_next",
            "Evaluates gates for the current stage and advances or holds the run. This is the \
             primary driver for agent-controlled workflows. All gates must be true to advance; \
             otherwise the run holds. Branch stages use gate outcomes to select the next_stage_id \
             once gates pass. Timeout policies may synthesize outcomes for alternate_branch \
             routing. Returns the decision and new stage.",
        ),
        entry(
            "scenario_submit",
            "Submits external artifacts to a run's audit trail for later review. Use this to \
             attach documents, signatures, or receipts for audit and runpack export. Payloads are \
             hashed into content_hash and recorded in the submission log. Submissions are \
             idempotent by submission_id; conflicting payloads return a conflict error.",
        ),
        entry(
            "scenario_trigger",
            "Submits a trigger event with an explicit timestamp and evaluates the run. Unlike \
             scenario_next, triggers carry kind, source_id, and optional payload metadata for \
             time-based predicates and auditing. The trigger_id ensures idempotent processing: \
             repeated calls with the same trigger_id return the cached decision.",
        ),
        entry(
            "evidence_query",
            "Queries an evidence provider with the configured disclosure policy applied. Returns \
             the EvidenceResult containing the value (or hash), anchor metadata, and optional \
             signature. Use this for debugging predicates or building custom gates outside the \
             standard scenario flow.",
        ),
        entry(
            "runpack_export",
            "Exports a deterministic audit bundle (runpack) containing the scenario spec, all \
             triggers, gate evaluations, decisions, and disclosure packets. The manifest includes \
             SHA-256 hashes of every artifact. Runpacks enable offline verification: anyone can \
             replay the decision logic and confirm the same outcomes.",
        ),
        entry(
            "runpack_verify",
            "Verifies a runpack's manifest and artifacts offline. Checks that all hashes match, \
             the decision sequence is internally consistent, and no artifacts are missing or \
             tampered. Returns a verification report. Use this for compliance audits, incident \
             review, or CI/CD gate validation.",
        ),
        // =====================================================================
        // CORE TYPES - Scenario & Stage Specifications
        // =====================================================================
        entry(
            "ScenarioSpec",
            "The complete specification for a deterministic decision workflow. Contains an \
             ordered list of stages, predicate definitions, and optional policies/schemas. A \
             ScenarioSpec is immutable once registered: its canonical JSON form is hashed to \
             produce the spec_hash. Same spec + same evidence = same decisions, always.",
        ),
        entry(
            "GateSpec",
            "Defines a single gate within a stage. Each gate has a gate_id and a requirement tree \
             (RET expression). A gate passes only when its requirement evaluates to true under \
             Kleene tri-state logic. Gates fail-closed: false or unknown blocks advancement. \
             Multiple gates in a stage are evaluated together.",
        ),
        entry(
            "PredicateSpec",
            "Binds a predicate key to an evidence query, comparator, and expected value. The \
             predicate key is referenced by Requirement leaves. When evaluated, the runtime \
             queries the provider, applies the comparator to the evidence, and returns \
             true/false/unknown. Missing expected (except for exists/not_exists) yields unknown.",
        ),
        // =====================================================================
        // RET SYSTEM - Requirement Evaluation Trees
        // =====================================================================
        entry(
            "Requirement",
            "A Requirement Evaluation Tree (RET) is a boolean algebra over tri-state outcomes. It \
             composes And, Or, Not, RequireGroup, and Predicate nodes into a tree. Evaluation \
             uses strong Kleene logic: false dominates And, true dominates Or, and unknown \
             propagates. Gates pass only when the root evaluates to true. RETs make gate logic \
             explicit, auditable, and replayable.",
        ),
        entry(
            "requirement",
            "The RET expression that a gate must satisfy. This field contains the root of a \
             Requirement tree (And/Or/Not/RequireGroup/Predicate). The gate passes only when the \
             entire tree evaluates to true. Design requirements to handle unknown outcomes \
             explicitly via branching or RequireGroup thresholds.",
        ),
        entry(
            "RequireGroup",
            "An N-of-M quorum operator in a Requirement tree. Specifies a minimum count (min) of \
             child requirements that must pass. Uses tri-state logic: returns true when at least \
             min children are true; returns false when even all unknowns becoming true cannot \
             reach min; otherwise returns unknown. Use for multi-party approval, threshold \
             signatures, or redundant checks.",
        ),
        entry(
            "Predicate",
            "A leaf node in a Requirement tree that references a predicate key defined in the \
             ScenarioSpec. When evaluated, looks up the PredicateSpec, queries the evidence \
             provider, applies the comparator, and returns a tri-state outcome. Predicate keys \
             should be stable and descriptive (e.g., 'env_is_prod', 'after_freeze').",
        ),
        entry(
            "GateOutcome",
            "The tri-state result of evaluating a gate: true, false, or unknown. True means the \
             requirement is satisfied. False means evidence contradicts the requirement. Unknown \
             means evidence is missing, the comparator cannot evaluate (type mismatch), or the \
             provider failed. Branching can route on any outcome; only true advances linear/fixed \
             stages.",
        ),
        // =====================================================================
        // EVIDENCE CHAIN - Queries, Results, and Anchors
        // =====================================================================
        entry(
            "comparator",
            "The comparison operator applied to evidence. Supported comparators: equals, \
             not_equals, greater_than, greater_than_or_equal, less_than, less_than_or_equal, \
             contains, in_set, exists, not_exists. All comparators except exists/not_exists \
             require an expected value and return unknown on type mismatch. Numeric comparators \
             return unknown for non-numbers. exists/not_exists ignore expected.",
        ),
        entry(
            "expected",
            "The target value compared against evidence output. Type must match the evidence \
             type: JSON values for equals/in_set, numbers for greater_than, arrays for in_set \
             (evidence matches any element). If expected is missing or mismatched, the comparator \
             returns unknown (fail-closed). Not required for exists/not_exists.",
        ),
        entry(
            "EvidenceQuery",
            "The request sent to an evidence provider. Contains provider_id (which provider to \
             ask), predicate (which check to run), and params (provider-specific arguments). The \
             query is deterministic: same query always returns the same result given the same \
             external state. Queries are logged for audit.",
        ),
        entry(
            "EvidenceContext",
            "Runtime context passed to evidence providers during queries. Includes tenant_id, \
             run_id, scenario_id, stage_id, trigger_id, trigger_time, and optional correlation_id \
             for audit correlation. Context is metadata only and does not change predicate logic.",
        ),
        entry(
            "EvidenceResult",
            "The response from an evidence provider. Contains the evidence value (or its hash if \
             raw disclosure is blocked), the evidence_hash for integrity, an anchor linking to \
             the source, and optional signature metadata. Comparators evaluate against the value \
             field. Results are captured in runpacks for replay.",
        ),
        entry(
            "EvidenceValue",
            "The actual evidence payload returned by a provider. Can be JSON (objects, arrays, \
             strings, numbers, booleans, null) or raw bytes. The comparator interprets the value \
             type: numeric comparisons require numbers, in_set requires the expected to be an \
             array. Type mismatches yield unknown outcomes.",
        ),
        entry(
            "EvidenceAnchor",
            "Metadata linking evidence to its source for offline verification. Contains \
             anchor_type and anchor_value set by the provider (e.g., 'receipt_id', 'log_offset'). \
             Anchors enable audit trails: given an anchor, you can re-query the provider (if \
             still available) or verify against archived snapshots. Anchors are included in \
             runpacks.",
        ),
        entry(
            "EvidenceRef",
            "An opaque URI reference pointing to evidence content stored outside the runtime. The \
             runtime records the ref but does not fetch or resolve it; external auditors can use \
             the URI to retrieve evidence as needed.",
        ),
        entry(
            "predicate",
            "The predicate name to evaluate within a provider. Each provider exposes named \
             predicates (e.g., 'get' for env, 'after' for time, 'status' for http). The predicate \
             determines what the provider checks and what params it accepts. See providers.json \
             for the complete predicate catalog per provider.",
        ),
        entry(
            "params",
            "Provider-specific parameters passed to a predicate. Structure varies by provider: \
             env.get needs {key}, time.after needs {timestamp}, http.status needs {url}. Invalid \
             or missing required params cause the provider to fail, yielding an unknown outcome.",
        ),
        // =====================================================================
        // FLOW CONTROL - Stages and Advancement
        // =====================================================================
        entry(
            "stages",
            "An ordered list of decision phases in a ScenarioSpec. Each stage contains gates to \
             evaluate and an advance_to policy. Runs progress through stages sequentially unless \
             branching redirects them. Stages isolate concerns: early stages might check \
             prerequisites, middle stages verify conditions, final stages authorize actions.",
        ),
        entry(
            "gates",
            "The list of GateSpecs evaluated when a run enters a stage. All gates are evaluated \
             together (not short-circuited). The stage's advance_to policy determines how gate \
             outcomes affect progression. Multiple gates enable parallel checks: e.g., verify \
             both time constraints and approvals before advancing.",
        ),
        entry(
            "advance_to",
            "The policy controlling how a run progresses from the current stage. Four modes: \
             'linear' advances to the next stage in order; 'fixed' jumps to a named stage; \
             'branch' routes based on gate outcomes (true/false/unknown each map to a \
             next_stage_id); 'terminal' ends the run. Branch mode enables conditional workflows.",
        ),
        entry(
            "entry_packets",
            "Disclosure packets emitted when a run enters a stage. Use entry_packets to release \
             information at specific workflow points: e.g., reveal configuration after approval, \
             emit audit events, or trigger downstream systems. Packets include payload, \
             schema_id, and visibility_labels for access control.",
        ),
        // =====================================================================
        // AUDIT & DETERMINISM
        // =====================================================================
        entry(
            "spec_hash",
            "A canonical SHA-256 hash of the ScenarioSpec in deterministic JSON form. Two specs \
             with identical content always produce the same hash regardless of field ordering. \
             Store the spec_hash when starting a run: it proves exactly which spec version \
             governed the decisions. Essential for audit and compliance.",
        ),
        entry(
            "evidence_hash",
            "SHA-256 hash of an evidence value for integrity verification. Computed over the \
             canonical form of EvidenceValue. Evidence hashes enable verification without \
             exposing raw values: auditors can confirm evidence matched expectations even when \
             raw disclosure is blocked.",
        ),
        entry(
            "content_hash",
            "Hash metadata for any payload content. Includes the hash algorithm and hash value. \
             Enables integrity verification of packets, submissions, and evidence without \
             requiring access to the raw content. Used throughout runpacks.",
        ),
        entry(
            "manifest",
            "The runpack manifest containing metadata and hashes for all artifacts. Lists every \
             file in the runpack with its SHA-256 hash, plus generation timestamp and spec_hash \
             reference. Verification compares computed hashes against the manifest to detect \
             tampering or missing files.",
        ),
        entry(
            "manifest_name",
            "Override filename for the runpack manifest. Defaults to 'manifest.json'. Customize \
             when exporting multiple runpacks to the same directory or when integrating with \
             systems that expect specific filenames.",
        ),
        entry(
            "manifest_path",
            "Path to the manifest file inside the runpack directory. Used by runpack_verify to \
             locate the manifest. Typically 'manifest.json' at the runpack root. The verifier \
             reads this file first to discover all other artifacts.",
        ),
        entry(
            "trigger_time",
            "Caller-supplied timestamp from the trigger event used by time predicates and \
             EvidenceContext. Uses the Timestamp type (unix_millis or logical when allowed) to \
             avoid wall-clock reads. Auditors can replay runs with the recorded trigger_time.",
        ),
        entry(
            "logical",
            "A logical timestamp value used for deterministic ordering when wall-clock time is \
             unavailable. Caller-supplied integers (>= 0) are accepted only when allow_logical is \
             enabled. Useful for testing and simulation.",
        ),
        entry(
            "signature",
            "Cryptographic signature metadata attached to evidence. Contains the signature scheme \
             (e.g., ed25519), the public key identifier, and the signature bytes. Enables \
             providers to prove evidence authenticity. Verifiers can check signatures offline \
             using the anchored key reference.",
        ),
        // =====================================================================
        // IDENTIFIERS
        // =====================================================================
        entry(
            "scenario_id",
            "Stable identifier for a scenario across its lifecycle: registration, runs, and \
             audits. Choose descriptive, versioned IDs (e.g., 'deployment-gate-v2'). The \
             scenario_id plus spec_hash together identify exactly which workflow definition was \
             used.",
        ),
        entry(
            "run_id",
            "Unique identifier scoping a single execution of a scenario. Generated at \
             scenario_start and used for all subsequent operations. Run IDs appear in runpacks, \
             decisions, and audit logs. Keep run_ids for correlation and incident response.",
        ),
        entry(
            "stage_id",
            "Identifier for a stage within a scenario. Must be unique within the ScenarioSpec. \
             Referenced by advance_to policies and branch targets. Stage IDs appear in run \
             status, decisions, and entry_packet emissions. Use descriptive names: 'approval', \
             'verification', 'release'.",
        ),
        entry(
            "trigger_id",
            "Identifier ensuring idempotent trigger processing. Repeated calls with the same \
             trigger_id return the cached decision without re-evaluation. Use UUIDs or \
             event-derived IDs. Critical for safe retries in distributed systems.",
        ),
        entry(
            "tenant_id",
            "Identifier for tenant isolation in multi-tenant deployments. Scopes runs, state \
             stores, and policies. Each tenant's data is logically separated. Defaults to a \
             single tenant if not specified. Required for SaaS and shared-infrastructure patterns.",
        ),
        entry(
            "gate_id",
            "Identifier for a gate within a stage. Must be unique within the stage. Appears in \
             run logs, decisions, and branch routing. Use descriptive names reflecting what the \
             gate checks: 'env_gate', 'time_gate', 'approval_gate'.",
        ),
        entry(
            "decision_id",
            "Unique identifier for a recorded decision. Generated when a trigger evaluation \
             produces an outcome and linked to the decision sequence, trigger_id, and stage_id. \
             Decision IDs enable audit trails and debugging.",
        ),
        entry(
            "packet_id",
            "Identifier for a disclosure packet. Must be unique within the scenario. Used to \
             track emissions, filter by packet type, and correlate with dispatch_targets. \
             Referenced in entry_packets and disclosure logs.",
        ),
        entry(
            "provider_id",
            "Identifier for an evidence provider registered in decision-gate.toml. Providers \
             supply predicates: 'time' for timestamps, 'env' for environment variables, 'http' \
             for health checks, 'json' for file queries. Custom providers can be registered via \
             MCP configuration.",
        ),
        entry(
            "correlation_id",
            "Identifier linking related requests and decisions across systems. Pass a \
             correlation_id with triggers to trace decision flows through logs, metrics, and \
             external services. Propagated in EvidenceContext for provider logging.",
        ),
        entry(
            "submission_id",
            "Identifier for an external artifact submitted via scenario_submit. Must be unique \
             within the run. Enables idempotent submissions: repeated calls with the same payload \
             return the existing record, while conflicting payloads return an error.",
        ),
        entry(
            "schema_id",
            "Identifier for a schema attached to packets. Schemas validate payload structure \
             before emission. Register schemas in the ScenarioSpec's schemas array. Packets \
             reference schemas by schema_id for type safety and documentation.",
        ),
        // =====================================================================
        // RUN STATE & TIMING
        // =====================================================================
        entry(
            "run_config",
            "Configuration provided when starting a run via scenario_start. Includes tenant_id, \
             run_id, scenario_id, dispatch_targets, and policy_tags. Run config is immutable \
             after start and recorded in runpacks for reproducibility.",
        ),
        entry(
            "run_state_store",
            "Backend configuration for persisting run state. Options include in-memory (for \
             testing) or SQLite. The store holds run progress, decision history, and pending \
             triggers. Configure durability and retention based on compliance needs.",
        ),
        entry(
            "started_at",
            "Caller-supplied timestamp marking when the run began. Required at scenario_start and \
             used for timing calculations, stage_entered_at, and audit records. Prefer explicit \
             timestamps for deterministic replay.",
        ),
        entry(
            "stage_entered_at",
            "Timestamp recorded when the run entered the current stage. Used to evaluate stage \
             timeouts and for audit replay. Set at scenario_start and updated on advances.",
        ),
        entry(
            "status",
            "Current state indicator for a run or verification. Run statuses: 'active', \
             'completed', 'failed'. Verification statuses: 'pass', 'fail'. Check status to \
             determine next actions or surface issues.",
        ),
        entry(
            "timeout",
            "Stage timeout configuration. Set to a TimeoutSpec containing timeout_ms and \
             policy_tags, or null for no timeout. Timeouts prevent runs from stalling \
             indefinitely and are handled by on_timeout.",
        ),
        entry(
            "TimeoutPolicy",
            "Policy controlling what happens when a stage times out. Options: 'fail' marks the \
             run failed with a timeout reason; 'advance_with_flag' advances and sets the decision \
             timeout flag; 'alternate_branch' routes using unknown outcomes in branch rules. \
             Choose based on workflow criticality and routing needs.",
        ),
        entry(
            "on_timeout",
            "Stage timeout policy (TimeoutPolicy). Always present in StageSpec and used only when \
             timeout is set; ignored when timeout is null. Supports 'fail', 'advance_with_flag', \
             or 'alternate_branch' behaviors.",
        ),
        entry(
            "trigger",
            "Event payload submitted via scenario_trigger to advance a run. Contains trigger_id, \
             run_id, kind, time, source_id, optional payload, and correlation_id. Triggers are \
             recorded for audit replay.",
        ),
        // =====================================================================
        // DISCLOSURE & DISPATCH
        // =====================================================================
        entry(
            "policy_tags",
            "Labels applied to runs, predicates, or disclosures for policy routing. Tags enable \
             conditional behavior: different disclosure rules per environment, tenant-specific \
             rate limits, or audit categories. Define tags in the ScenarioSpec and apply them to \
             runs, predicates, packets, or timeouts as needed.",
        ),
        entry(
            "visibility_labels",
            "Access control labels attached to emitted packets. Labels are available to policy \
             deciders and downstream consumers to implement disclosure rules. Use for graduated \
             disclosure: some systems see summaries, others see full details.",
        ),
        entry(
            "dispatch_targets",
            "Destinations where emitted packets are delivered. Configure targets in run_config: \
             agent, session, external, or channel. Multiple targets enable fan-out to different \
             systems.",
        ),
        entry(
            "payload",
            "The content body of a packet, submission, or trigger payload. Encoded as \
             PacketPayload: json, bytes, or external content_ref (uri + content_hash, optional \
             encryption). Payloads are hashed for integrity and may be schema-validated before \
             emission.",
        ),
        entry(
            "decision",
            "The recorded outcome of a trigger evaluation. Contains decision_id, seq, trigger_id, \
             stage_id, decided_at, and a DecisionOutcome (start/advance/hold/fail/complete). \
             Advance outcomes include a timeout flag when triggered by timeouts. Decisions form \
             the audit trail for run progression.",
        ),
        entry(
            "record",
            "Wrapper for submission responses. Contains submission_id, run_id, payload, \
             content_type, content_hash, submitted_at, and correlation_id. Records prove \
             artifacts were submitted and hashed for audit.",
        ),
        entry(
            "request",
            "Wrapper for tool request payloads. Used in MCP protocol messages. Contains the tool \
             name, parameters, and request metadata. Internal structure; most users interact via \
             higher-level scenario_* tools.",
        ),
        // =====================================================================
        // PROVIDER CONFIGURATION
        // =====================================================================
        entry(
            "transport",
            "MCP transport protocol for provider communication: 'stdio' for subprocess pipes, \
             'http' for JSON-RPC over HTTP, 'sse' for server-sent events. Choose based on \
             deployment: stdio for local providers, http/sse for remote services. Configure in \
             decision-gate.toml [[providers]].",
        ),
        entry(
            "bind",
            "Address the MCP server binds to for HTTP or SSE transports. Format: 'host:port' \
             (e.g., '127.0.0.1:8080', '0.0.0.0:9000'). Required when the server transport is \
             http/sse. Omit for stdio.",
        ),
        entry(
            "capabilities_path",
            "Filesystem path to a provider's capability contract JSON. The contract declares \
             supported predicates, param schemas, and comparator compatibility. The runtime \
             validates queries against capabilities before dispatch. Distribute contracts \
             alongside provider binaries.",
        ),
        entry(
            "user_agent",
            "User-Agent header string for HTTP provider requests. Defaults to a Decision Gate \
             identifier. Customize for API requirements, rate-limit identification, or debugging. \
             Appears in external service logs.",
        ),
        entry(
            "allowed_hosts",
            "Hostname allowlist for HTTP provider outbound requests. Only URLs matching these \
             hosts are permitted. Prevents SSRF: queries to unapproved hosts fail with a security \
             error. Required for http providers in production.",
        ),
        // =====================================================================
        // SECURITY & TRUST
        // =====================================================================
        entry(
            "default_policy",
            "Default trust policy for evidence providers. Options: 'audit' or 'require_signature' \
             (with key list). Individual providers can override. Start with 'audit' and tighten \
             per-provider as needed.",
        ),
        entry(
            "allow_raw_values",
            "Global setting permitting raw evidence values in results. When false, providers \
             return hashes instead of values. Enable only when consuming systems need actual \
             evidence content. Combine with require_provider_opt_in for layered control.",
        ),
        entry(
            "require_provider_opt_in",
            "Requires providers to explicitly opt into raw disclosure via allow_raw in their \
             config. Even if allow_raw_values is true globally, providers without opt-in return \
             hashes. Defense-in-depth for sensitive evidence.",
        ),
        entry(
            "allow_raw",
            "Per-provider setting allowing raw evidence disclosure. Requires allow_raw_values to \
             be true globally. Set on providers whose evidence is safe to expose (e.g., \
             timestamps, health status). Omit for sensitive providers (secrets, credentials).",
        ),
        entry(
            "allow_insecure_http",
            "Permits http:// (non-TLS) URLs for MCP providers globally. Disable in production. \
             Use only for local development or air-gapped networks. HTTP traffic is unencrypted \
             and vulnerable to interception.",
        ),
        entry(
            "allow_http",
            "Per-provider setting permitting http:// URLs for the HTTP evidence provider. \
             Defaults to false (HTTPS only). Enable for internal health endpoints that lack TLS. \
             Prefer HTTPS for any network-accessible services.",
        ),
        entry(
            "allow_logical",
            "Permits logical timestamps in time predicates instead of requiring real unix_millis. \
             Enable for testing and simulation where deterministic time control is needed. \
             Disable in production for real-time constraints.",
        ),
        entry(
            "allow_yaml",
            "Permits YAML parsing in the JSON evidence provider. YAML is a superset of JSON with \
             additional syntax. Enable when config files use YAML format. Disable to restrict to \
             pure JSON for stricter validation.",
        ),
        // =====================================================================
        // PROVIDER-SPECIFIC: ENV
        // =====================================================================
        entry(
            "allowlist",
            "List of environment variable keys the env provider may read. Queries for keys not in \
             the allowlist fail with a policy error. Use allowlists to limit exposure: only \
             permit the specific variables your predicates need.",
        ),
        entry(
            "denylist",
            "List of environment variable keys the env provider must never read. Queries for \
             denied keys fail immediately. Use denylists for defense-in-depth: block \
             known-sensitive keys (API_KEY, SECRET_*, etc.) even if accidentally queried.",
        ),
        entry(
            "max_key_bytes",
            "Maximum byte length for environment variable keys queried by the env provider. \
             Prevents resource exhaustion from pathological key names. Defaults to a reasonable \
             limit. Queries exceeding this fail with a validation error.",
        ),
        entry(
            "max_value_bytes",
            "Maximum byte length for environment variable values returned by the env provider. \
             Prevents oversized values from bloating evidence results. Values exceeding this are \
             truncated or rejected per provider config.",
        ),
        entry(
            "overrides",
            "Deterministic override map for environment values during testing. Keys in overrides \
             replace real environment lookups, ensuring reproducible evidence. Use for CI/CD \
             where environment varies across runners.",
        ),
        // =====================================================================
        // PROVIDER-SPECIFIC: JSON
        // =====================================================================
        entry(
            "root",
            "Base directory for JSON provider file resolution. File paths in queries are resolved \
             relative to root. Prevents directory traversal: paths escaping root fail with a \
             security error. Set to the config directory or a dedicated data folder.",
        ),
        entry(
            "max_bytes",
            "Maximum file size in bytes the JSON provider will read. Prevents resource exhaustion \
             from oversized files. Queries for files exceeding this limit fail with a validation \
             error. Size appropriately for your config files.",
        ),
        entry(
            "jsonpath",
            "JSONPath selector used by the JSON provider to extract values from documents. Syntax \
             follows RFC 9535. Examples: '$.version', '$.config.features[*].name'. The extracted \
             value becomes the evidence for comparator evaluation.",
        ),
        // =====================================================================
        // PROVIDER-SPECIFIC: HTTP
        // =====================================================================
        entry(
            "max_body_bytes",
            "Maximum request body size in bytes for JSON-RPC requests. Prevents oversized \
             payloads from exhausting server resources. Requests exceeding this are rejected \
             before processing. Configure based on expected payload sizes.",
        ),
        entry(
            "max_response_bytes",
            "Maximum HTTP response body size the HTTP provider will read. Prevents memory \
             exhaustion from unbounded responses. Responses exceeding this are truncated or \
             rejected. Size for typical health check or API responses.",
        ),
        entry(
            "timeout_ms",
            "Timeout in milliseconds. Used for stage timeouts (TimeoutSpec) and for HTTP provider \
             requests. Operations exceeding this duration fail per policy.",
        ),
        // =====================================================================
        // PROVIDER-SPECIFIC: TIME
        // =====================================================================
        entry(
            "unix_millis",
            "Unix timestamp expressed in milliseconds since epoch (1970-01-01 UTC). Standard \
             format for trigger_time and time predicates. Millisecond precision enables \
             sub-second scheduling. Convert from ISO 8601: parse to Date, call getTime().",
        ),
        // =====================================================================
        // RUNPACK EXPORT
        // =====================================================================
        entry(
            "output_dir",
            "Directory where runpack_export writes the audit bundle. The exporter creates the \
             manifest and artifact files here. Ensure write permissions and sufficient disk \
             space. Existing files may be overwritten.",
        ),
        entry(
            "runpack_dir",
            "Root directory of a runpack for verification. runpack_verify reads the manifest from \
             this location and checks all referenced artifacts. Point to the directory containing \
             manifest.json.",
        ),
        entry(
            "generated_at",
            "Timestamp recorded in the runpack manifest indicating when the export was created. \
             Expressed as ISO 8601. Useful for audit logs and freshness checks. Does not affect \
             verification outcome.",
        ),
        entry(
            "hash_algorithm",
            "Algorithm used for hashing evidence and runpack artifacts. Currently SHA-256 \
             exclusively. Recorded in manifests for forward compatibility. Do not assume other \
             algorithms without checking this field.",
        ),
        entry(
            "include_verification",
            "Flag indicating whether to emit a verification report alongside the runpack export. \
             When true, runpack_export also runs verification and includes the report. Useful for \
             immediate validation after export.",
        ),
        entry(
            "issue_entry_packets",
            "Flag controlling whether scenario_start emits entry_packets for the initial stage. \
             Set false to defer packet emission until the first trigger or next. Useful when runs \
             need setup before disclosures begin.",
        ),
    ];

    entries.sort_by(|a, b| a.term.cmp(&b.term));

    TooltipsManifest {
        version: TOOLTIP_VERSION.to_string(),
        entries,
    }
}

/// Builds a glossary markdown document from the tooltip manifest.
#[must_use]
pub fn tooltips_glossary_markdown() -> String {
    let manifest = tooltips_manifest();
    let mut out = String::new();
    out.push_str("# Decision Gate Glossary\n\n");
    out.push_str("Short, canonical definitions for Decision Gate terms.\n\n");
    for entry in manifest.entries {
        out.push_str("## `");
        out.push_str(&entry.term);
        out.push_str("`\n\n");
        out.push_str(&entry.description);
        out.push_str("\n\n");
    }
    out
}
