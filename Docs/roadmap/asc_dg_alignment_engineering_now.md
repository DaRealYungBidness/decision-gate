<!--
Docs/roadmap/asc_dg_alignment_engineering_now.md
============================================================================
Document: Decision Gate + Asset Core Alignment (Engineering Now)
Description: Near-term engineering tasks for independent DG and optional ASC overlap.
Purpose: Define the immediate technical work needed to align namespace, auth, and evidence anchors.
Dependencies:
  - Docs/guides/assetcore_interop_runbook.md
  - Docs/security/threat_model.md
  - Docs/business/open_core_strategy.md
============================================================================
-->

# Decision Gate + Asset Core Alignment (Engineering Now)

## Overview
This roadmap captures the immediate engineering work needed to keep Decision Gate
independent while enabling world-class overlap with Asset Core. The goal is to
standardize the integration boundaries (namespace, auth, evidence anchors) without
introducing tight code coupling.

## Guiding Principles
- **Independence first**: DG must run without Asset Core; ASC must run without DG.
- **Overlap is optional but coherent**: When integrated, namespaces and evidence
  anchors are deterministic and auditable.
- **Fail closed at trust boundaries**: Namespace and evidence resolution must be
  explicit and validated.
- **No adapter business logic**: Protocol adapters remain thin and stateless.

## Immediate Engineering Tasks

### 1) Integration Contract (Spec-First)
- Define a DG <-> ASC integration contract document with:
  - Namespace scoping rules (explicit `namespace_id`, no defaults, fail closed).
  - Evidence anchoring requirements (`world_seq`, `commit_id`, `namespace_id`).
  - Correlation ID passthrough rules (client + server identifiers).
  - Auth/RBAC mapping expectations (ASC principals -> DG permissions).
- Decide on a canonical list of Asset Core anchor types in DG tooltips and schema
  docs (e.g., `world_seq`, `commit_id`, `namespace_id`).

### 2) Namespace Authority Decision
- Choose the source of truth for DG namespaces in an ASC deployment:
  - Option A: DG reads the ASC namespace catalog (read-only).
  - Option B: DG owns its namespace registry and reconciles with ASC via adapters.
- Document how DG fails closed when namespaces are missing or unauthorized.

### 3) Evidence Provider Integration
- Define the Asset Core evidence provider contract:
  - Read daemon query surface and required inputs.
  - Output anchors and deterministic query expectations.
  - Rate limits, size limits, and error mapping.
- Create a plan for provider-specific tests (anchor correctness and determinism).

### 4) Runpack Anchor Semantics
- Ensure runpack export includes Asset Core anchor metadata for every evidence
  entry that queries ASC.
- Specify how offline verification validates anchor consistency (namespace and
  `world_seq` reconciliation).

### 5) Optional Write-Path Enforcement (Phase 2)
- Draft an opt-in design for write-daemon precheck gating:
  - DG callout during precheck (fail closed, no state mutation).
  - Timeouts, retries, and error mapping.
  - Policy class or config flags to enable per-namespace enforcement.

### 6) Integration Test Plan
- Define minimal tests to validate the overlap contract:
  - Namespace mismatch fails closed.
  - Evidence anchors are present and stable across replays.
  - Correlation IDs preserved across DG -> ASC calls.

## Open Questions (Must Answer Before Implementation)
- Which system is the authoritative namespace registry in integrated deployments?
- What are the canonical anchor types for ASC evidence, and where are they
  documented (contract/tooltips vs runbook)?
- Should DG accept ASC principals directly or rely on an integration layer that
  maps ASC auth to DG permissions?
- What latency budget is acceptable for evidence queries against ASC?
- Do we require a strict "ASC evidence provider" ID or allow multiple providers
  backed by ASC (per cluster/tenant)?

