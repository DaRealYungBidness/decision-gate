<!--
Docs/architecture/decision_gate_evidence_trust_anchor_architecture.md
============================================================================
Document: Decision Gate Evidence Trust + Anchor Architecture
Description: Current-state reference for evidence trust lanes, signature
             enforcement, anchor policy configuration, and validation paths.
Purpose: Provide an implementation-grade map of how DG enforces evidence trust
         and anchor integrity across the control plane and runpack verifier.
Dependencies:
  - decision-gate-core/src/core/evidence.rs
  - decision-gate-core/src/runtime/engine.rs
  - decision-gate-core/src/runtime/runpack.rs
  - decision-gate-mcp/src/evidence.rs
  - decision-gate-mcp/src/tools.rs
  - decision-gate-config/src/config.rs
============================================================================
Last Updated: 2026-02-03 (UTC)
============================================================================
-->

# Decision Gate Evidence Trust + Anchor Architecture

> **Audience:** Engineers implementing or reviewing evidence trust enforcement,
> signature validation, and anchor policy behavior.

---

## Table of Contents

1. [Executive Overview](#executive-overview)
2. [Evidence Model](#evidence-model)
3. [Trust Lanes and Requirements](#trust-lanes-and-requirements)
4. [Provider Trust Policy (Signatures)](#provider-trust-policy-signatures)
5. [Evidence Disclosure Policy](#evidence-disclosure-policy)
6. [Anchor Policy Configuration](#anchor-policy-configuration)
7. [Anchor Validation in the Control Plane](#anchor-validation-in-the-control-plane)
8. [Runpack Anchor Verification](#runpack-anchor-verification)
9. [Asserted Evidence (Precheck)](#asserted-evidence-precheck)
10. [File-by-File Cross Reference](#file-by-file-cross-reference)

---

## Executive Overview

Decision Gate separates **trust** (verified vs asserted evidence) from
**integrity** (anchors and signatures). Trust lanes control whether evidence is
eligible for gate evaluation, while anchor policies and signatures provide
verifiable links to external systems. Enforcement occurs in two places:

- **Control plane runtime**: applies trust requirements and validates anchors
  when configured.
- **Runpack verifier**: replays anchor policy checks offline.

Evidence disclosure is separately controlled at the tool layer.
[F:decision-gate-core/src/core/evidence.rs L88-L255](decision-gate-core/src/core/evidence.rs#L88-L255)[F:decision-gate-core/src/runtime/engine.rs L505-L552](decision-gate-core/src/runtime/engine.rs#L505-L552)[F:decision-gate-core/src/runtime/runpack.rs L501-L549](decision-gate-core/src/runtime/runpack.rs#L501-L549)[F:decision-gate-mcp/src/tools.rs L858-L885](decision-gate-mcp/src/tools.rs#L858-L885)

---

## Evidence Model

Core evidence types define the canonical payloads and integrity metadata:

- `EvidenceResult` carries a value, trust lane, hash, anchor, and optional
  signature.
- `EvidenceAnchor` contains an anchor type and JSON-encoded anchor value.
- `EvidenceAnchorPolicy` maps providers to required anchor types and fields.

[F:decision-gate-core/src/core/evidence.rs L142-L255](decision-gate-core/src/core/evidence.rs#L142-L255)

---

## Trust Lanes and Requirements

Trust lanes are a two-level lattice:

- `Verified` (default) for provider-sourced evidence.
- `Asserted` for client-supplied evidence.

`TrustRequirement` specifies the minimum acceptable lane and is applied per
condition and gate. If evidence does not meet the requirement, it is converted
into an `Unknown` result with a `trust_lane` error.

[F:decision-gate-core/src/core/evidence.rs L88-L136](decision-gate-core/src/core/evidence.rs#L88-L136)[F:decision-gate-core/src/runtime/engine.rs L505-L552](decision-gate-core/src/runtime/engine.rs#L505-L552)[F:decision-gate-core/src/runtime/engine.rs L1463-L1482](decision-gate-core/src/runtime/engine.rs#L1463-L1482)

Control-plane configuration exposes:
- global `trust_requirement`
- per-provider overrides (via `provider_trust_overrides`)

[F:decision-gate-core/src/runtime/engine.rs L107-L127](decision-gate-core/src/runtime/engine.rs#L107-L127)

---

## Provider Trust Policy (Signatures)

The MCP evidence federation enforces provider trust policy before results reach
control-plane evaluation:

- `TrustPolicy::Audit` accepts unsigned results.
- `TrustPolicy::RequireSignature` requires `EvidenceSignature` and validates
  ed25519 signatures using configured public keys.

Policy evaluation:
- Missing signatures, unsupported schemes, or unauthorized keys are rejected.
- If the evidence hash is missing, it is computed from the canonical payload.

[F:decision-gate-mcp/src/evidence.rs L112-L209](decision-gate-mcp/src/evidence.rs#L112-L209)[F:decision-gate-mcp/src/evidence.rs L639-L727](decision-gate-mcp/src/evidence.rs#L639-L727)

---

## Evidence Disclosure Policy

Evidence disclosure is enforced at `evidence_query` time:

- `evidence.allow_raw_values` controls global raw value disclosure.
- `evidence.require_provider_opt_in` additionally requires provider opt-in.
- Providers opt-in via `ProviderConfig.allow_raw`.

If raw values are not allowed, the tool response redacts `value` and
`content_type`, but retains hashes and anchors.
[F:decision-gate-config/src/config.rs L959-L977](decision-gate-config/src/config.rs#L959-L977)[F:decision-gate-mcp/src/tools.rs L858-L885](decision-gate-mcp/src/tools.rs#L858-L885)[F:decision-gate-mcp/src/evidence.rs L188-L209](decision-gate-mcp/src/evidence.rs#L188-L209)

---

## Anchor Policy Configuration

Anchor policy configuration is expressed in MCP config as
`anchors.providers[{provider_id, anchor_type, required_fields}]`. The config is
validated and converted into the runtime `EvidenceAnchorPolicy` used by the
control plane and runpack verifier. Provider ids must be **unique** and
trimmed to prevent ambiguous anchor enforcement.
[F:decision-gate-config/src/config.rs L1378-L1466](decision-gate-config/src/config.rs#L1378-L1466)

---

## Anchor Validation in the Control Plane

When an anchor requirement is configured for a provider:

- Evidence results must include `evidence_anchor`.
- `anchor_type` must match the requirement.
- `anchor_value` must be canonical JSON object with required scalar fields.
- Gate evaluation evidence records are stored in canonical condition order to
  keep runpack artifacts deterministic across executions.

Invalid anchors result in an `anchor_invalid` provider error and the evidence
result is converted to an empty verified result for evaluation.
[F:decision-gate-core/src/runtime/engine.rs L921-L977](decision-gate-core/src/runtime/engine.rs#L921-L977)[F:decision-gate-core/src/runtime/engine.rs L979-L1012](decision-gate-core/src/runtime/engine.rs#L979-L1012)

---

## Runpack Anchor Verification

Runpack verification replays anchor policy checks offline:

- Scenario spec and gate eval logs are loaded from runpack artifacts.
- Condition-to-provider mapping is derived from the spec.
- Evidence anchors in gate evaluation logs are validated against the policy.

Errors are collected and reported in the verification report.
[F:decision-gate-core/src/runtime/runpack.rs L501-L549](decision-gate-core/src/runtime/runpack.rs#L501-L549)[F:decision-gate-core/src/runtime/runpack.rs L552-L593](decision-gate-core/src/runtime/runpack.rs#L552-L593)

---

## Asserted Evidence (Precheck)

Precheck uses **asserted** evidence without contacting providers:

- Payload values are wrapped as `EvidenceResult` with lane `Asserted`.
- Control plane applies trust requirements per condition/gate, which can force
  asserted evidence to `Unknown` depending on configuration.

[F:decision-gate-mcp/src/tools.rs L1625-L1667](decision-gate-mcp/src/tools.rs#L1625-L1667)[F:decision-gate-core/src/runtime/engine.rs L505-L552](decision-gate-core/src/runtime/engine.rs#L505-L552)

---

## File-by-File Cross Reference

| Area | File | Notes |
| --- | --- | --- |
| Evidence model + trust lanes | `decision-gate-core/src/core/evidence.rs` | Canonical types for trust lanes, anchors, policies. |
| Trust enforcement + anchor validation | `decision-gate-core/src/runtime/engine.rs` | Applies trust requirements and validates anchors. |
| Runpack anchor verification | `decision-gate-core/src/runtime/runpack.rs` | Offline validation against anchor policy. |
| Provider signature policy | `decision-gate-mcp/src/evidence.rs` | TrustPolicy parsing + signature enforcement. |
| Evidence disclosure policy | `decision-gate-mcp/src/tools.rs` | Raw evidence redaction for evidence_query. |
| Config surface | `decision-gate-config/src/config.rs` | evidence.* and anchors.* configuration. |
