<!--
Docs/security/threat_model.md
============================================================================
Document: Decision Gate Threat Model
Description: Zero Trust threat model for the Decision Gate control plane
Purpose: Define assets, boundaries, and adversary assumptions for Decision Gate
Dependencies:
  - Docs/standards/codebase_engineering_standards.md
============================================================================
-->

# Decision Gate Threat Model

## Overview

Decision Gate is a deterministic, replayable control plane for gated disclosure
and stage advancement. It evaluates evidence-backed conditions, emits auditable
decisions, and supports offline verification via runpacks. It does not run
agent conversations.

Decision Gate is composed of:

- Control plane core (scenario engine, evidence evaluation, run state, runpack
  builder/verifier).
- MCP server (JSON-RPC over stdio/HTTP/SSE) and CLI tooling.
- Provider federation (built-in providers plus external MCP providers).
- Dispatch/broker layer (payload resolution and delivery).
- Storage layers (SQLite or in-memory run state + schema registry, optional
  object storage for runpacks).

## Related Documentation

- Repository overview and security posture: `README.md` and `SECURITY.md`.
- Operational guidance and controls: `Docs/guides/security_guide.md`.
- Standards and investigation workflow: `Docs/standards/codebase_engineering_standards.md`,
  `Docs/standards/agent_investigation_guide.md`.
- Architecture references: `Docs/architecture/decision_gate_auth_disclosure_architecture.md`,
  `Docs/architecture/decision_gate_evidence_trust_anchor_architecture.md`,
  `Docs/architecture/decision_gate_runpack_architecture.md`,
  `Docs/architecture/decision_gate_namespace_registry_rbac_architecture.md`,
  `Docs/architecture/decision_gate_provider_capability_architecture.md`.
- Component READMEs: `decision-gate-core/README.md`, `decision-gate-mcp/README.md`,
  `decision-gate-broker/README.md`, `decision-gate-providers/README.md`.

## Security Goals

- Deterministic evaluation with no hidden mutation of state.
- Evidence-backed disclosure only; fail closed on missing, invalid, or
  unverifiable evidence.
- Auditability and tamper detection for run state and runpacks.
- Minimized data exposure; default to safe summaries and redacted evidence.
- Clear trust boundaries between control plane, providers, dispatch targets,
  and storage.
- Least-privilege tool access and registry operations with explicit authz.
- Bounded resource usage (request size, provider response size, rate limits).

## Non-Goals / Out of Scope

- Protecting confidentiality after data is disclosed to downstream systems.
- Protecting against full host or kernel compromise without external controls.
- Securing external MCP providers, downstream sinks, or client applications.
- Hardware attestation, secure enclave guarantees, or key custody services.
- Cryptographic signing of runpacks or schema records (beyond metadata fields).
- TLS termination and proxy trust (deployment responsibility).

## Assets

- Scenario specifications, conditions, and policy tags (security logic).
- Run state logs: triggers, gate evaluations, decisions, packets, submissions,
  tool calls.
- Evidence values, hashes, anchors, and signatures.
- Namespace authority configuration and namespace mappings.
- Data shape registry records (JSON Schemas), versions, and optional signing
  metadata.
- Dispatch payloads, envelopes, and receipts.
- Runpack artifacts, manifests, and verification reports.
- Provider contracts (capability contracts) and schemas.
- Audit logs (tool authz, precheck, registry ACL, tenant authz, usage).
- Configuration files, provider auth tokens, registry ACL/principal mappings,
  and signature verification keys.
- Run state store (SQLite or in-memory), schema registry store, and runpack
  output directory.
- Object storage buckets for runpack artifacts and archives (S3-compatible).
- Docs catalog content and any extra docs ingested from disk.

## Adversary Model

- Nation-state adversaries with full knowledge of Decision Gate behavior.
- Untrusted or compromised clients emitting triggers or tool calls.
- Malicious or faulty evidence providers and external MCP servers.
- Compromised insiders with access to configuration, storage, or logs.
- Network attackers able to MITM, replay, or drop traffic.
- Malicious or mistaken scenario authors who can define unsafe specs.
- Malicious schema registrants or policy administrators who can poison registry
  entries.
