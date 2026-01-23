// decision-gate-contract/src/tooltips.rs
// ============================================================================
// Module: Tooltip Catalog
// Description: Canonical tooltip strings for Decision Gate docs and UI.
// Purpose: Provide a stable key-value catalog for documentation rendering.
// Dependencies: std::collections
// ============================================================================

//! ## Overview
//! Tooltips provide short, reusable explanations for UI and documentation
//! surfaces. Keys are stable, and values are plain ASCII strings to enable
//! downstream localization pipelines.
//! Security posture: tooltips are static content; see
//! `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::collections::BTreeMap;

// ============================================================================
// SECTION: Tooltip Catalog
// ============================================================================

/// Stable tooltip catalog type.
pub type TooltipCatalog = BTreeMap<String, String>;

/// Returns the canonical tooltip catalog.
#[allow(clippy::too_many_lines, reason = "flat tooltip catalog is naturally long")]
#[must_use]
pub fn tooltips() -> TooltipCatalog {
    let mut catalog = BTreeMap::new();
    catalog.insert(
        String::from("scenario.spec"),
        String::from("Canonical scenario specification for Decision Gate."),
    );
    catalog.insert(
        String::from("scenario.spec.stages"),
        String::from("Ordered stages defining gate evaluation flow."),
    );
    catalog.insert(
        String::from("scenario.spec.predicates"),
        String::from("Predicate definitions that bind providers to comparators."),
    );
    catalog.insert(
        String::from("scenario.spec.requirement"),
        String::from("RET requirement tree composed from predicate keys."),
    );
    catalog.insert(
        String::from("scenario.spec.advance_to"),
        String::from("Stage advancement policy: linear, fixed, branch, or terminal."),
    );
    catalog.insert(
        String::from("evidence.query.provider_id"),
        String::from("Provider identifier registered in the MCP config."),
    );
    catalog.insert(
        String::from("evidence.query.predicate"),
        String::from("Provider-specific predicate name."),
    );
    catalog.insert(
        String::from("evidence.query.params"),
        String::from("Provider-specific parameters for the predicate."),
    );
    catalog.insert(
        String::from("tool.scenario_define"),
        String::from("Register a scenario and compute its canonical hash."),
    );
    catalog.insert(
        String::from("tool.scenario_start"),
        String::from("Start a new scenario run with a RunConfig."),
    );
    catalog.insert(
        String::from("tool.scenario_status"),
        String::from("Fetch current stage, last decision, and safe summary."),
    );
    catalog.insert(
        String::from("tool.scenario_next"),
        String::from("Evaluate gates and advance or hold the run."),
    );
    catalog.insert(
        String::from("tool.scenario_submit"),
        String::from("Submit external artifacts into run state for audit."),
    );
    catalog.insert(
        String::from("tool.scenario_trigger"),
        String::from("Submit a trigger event and evaluate the run."),
    );
    catalog.insert(
        String::from("tool.evidence_query"),
        String::from("Query evidence providers with disclosure policy applied."),
    );
    catalog.insert(
        String::from("tool.runpack_export"),
        String::from("Export deterministic runpack artifacts for offline verification."),
    );
    catalog.insert(
        String::from("tool.runpack_verify"),
        String::from("Verify runpack manifest and artifact hashes."),
    );
    catalog.insert(
        String::from("provider.time"),
        String::from("Deterministic time predicates sourced from trigger timestamps."),
    );
    catalog.insert(
        String::from("provider.env"),
        String::from("Environment variable lookups with allow/deny policy."),
    );
    catalog.insert(
        String::from("provider.json"),
        String::from("JSON/YAML file queries with JSONPath selectors."),
    );
    catalog.insert(
        String::from("provider.http"),
        String::from("HTTP endpoint checks with strict size and host limits."),
    );
    catalog.insert(
        String::from("config.server.transport"),
        String::from("MCP transport: stdio, http, or sse."),
    );
    catalog.insert(
        String::from("config.server.bind"),
        String::from("Bind address required for http/sse transports."),
    );
    catalog.insert(
        String::from("config.server.max_body_bytes"),
        String::from("Maximum JSON-RPC request size in bytes."),
    );
    catalog.insert(
        String::from("config.trust.default_policy"),
        String::from("Default trust policy for evidence providers."),
    );
    catalog.insert(
        String::from("config.evidence.allow_raw_values"),
        String::from("Allow raw evidence values to be returned by evidence_query."),
    );
    catalog.insert(
        String::from("config.evidence.require_provider_opt_in"),
        String::from("Require provider opt-in before returning raw evidence values."),
    );
    catalog.insert(
        String::from("config.providers"),
        String::from("Provider registrations for built-in and external MCP providers."),
    );
    catalog.insert(
        String::from("config.providers.capabilities_path"),
        String::from("Path to the provider capability contract JSON for MCP providers."),
    );
    catalog.insert(
        String::from("provider.predicates.determinism"),
        String::from("Determinism class for predicate outputs (deterministic/time/external)."),
    );
    catalog.insert(
        String::from("provider.predicates.allowed_comparators"),
        String::from("Comparator allow-list enforced during scenario authoring."),
    );
    catalog
}
