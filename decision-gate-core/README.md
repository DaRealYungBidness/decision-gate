# Decision Gate Core

## IMPORTANT: Active Design Phase

Decision Gate is in an active design and stabilization phase.

- **Backwards compatibility is not guaranteed.**
- **Core semantics are still stabilizing.**
- **Feedback and usage reports are welcome, but code contributions to the core are closed**
  until a stable phase is explicitly announced.

## Overview

Decision Gate is a backend-agnostic control plane for gated disclosure and stage
advancement. It does **not** run agent conversations; it ingests triggers,
evaluates evidence-backed gates, dispatches controlled disclosures into
whatever agent SDK or workflow the host application uses, and exports
runpacks for offline verification.

## Product State (Current)

Decision Gate core is implemented end-to-end as a backend-agnostic control-plane engine.
It includes canonical schemas, deterministic evaluation, tool-call APIs, and
offline-verifiable runpacks. Production integration (HTTP/MCP services,
persistent storage, and policy engines) is intentionally external to this crate.

## Governance and Standards

This crate follows the same documentation and enforcement posture as the
Decision Gate repository:

- `Docs/standards/codebase_formatting_standards.md`
- `Docs/standards/codebase_engineering_standards.md`
- `Docs/security/threat_model.md`

All configuration is centralized, schema-driven, and must pass a Zero Trust
review. Decision Gate assumes nation-state adversaries by default.

## Relationship to Requirements Crate

Decision Gate uses the vendored `ret-logic/` crate (RET: Requirement Evaluation Tree) as
its universal gate algebra and evaluation kernel. Evidence anchoring, runpack
artifacts, and disclosure policy remain Decision Gate responsibilities.

## Core Capabilities

- Deterministic trigger ingestion, gate evaluation, and decision logging.
- Tool-call surface (`scenario.status/next/submit`) backed by the same engine.
- Runpack generation with RFC 8785 canonical hashing and offline verifier.

## Canonical Source of Truth

All external surfaces (HTTP, MCP, SDKs) must call the same control-plane engine
codepath. No adapter may implement divergent logic. This preserves invariance
and ensures that tool-calls, HTTP requests, and batch triggers yield identical
results for the same inputs.

## Current Implementation Scope

Implemented in `decision-gate-core`:

- Canonical schemas: scenario specs, stages, packets, triggers, decisions, run state.
- Evidence contract: queries, results, anchors, comparators, tri-state outcomes.
- Deterministic engine: idempotent trigger handling, gate evaluation, safe summaries.
- Tool-call API: `scenario.status`, `scenario.next`, `scenario.submit`.
- Runpack builder + offline verifier with RFC 8785 canonical JSON hashing.
- In-memory run state store and in-memory artifact sink for tests/examples.

Not implemented here (explicitly missing today):

- HTTP/MCP services and transport adapters.
- Durable run state storage (database-backed store).
- Durable runpack storage (filesystem/blob store adapters).
- Policy engine integration beyond the `PolicyDecider` trait.
- Schema registries, policy registries, and any adapter-specific implementations.

## Getting Started

- Example runner: `cargo run -p decision-gate-core --example minimal`
- Unit tests: `cargo test -p decision-gate-core`

## Stability Notes

The core is functional and deterministic, but still in a stabilization phase.
Backwards compatibility is not guaranteed until a stable release is announced.