- Attackers controlling content references or broker sources (SSRF/exfiltration
  risk).
- Attackers who can tamper with on-disk provider contracts, configs, or runpack
  artifacts.

## Trust Boundaries

- MCP server transports (stdio, HTTP, SSE): all JSON-RPC inputs are untrusted.
- Scenario definition input: specs can encode disclosure logic and data access.
- Evidence provider boundary: built-in providers vs external MCP providers.
- Namespace authority backend (Asset Core or registry): namespace validation is
  external and must fail closed.
- Provider contracts and configuration files on disk.
- Schema registry backend (in-memory/SQLite) and registry ACL decisions.
- Run state store and runpack artifacts: treat storage as untrusted.
- Runpack object storage (S3-compatible) and metadata: treat as untrusted and
  verify hashes for every artifact.
- Broker sources (http/file/inline) and sinks (external systems).
- Dispatch targets and downstream systems receiving disclosures.
- Offline verification environment and artifact readers.
- Tenant authorization adapters and usage meters (if configured) are external
  decision points.
- Docs extra_paths ingestion (local disk) and MCP resources/list/read.

## Entry Points and Attack Surfaces

- MCP JSON-RPC methods: `tools/list`, `tools/call`, `resources/list`,
  `resources/read`.
- MCP tools: `scenario_define`, `scenario_start`, `scenario_status`,
  `scenario_next`, `scenario_submit`, `scenario_trigger`, `evidence_query`,
  `runpack_export`, `runpack_verify`, `providers_list`,
  `provider_contract_get`, `provider_check_schema_get`, `schemas_list`,
  `schemas_register`, `schemas_get`, `scenarios_list`, `precheck`,
  `decision_gate_docs_search`.
- CLI commands: `serve`, `runpack export`, `runpack verify`, authoring
  validate/normalize.
- Config file and environment variable `DECISION_GATE_CONFIG`.
- External MCP provider processes and HTTP endpoints.
- Built-in providers: `env`, `json`, `http`, `time` (filesystem, environment,
  network).
- External content references for packet payloads (`http://`, `https://`,
  `file://`, `inline:`).
- Config paths, provider contracts, provider commands/URLs, docs extra paths.
- Runpack storage destinations (local output dir or object storage).

## Security Controls and Invariants

- Canonical JSON hashing (RFC 8785) with non-finite float rejection for specs,
  logs, runpacks, and hashes.
- Tri-state evaluation with `Unknown` treated as non-passing.
- Evidence hash normalization; optional signature verification (ed25519) when
  configured.
- Provider contract registry validates provider check params and allowed
  comparators; strict comparator/type validation is default-on.
- Namespace authority checks enforce tenant/namespace scoping and fail closed
  on unknown or unavailable catalogs.
- Evidence trust lanes enforced (verified by default); dev-permissive explicitly
  lowers trust to asserted for non-exempt providers only.
- Schema registry ACL enforces role/policy-class access and can require signing
  metadata; registry operations are audited.
- Anchor policy enforcement rejects evidence missing required anchors and
  propagates anchor requirements into runpack verification.
- Size and path limits for config files, provider contracts, run state stores,
  docs ingestion, runpack artifacts, and object-store keys.
- RET logic hard limits: DSL inputs capped at 1 MiB with nesting depth 32;
  serialized requirement inputs capped at 1 MiB with default max depth 32;
  plan execution stack depth capped at 64 frames; constant pools capped at
  65,536 entries.
- HTTP/SSE and stdio request body limits; provider-specific response size
  limits and timeouts.
- Inflight request caps and optional rate limiting for MCP tool calls.
- MCP tool calls require explicit authn/authz (local-only by default; bearer or
  mTLS subject allowlists when configured) with audit logging.
- Tool visibility filters list/call surfaces; docs search/resources can be
  disabled.
- Tenant authorization hook (if configured) gates tool calls and is audited.
- Precheck is read-only: asserted evidence validated against schemas, no run
  state mutation or disclosures.
