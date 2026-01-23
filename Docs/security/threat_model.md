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
and stage advancement. It evaluates evidence-backed predicates, emits auditable
decisions, and supports offline verification via runpacks. It does not run
agent conversations.

## Security Goals
- Deterministic evaluation with no hidden mutation of state.
- Evidence-backed disclosure only; fail closed on missing, invalid, or
  unverifiable evidence.
- Auditability and tamper detection for run state and runpacks.
- Minimized data exposure; default to safe summaries and redacted evidence.
- Clear trust boundaries between control plane, providers, dispatch targets,
  and storage.

## Non-Goals / Out of Scope
- Protecting confidentiality after data is disclosed to downstream systems.
- Protecting against full host or kernel compromise without external controls.
- Securing external MCP providers, downstream sinks, or client applications.
- Hardware attestation, secure enclave guarantees, or key custody services.

## Assets
- Scenario specifications, predicates, and policy tags (security logic).
- Run state logs: triggers, gate evaluations, decisions, packets, submissions,
  tool calls.
- Evidence values, hashes, anchors, and signatures.
- Dispatch payloads, envelopes, and receipts.
- Runpack artifacts and manifest.
- Provider capability contracts and schemas.
- Configuration files, provider auth tokens, and signature verification keys.
- Run state store (SQLite) and runpack output directory.

## Adversary Model
- Nation-state adversaries with full knowledge of Decision Gate behavior.
- Untrusted or compromised clients emitting triggers or tool calls.
- Malicious or faulty evidence providers and external MCP servers.
- Compromised insiders with access to configuration, storage, or logs.
- Network attackers able to MITM, replay, or drop traffic.
- Malicious or mistaken scenario authors who can define unsafe specs.

## Trust Boundaries
- MCP server transports (stdio, HTTP, SSE): all JSON-RPC inputs are untrusted.
- Scenario definition input: specs can encode disclosure logic and data access.
- Evidence provider boundary: built-in providers vs external MCP providers.
- Provider capability contracts and configuration files on disk.
- Run state store and runpack artifacts: treat storage as untrusted.
- Broker sources (http/file/inline) and sinks (external systems).
- Dispatch targets and downstream systems receiving disclosures.
- Offline verification environment and artifact readers.

## Entry Points and Attack Surfaces
- MCP tools: `scenario_define`, `scenario_start`, `scenario_status`,
  `scenario_next`, `scenario_submit`, `scenario_trigger`, `evidence_query`,
  `runpack_export`, `runpack_verify`.
- CLI commands: `serve`, `runpack export`, `runpack verify`, authoring
  validate/normalize.
- External MCP provider processes and HTTP endpoints.
- Built-in providers: `env`, `json`, `http`, `time` (filesystem, environment,
  network).
- External content references for packet payloads (`http://`, `https://`,
  `file://`, `inline:`).
- Config paths, capability contracts, provider commands/URLs.

## Security Controls and Invariants
- Canonical JSON hashing (RFC 8785) for specs, logs, runpack artifacts, and
  tool calls.
- Tri-state evaluation with `Unknown` treated as non-passing.
- Evidence hash normalization; optional signature verification (ed25519) when
  configured.
- Capability registry validates predicate params and allowed comparators.
- Size and path limits for config files, provider contracts, runpack artifacts.
- HTTP/SSE request body limits; provider-specific response size limits.
- MCP tool calls require explicit authn/authz (local-only by default; bearer or
  mTLS subject allowlists when configured) with audit logging.
- Safe summaries for client-facing status; evidence redaction by policy.
- Append-only run state logs and deterministic replay semantics.

## Threats and Mitigations

### Input Validation and Parsing
- JSON-RPC, config files, and JSONPath are untrusted; schema validation and size
  limits apply.
- Stdio transports have no inherent body limit; deployment must bound or
  sandbox inputs.

### Evidence Integrity and Authenticity
- Built-in providers are trusted only for deterministic behavior; external
  providers are untrusted.
- Signature enforcement is optional and configured per provider; default is
  audit-only.
- Evidence hashes are canonicalized; signatures are verified against the hash
  when required.

### Disclosure and Policy Enforcement
- Disclosure is controlled by scenario specs and optional policy deciders.
- Default disclosure policy is permit-all; deployments must enforce disclosure
  authorization with policy adapters.
- MCP tool calls enforce authn/authz before request handling.
- Evidence query results are redacted unless raw disclosure is explicitly
  allowed.

### Storage and Runpack Integrity
- SQLite store verifies hash consistency but does not provide tamper-proof
  authenticity.
- Runpack manifests are hash-indexed for internal consistency but are not
  signed.
- External signing or WORM storage is required for strong non-repudiation.

### Confidentiality and Data Exposure
- Evidence values and payloads may include secrets; use allowlists and
  redaction.
- `env` and `json` providers can expose sensitive data if not restricted.
- External payload sources can exfiltrate data if URIs are attacker-controlled.

### Availability and Resource Exhaustion
- Large requests, responses, or inline payloads can exhaust memory.
- HTTP providers have timeouts; external providers must enforce limits too.
- Provider errors resolve to `Unknown`, causing holds rather than disclosure.

### Supply Chain and Provider Compromise
- External providers run with local process privileges; treat as compromised.
- Capability contracts loaded from disk can be tampered with and must be
  protected.

### Multi-Tenant and Isolation
- Tenant and run identifiers are data labels, not access controls.
- Any shared runtime must enforce authn/authz and rate limiting upstream.

## Operational Requirements
- Restrict MCP access to authenticated transports (mTLS, IPC ACLs, reverse
  proxy auth).
- Configure `server.auth` for non-loopback deployments; rotate tokens and
  maintain tool allowlists.
- Require signature verification for external providers where integrity
  matters.
- Configure allowlists for `env`, `json`, and `http` providers; avoid
  unrestricted file access.
- Limit or disable `runpack_export` for untrusted callers; restrict output
  paths.
- Store run state and runpacks in tamper-evident storage; sign manifests
  externally.
- Apply OS-level sandboxing for external providers and broker sources.

## Failure Posture
- Fail closed on missing, invalid, or unverifiable evidence.
- Do not disclose data on `Unknown` or ambiguous outcomes.

## Threat Model Delta
- Added MCP tool call authn/authz with local-only defaults, bearer/mTLS modes,
  and audit logging.
