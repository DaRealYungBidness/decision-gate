<!--
Docs/roadmap/open_items.md
============================================================================
Document: Decision Gate Open Items
Description: Open roadmap items and release readiness gaps.
Purpose: Track remaining work after MCP core implementation.
Dependencies:
  - Docs/roadmap/decision_gate_mcp_roadmap.md
  - Docs/security/threat_model.md
============================================================================
-->

# Decision Gate Open Items

## Overview
This document tracks remaining roadmap items now that the MCP foundation,
provider federation, and CLI scaffolding are complete. The focus is on
release-readiness gaps, invariance alignment, and system-level validation.

## Decision Summary (Current Defaults)
These defaults anchor the roadmap and should be treated as authoritative until
explicitly revised.

1. **Canonical Contract Location**
   - A dedicated crate (`decision-gate-contract`) owns contract generation and
     all derived docs artifacts.
2. **Generated Docs and Artifacts**
   - Contract outputs are committed in this repo under
     `Docs/generated/decision-gate/`.
3. **Authoring Formats**
   - Canonical ScenarioSpec format is JSON.
   - RON is allowed as an authoring input and converted to JSON.
   - YAML is not supported unless explicitly added later.

## 1) Canonical Contract and Generated Docs Bundle
**What**: Establish a canonical contract crate (`decision-gate-contract`) that
drives all projections (tooling docs, schemas, tooltips, examples) and emits a
versioned, hashed bundle in `Docs/generated/decision-gate/`.
**Why**: This is the Doctrine of Invariance for Decision Gate. A single source
of truth makes docs, SDKs, and website content deterministic and auditable.
**How**: Implement a contract generator that pulls from core schema types,
MCP tool definitions, and provider capability metadata (defined as Rust data
structures). Emit `tooling.json`, `tooling.md`, `tooltips.json`, config and
scenario JSON schemas, and example runpacks. Commit generated artifacts in
this repo and sync them via an Asset-Core-Web script named for Decision Gate,
writing to a distinct namespace to avoid collisions.

## 2) System Tests Crate (End-to-End)
**What**: Create `decision-gate-system-tests` to run end-to-end MCP workflows
against a real local server (stdio or loopback HTTP).
**Why**: Only real transport exercises the full tool surface, JSON-RPC framing,
and runpack verification in a way auditors trust.
**How**: Spin up the MCP server with built-in providers and a stub MCP provider
over stdio. Execute scenario define/start/next/submit/trigger paths, verify
runpacks, and include tamper and failure-path cases. Keep fixtures deterministic.

## 3) Authoring Formats (ScenarioSpec and Requirements)
**What**: Define the canonical authoring format as JSON for ScenarioSpec and
requirements. Optionally accept RON as a human-friendly authoring input.
**Why**: JSON is the most tooling-friendly and stable for codegen, schemas, and
audits. YAML adds ambiguity and non-determinism without strong value here.
**How**: Provide a conversion path in the contract crate or CLI that takes RON
and emits canonical JSON. Do not store YAML as a primary format.

## 4) MCP Tool Surface: Docs, Schemas, and Enums
**What**: Produce a complete MCP tool contract (schemas, examples, tooltips,
and documentation) and keep it generated from the canonical contract.
**Why**: MCP tools are the primary integration surface; drift-free tooling docs
are essential for inspection and SDK generation.
**How**: Model tool names as a Rust enum internally (for correctness) while
preserving string names on the wire. Generate tool schemas and tooltips from
the contract crate so the website can render hoverable tooltips without manual
maintenance.

## 5) Provider Capability Metadata and Validation
**What**: Define provider capabilities as Rust data structures (predicate name,
params schema, response schema, determinism class, and anchor expectations).
**Why**: Capability metadata enables strict validation, richer errors, and
automatic documentation. It also supports future SDK generation.
**How**: Extend provider registry validation to verify predicate support and
param schemas. Emit `providers.json` and tooltips from the same metadata.

## 6) Inbound AuthN/AuthZ for MCP Tool Calls
**What**: Add explicit auth interfaces for MCP tool calls (token/mTLS and
per-tool authorization) with audit logging.
**Why**: The current local-only posture is not production-safe, and tool calls
are the highest-risk boundary.
**How**: Introduce a `ToolAuthz` trait in `decision-gate-mcp`, enforce it in
`server.rs`/`tools.rs`, and include a default local-only policy with warnings.

## 7) Durable Run State Store
**What**: Implement a persistent `RunStateStore` backend.
**Why**: In-memory storage is not acceptable for production use or audit-grade
system testing.
**How**: Add a database or log-backed store with deterministic serialization
and typed errors. Keep interfaces compatible with `decision_gate_core`.

## 8) Durable Runpack Storage
**What**: Add production-grade `ArtifactSink` and `ArtifactReader` backends.
**Why**: Runpacks are the audit trail; durable storage is required for real use.
**How**: Implement object store or secured filesystem adapters with strict path
validation and explicit error typing.

## 9) Transport Hardening and Operational Telemetry
**What**: Add rate limiting, structured error responses, TLS/mTLS, and audit logs.
**Why**: This is required for hyperscaler/DoD-grade deployments.
**How**: Harden JSON-RPC handlers and introduce structured audit logging with
redaction policies for evidence output.

## 10) Agent Progress vs Plan State
**What**: Clarify that Decision Gate evaluates evidence and run state, while
agent planning is external. Progress signals should be modeled as evidence or
submissions.
**Why**: This keeps Decision Gate deterministic and avoids embedding agent logic.
**How**: Provide a default pattern: agents emit progress as `scenario_submit`
payloads or evidence predicates. If “plan artifacts” are desired, store them
as explicit packet payloads or submissions, not core run state.

## 11) Policy Engine Integration
**What**: Replace `PermitAll` with real policy adapters.
**Why**: Dispatch authorization is critical to disclosure control.
**How**: Add policy backends and include their schemas in the contract bundle.

## Notes on Structural Readiness
Evidence, storage, and dispatch interfaces already exist in
`decision-gate-core/src/interfaces/mod.rs`, enabling durable backends and
policy enforcement without core rewrites. The missing pieces are the canonical
contract pipeline and inbound MCP tool-call auth.