- Safe summaries for client-facing status; evidence redaction by policy.
- SQLite run state uses canonical JSON + hash verification on load; runpack
  manifests use file hashes + root hash for integrity.

## Threats and Mitigations

### Authentication, Authorization, and Access Control

- Unauthorized tool access: local-only defaults, bearer/mTLS modes, per-tool
  allowlists, tool visibility filters, and audit logging.
- Tenant/namespace abuse: namespace authority checks, default namespace
  deny-by-default, tenant authz hooks, and registry ACLs.
- Registry poisoning/leakage: ACL rules and optional signing metadata
  requirements with audit trails.

### Input Validation and Parsing

- Untrusted JSON-RPC/config inputs: strict typed decoding, comparator
  validation, canonical JSON normalization, and size/path limits.
- JSONPath/YAML parsing in `json` provider: path traversal checks, size limits,
  and structured error handling.
- Provider contract tampering: contract path validation and canonical hashing
  of contract payloads.

### Evidence Integrity and Authenticity

- Malicious or faulty providers: trust lanes, optional signature verification,
  anchor policy enforcement, and canonical evidence hashing.
- External MCP providers: response size limits, timeouts, and correlation ID
  sanitization; treat as untrusted processes or remote services.

### Disclosure and Data Exposure

- Evidence leakage through tools: evidence redaction policies for
  `evidence_query` and `scenario_next` feedback; safe summaries by default.
- Policy bypass in dispatch: optional policy engine (`permit_all`, `deny_all`,
  or static rules) gates disclosure before dispatch.

### Storage and Runpack Integrity

- Run state tampering: SQLite store verifies canonical hash on load and fails
  closed; run state versions are append-only (with optional retention pruning).
- Runpack tampering: verifier checks artifact hashes, root hash, and anchor
  policy; no built-in signing (external signing/WORM required for
  non-repudiation).

### External Providers, Sources, and Dispatch

- Broker sources (http/file/inline) used for payload resolution: content hash
  verification, content type checks, size limits, no redirects (HTTP), and
  optional root path enforcement (file).
- Built-in providers: allowlists/denylists and size limits for `env`, root
  restrictions and size limits for `json`, and host allowlists + https-only
  defaults for `http`.

### Availability and Resource Exhaustion

- Large requests/responses: `max_body_bytes`, provider response caps, schema
  size limits, runpack artifact limits, and optional rate limiting/inflight
  limits.
- Provider timeouts: HTTP provider timeouts and MCP provider response caps.

### Supply Chain and Execution Environment

- External providers execute with local privileges; use OS sandboxing, scoped
  credentials, and minimal permissions.
- Provider contracts, configs, and docs extra_paths are local file inputs and
  must be protected by file system ACLs and integrity controls.

### Multi-Tenant and Isolation

- Tenant/namespace IDs are labels, not access controls: enforce authn/authz,
  tenant authz hooks, and registry ACLs in shared deployments.

### Auditability and Observability

- Auth decisions, registry access, tenant authz, and usage are logged with
  structured audit events; precheck logs are hash-only by default.

## Implementation References (Controls and Protections)

### Core Runtime

- Canonical JSON hashing and non-finite float rejection:
  `decision-gate-core/src/core/hashing.rs`.
- Tri-state comparator evaluation:
  `decision-gate-core/src/runtime/comparator.rs`.
- Trust lane enforcement and anchor policy validation:
  `decision-gate-core/src/runtime/engine.rs`,
  `decision-gate-core/src/core/evidence.rs`.
- Safe summaries:
  `decision-gate-core/src/core/summary.rs`.
- Runpack build/verify and artifact size limits:
  `decision-gate-core/src/runtime/runpack.rs`.

### MCP Server and Tooling

- Authn/authz, tool allowlists, bearer parsing, and auth audit:
  `decision-gate-mcp/src/auth.rs`, `decision-gate-config/src/config.rs`.
- Request limits (max body, inflight, rate limiting) and transport handling:
  `decision-gate-mcp/src/server.rs`, `decision-gate-config/src/config.rs`.
