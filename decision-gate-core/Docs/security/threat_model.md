<!--
decision-gate-core/Docs/security/threat_model.md
============================================================================
Document: Decision Gate Threat Model
Description: Zero Trust threat model for the Decision Gate core
Purpose: Define inputs, boundaries, and adversary assumptions for Decision Gate
Dependencies:
  - Docs/standards/codebase_engineering_standards.md
============================================================================
-->

# Decision Gate Threat Model

## Overview
Decision Gate is a control plane for gated disclosure and stage advancement. It assumes
hostile inputs and requires evidence-based decisions that can be verified
offline. Decision Gate does not run agent conversations; it evaluates gates and emits
disclosure decisions.

## Adversary Model
- Nation-state adversaries with full knowledge of Decision Gate behavior.
- Untrusted or compromised clients emitting triggers.
- Malicious or faulty evidence providers.
- Attempts to coerce disclosure without evidence.

## Trust Boundaries
- Trigger ingestion is a boundary; all triggers are untrusted until authenticated.
- Evidence providers are untrusted; evidence must be anchored and hash-verified.
- Anchor policy enforcement is a boundary: missing or malformed anchors fail closed.
- Dispatch targets are untrusted; disclosure decisions must be auditable.
- Artifact sinks/readers are untrusted; runpack outputs must be hash-verified.
- Tool-call APIs are untrusted; inputs must be validated and logged deterministically.

## Failure Posture
- Fail closed on missing, invalid, or unverifiable evidence.
- Do not disclose data on `Unknown` or ambiguous outcomes.

## Threat Model Delta
- Updated to include tool-call, anchor policy, and runpack boundaries.
