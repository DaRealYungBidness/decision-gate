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
        entry(
            "scenario_define",
            "Register a ScenarioSpec, validate it, and return the canonical spec hash.",
        ),
        entry(
            "scenario_start",
            "Create a run state for a scenario and optionally issue entry packets.",
        ),
        entry("scenario_status", "Fetch a read-only run snapshot and safe summary."),
        entry(
            "scenario_next",
            "Evaluate gates for an agent-driven step and advance or hold the run.",
        ),
        entry("scenario_submit", "Submit external artifacts for audit and later checks."),
        entry("scenario_trigger", "Submit a trigger event and evaluate the run."),
        entry("evidence_query", "Query evidence providers with disclosure policy applied."),
        entry("runpack_export", "Export deterministic runpack artifacts for offline verification."),
        entry("runpack_verify", "Verify runpack manifest and artifacts offline."),
        entry("scenario_id", "Stable scenario identifier used to register, start, and audit runs."),
        entry("ScenarioSpec", "Scenario specification defining stages, predicates, and policies."),
        entry("Requirement", "RET definition composed of And/Or/Not/RequireGroup/Predicate nodes."),
        entry("run_id", "Run identifier scoped to a scenario and used for state and runpacks."),
        entry("stage_id", "Stage identifier used to evaluate gates and record decisions."),
        entry("trigger_id", "Trigger identifier used for idempotent evaluation."),
        entry("tenant_id", "Tenant identifier for isolation and policy scoping."),
        entry("spec_hash", "Canonical SHA-256 hash of the ScenarioSpec."),
        entry("provider_id", "Evidence provider identifier registered in the MCP config."),
        entry("predicate", "Provider predicate name to evaluate."),
        entry("Predicate", "RET leaf that references a predicate key."),
        entry("PredicateSpec", "Predicate definition inside a ScenarioSpec."),
        entry("GateSpec", "Gate definition referencing a requirement inside a stage."),
        entry("params", "Provider-specific parameters for the predicate."),
        entry("comparator", "Comparison operator applied to the evidence result."),
        entry("expected", "Expected value compared against evidence output."),
        entry("EvidenceQuery", "Evidence query containing provider_id, predicate, and params."),
        entry("EvidenceContext", "Run context passed to evidence providers during queries."),
        entry("EvidenceResult", "Provider response containing evidence value and metadata."),
        entry("EvidenceValue", "Evidence payload (json or bytes) returned by a provider."),
        entry("EvidenceAnchor", "Stable anchor metadata for offline verification."),
        entry("EvidenceRef", "External reference URI for evidence content."),
        entry("content_hash", "Hash metadata for payload content."),
        entry("correlation_id", "Correlation identifier used to link requests and decisions."),
        entry("decision", "Decision record returned by evaluation tools."),
        entry("decision_id", "Decision identifier recorded in run history."),
        entry("gate_id", "Gate identifier used in run logs and audits."),
        entry("generated_at", "Timestamp recorded in runpack manifests."),
        entry("include_verification", "Whether to emit a runpack verification report."),
        entry("issue_entry_packets", "Whether to issue entry packets during scenario start."),
        entry("manifest", "Runpack manifest metadata and hashes."),
        entry("manifest_name", "Override for the runpack manifest file name."),
        entry("manifest_path", "Path to the runpack manifest inside the runpack directory."),
        entry("output_dir", "Output directory for runpack exports."),
        entry("packet_id", "Packet identifier for disclosures."),
        entry("payload", "Payload content provided to tools or packets."),
        entry("payload_ref", "Optional reference to external payload content."),
        entry("record", "Record wrapper for submission responses."),
        entry("request", "Tool request payload wrapper."),
        entry("run_config", "Run configuration for scenario start."),
        entry("runpack_dir", "Runpack root directory for verification."),
        entry("schema_id", "Schema identifier attached to packets."),
        entry("started_at", "Caller-supplied run start timestamp."),
        entry("status", "Status indicator for run or verification results."),
        entry("submission_id", "Submission identifier for external artifacts."),
        entry("trigger", "Trigger payload used to advance a run."),
        entry("requirement", "Requirement Evaluation Tree (RET) that gates must satisfy."),
        entry("RequireGroup", "RET operator requiring at least min of reqs to pass."),
        entry("GateOutcome", "Outcome selector for branching (true, false, unknown)."),
        entry("TimeoutPolicy", "Timeout handling policy for a stage."),
        entry("gates", "Gate list evaluated at a stage."),
        entry("stages", "Ordered stages defining gate evaluation flow."),
        entry("entry_packets", "Disclosures emitted when a stage starts."),
        entry("advance_to", "Stage advancement policy: linear, fixed, branch, or terminal."),
        entry("policy_tags", "Policy labels applied to disclosures or runs."),
        entry("visibility_labels", "Visibility labels attached to emitted packets."),
        entry("dispatch_targets", "Dispatch destinations for emitted packets."),
        entry("evidence_hash", "Hash of the evidence value for audit and integrity checks."),
        entry("evidence_anchor", "Anchor metadata linking evidence to a receipt or source."),
        entry("evidence_ref", "External URI reference for evidence content."),
        entry("signature", "Evidence signature metadata (scheme, key, signature)."),
        entry("content_type", "MIME type for evidence or packet content."),
        entry("trigger_time", "Caller-supplied timestamp used by time predicates."),
        entry("timeout", "Stage timeout duration; null means no timeout."),
        entry("on_timeout", "Timeout policy for a stage (fail or hold)."),
        entry("transport", "MCP transport: stdio, http, or sse."),
        entry("bind", "Bind address required for HTTP/SSE transports."),
        entry("max_body_bytes", "Maximum JSON-RPC request size in bytes."),
        entry("default_policy", "Default trust policy for evidence providers."),
        entry("allow_raw_values", "Allow raw evidence values to be returned."),
        entry("require_provider_opt_in", "Require provider opt-in before returning raw values."),
        entry("run_state_store", "Run state store backend configuration."),
        entry("capabilities_path", "Path to the provider capability contract JSON."),
        entry("allow_insecure_http", "Allow http:// URLs for MCP providers."),
        entry("allow_http", "Allow cleartext http:// URLs for the HTTP provider."),
        entry("allow_logical", "Allow logical trigger timestamps for time predicates."),
        entry("allow_yaml", "Allow YAML parsing in the JSON provider."),
        entry("allow_raw", "Allow raw evidence disclosure for this provider."),
        entry("allowlist", "Allowed keys for environment lookup."),
        entry("denylist", "Blocked keys for environment lookup."),
        entry("allowed_hosts", "Allowed HTTP hostnames for outbound checks."),
        entry("hash_algorithm", "Hash algorithm used for evidence or runpack hashing."),
        entry("jsonpath", "JSONPath selector used to extract values from JSON/YAML."),
        entry("logical", "Logical timestamp value used for deterministic ordering."),
        entry("max_bytes", "Maximum file size in bytes for the JSON provider."),
        entry("max_key_bytes", "Maximum bytes allowed for an environment key."),
        entry("max_response_bytes", "Maximum HTTP response size in bytes."),
        entry("max_value_bytes", "Maximum bytes allowed for an environment value."),
        entry("overrides", "Deterministic override map for environment values."),
        entry("root", "Root directory for JSON/YAML file resolution."),
        entry("timeout_ms", "Timeout in milliseconds for HTTP checks or stores."),
        entry("unix_millis", "Unix timestamp in milliseconds."),
        entry("user_agent", "User agent string used for HTTP provider requests."),
    ];

    entries.sort_by(|a, b| a.term.cmp(&b.term));

    TooltipsManifest {
        version: TOOLTIP_VERSION.to_string(),
        entries,
    }
}