- Correlation ID sanitization:
  `decision-gate-mcp/src/correlation.rs`.
- Tool visibility, docs gating, evidence redaction, and precheck handling:
  `decision-gate-mcp/src/tools.rs`.
- Audit event payloads:
  `decision-gate-mcp/src/audit.rs`.
- Tenant authz and usage meter seams:
  `decision-gate-mcp/src/tenant_authz.rs`, `decision-gate-mcp/src/usage.rs`.
- Provider contract validation + strict comparator validation:
  `decision-gate-mcp/src/capabilities.rs`, `decision-gate-mcp/src/validation.rs`.
- Evidence signature verification and MCP provider response caps:
  `decision-gate-mcp/src/evidence.rs`.

### Providers and Broker

- Built-in provider limits and policies:
  `decision-gate-providers/src/env.rs`, `decision-gate-providers/src/json.rs`,
  `decision-gate-providers/src/http.rs`, `decision-gate-providers/src/time.rs`.
- Provider allow/deny policy:
  `decision-gate-providers/src/registry.rs`.
- Broker payload validation and source restrictions:
  `decision-gate-broker/src/broker.rs`, `decision-gate-broker/src/source/file.rs`,
  `decision-gate-broker/src/source/http.rs`, `decision-gate-broker/src/source/inline.rs`.

### Storage and Contracts

- SQLite run state + schema registry integrity and size limits:
  `decision-gate-store-sqlite/src/store.rs`.
- In-memory stores (tests/demos only):
  `decision-gate-core/src/runtime/store.rs`.
- Object-store runpack export key validation:
  `decision-gate-mcp/src/runpack_object_store.rs`.
- Config file size/path validation and defaults:
  `decision-gate-config/src/config.rs`.
- Canonical tool and schema contracts:
  `decision-gate-contract/src/tooling.rs`, `decision-gate-contract/src/schemas.rs`.
- CLI authoring/runpack tooling:
  `decision-gate-cli/src/main.rs`.

## Operational Requirements

- Restrict MCP access to authenticated transports (mTLS, IPC ACLs, reverse
  proxy auth) and enforce TLS for HTTP/SSE (or explicit upstream termination).
- Configure `server.auth` for non-loopback deployments; rotate tokens and
  maintain tool allowlists.
- Keep dev-permissive disabled in production; require verified trust lanes.
- Require signature verification for external providers where integrity
  matters; manage key distribution securely.
- Configure allowlists for `env`, `json`, and `http` providers; avoid
  unrestricted file access.
- Restrict `schemas_register`, `schemas_get`, `schemas_list`, `precheck`, and
  `scenarios_list` to trusted callers (tool allowlists + tenant authz +
  registry ACLs). Restrict provider discovery tools with tool allowlists and
  `provider_discovery` allow/deny lists.
- Configure schema registry ACL rules and signing metadata requirements where
  provenance matters; protect the registry store.
- Limit or disable `runpack_export` for untrusted callers; restrict output
  paths and object-store prefixes.
- Store run state and runpacks in tamper-evident storage; sign manifests
  externally when non-repudiation is required.
- Apply OS-level sandboxing for external providers and broker sources.
- Set `server.max_body_bytes`, `server.limits`, and provider timeouts to
  prevent resource exhaustion.
- Disable docs search/resources in untrusted environments or restrict extra
  paths to read-only locations.

## Failure Posture

- Fail closed on missing, invalid, or unverifiable evidence.
- Reject invalid configs, tool calls, and schema registrations.
- Do not disclose data on `Unknown` or ambiguous outcomes.

## Threat Model Delta (2026-02-01)

- Added explicit coverage for docs search/resources, tool visibility filters,
  correlation ID sanitization, and audit payload redaction.
- Expanded broker/source protections and object-store key validation coverage.
- Mapped security controls to concrete code locations for traceability.
- Added explicit authoring input size/depth limits in contract normalization.
- Added HTTP source host allow/deny policy with private/link-local IP guards.
- Enforced symlink-safe file source opens for rooted file disclosures.
