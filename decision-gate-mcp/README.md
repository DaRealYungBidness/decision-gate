<!--
Decision Gate MCP README
============================================================================
Document: decision-gate-mcp
Description: MCP server and evidence federation for Decision Gate.
Purpose: Expose the Decision Gate control plane over JSON-RPC 2.0.
============================================================================
-->

# decision-gate-mcp

## Overview
`decision-gate-mcp` exposes Decision Gate as an MCP JSON-RPC 2.0 server over
stdio, HTTP, or SSE. Decision Gate is a deterministic checkpoint and
requirement-evaluation system for gating plan progression, and this crate is
its canonical tool surface for scenario operations, provider-backed evidence
queries, schema registry access, and precheck.

This crate is a thin transport and policy layer over the control plane in
`decision-gate-core`. It must never implement divergent behavior.

Decision Gate does not require external MCP providers to be useful. Built-in
providers (time, env, json, http) are available out of the box, and the `json`
provider enables gating any tool that can emit JSON artifacts.

## AssetCore Integration
DG integrates with AssetCore for deterministic evidence and replay without
coupling the codebases. See `Docs/integrations/assetcore/` for the canonical
integration hub and `Docs/architecture/decision_gate_assetcore_integration_contract.md`
for the contract.

## Capabilities
- MCP tools: scenario lifecycle, evidence_query, runpack export/verify.
- Discovery tools: providers_list, schemas_list/get, scenarios_list.
- Schema registry: register/list/get for versioned data shapes.
- Precheck: schema-validated, read-only evaluation of asserted data.
- Security controls: bearer or mTLS auth, tool allowlist, rate limits,
  inflight limits, audit logging.

## Evidence Sourcing Modes
Evidence can come from:
- **Built-in providers** (time, env, json, http).
- **External MCP providers** (stdio or HTTP, configured in `decision-gate.toml`).
- **Asserted evidence** via `precheck` (schema-validated, read-only).

Decision Gate does not execute arbitrary tasks; it evaluates evidence produced
by providers or supplied to precheck. For local workflows, run tools outside DG
and point the `json` provider at the resulting artifacts.

## Current Limits and Gaps
- No explicit dev-permissive toggle; trust is controlled by `trust.min_lane`.
- No registry RBAC/ACL beyond tool allowlist.
- Precheck audit is request-level; hash-only audit policy is not enforced.

## Configuration Highlights
- `server.transport`: `stdio`, `http`, or `sse`.
- `server.auth`: `bearer_token` or `mtls` with tool allowlists.
- `server.limits`: inflight and rate limiting.
- `schema_registry`: memory or sqlite backend with size/entry limits.
- `trust.min_lane`: global trust lane requirement.

## Tool Surface (MCP)
- scenario_define, scenario_start, scenario_status, scenario_next
- scenario_submit, scenario_trigger
- evidence_query
- providers_list, schemas_register, schemas_list, schemas_get, scenarios_list
- precheck
- runpack_export, runpack_verify

## Testing
```bash
cargo test -p decision-gate-mcp
```

## References
- Docs/security/threat_model.md
- Docs/roadmap/trust_lanes_registry_plan.md
- decision-gate-core/README.md
