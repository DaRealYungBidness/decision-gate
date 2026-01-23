# Decision Gate Launch-Grade Semantics Plan

This document defines the pre-launch behavior we want the system to ship with.
It is written so an implementer (human or LLM) can update the codebase directly
and then align the tooltips and docs after the changes are real.

Update note:
- This roadmap is now cross-referenced against the current codebase.
- Tooltips/docs updates are intentionally deferred until remaining runtime features land.

## Goals

- Ship an industry-grade, deterministic system with minimal ambiguity.
- Ensure contracts, runtime behavior, and tooltips describe the same reality.
- Make changes now, before launch, when compatibility concerns are minimal.

## Principles

- Determinism first. No runtime networking for EvidenceRef or payload refs.
- Contracts are authoritative. Tooltips and docs follow the contract and runtime.
- Prefer minimal core semantics; move opinionated or UI-specific behavior out.

## Launch Decisions (Summary)

1) Keep run status enum minimal: active, completed, failed. "Hold" remains a
   decision outcome, not a run status.
2) Enforce idempotent submissions by submission_id (per run).
3) Keep scenario_status minimal; use safe_summary for "hold" reasoning.
4) Implement timeout handling using existing TimeoutSpec/TimeoutPolicy.
5) Keep trigger time as caller-supplied; no monotonic enforcement for logical.
6) Keep EvidenceRef opaque; runtime does not fetch or resolve it.
7) Unify external payload references under PacketPayload/ContentRef.
8) Keep EvidenceContext minimal; no policy_tags added.
9) Update tooltips and docs only after code changes are real.
10) Update AGENTS.md and README.md to remind that tooltip updates are required
    after behavior changes.

## Current Status (Codebase Cross-Check)

Legend: Implemented / Partial / Not Implemented

1) Run lifecycle and status: Implemented (runtime + schema). Tooltips/docs pending.
2) Idempotent submissions: Not implemented.
3) scenario_status minimal contract: Implemented (runtime + schema). Tooltips/docs pending.
4) Timeout handling: Not implemented (TimeoutSpec/Policy exist; runtime does not enforce).
5) Trigger time semantics: Implemented (unix_millis + logical accepted; no monotonic enforcement).
6) EvidenceRef behavior: Implemented (opaque, not resolved by runtime). Tooltips/docs pending.
7) Unify external payload references: Not implemented (TriggerEvent still uses payload_ref).
8) EvidenceContext contents: Implemented (minimal; no policy_tags). Tooltips/docs pending.
9) Tooltip and doc alignment: Deferred until runtime changes are done.
10) AGENTS.md/README.md checklist: Not implemented.

## Detailed Requirements and Implementation Checklist

### 1) Run lifecycle and status

Decision: Keep RunStatus = active/completed/failed. Do not add pending/held.

Requirements:
- "Hold" is represented by DecisionOutcome::Hold + safe_summary.status = "hold".
- scenario_status returns current_stage_id, status, last_decision, issued_packet_ids,
  safe_summary. No gate outcomes or evidence values.

Status (codebase):
- Implemented in runtime and schemas.
- Tooltips/docs still mention pending/held and extra fields.

Implementation tasks:
- No schema changes required.
- Update tooltips to remove "pending/held" and match the schema.

Tests:
- Ensure scenario_status returns safe_summary when last_decision is Hold.

Docs/tooltips:
- Update decision-gate-contract tooltips to match the real contract.

### 2) Idempotent submissions

Decision: scenario_submit must be idempotent by submission_id (within a run).

Requirements:
- If the same submission_id is received with identical payload + content_type,
  return the existing SubmissionRecord (no new append).
- If the same submission_id is received with conflicting payload or content_type,
  return a deterministic error (conflict).

Status (codebase):
- Not implemented. scenario_submit always appends and never conflicts.

Implementation tasks:
- decision-gate-core: check existing submissions before append.
- decision-gate-core: return existing record on exact match; error on conflict.
- decision-gate-mcp: propagate the error cleanly.

Tests:
- New unit test: identical submission_id returns same record.
- New unit test: conflicting submission_id returns error.

Docs/tooltips:
- Update tooltips for scenario_submit and submission_id to call out idempotency.

### 3) scenario_status minimal contract

Decision: Keep scenario_status minimal and deterministic.

Requirements:
- Use safe_summary for UI-facing signal (unmet_gates, retry_hint, policy_tags).
- Do not return evidence values or gate-by-gate evaluations in scenario_status.

Status (codebase):
- Implemented in runtime and schemas.
- Tooltips/docs still claim gate outcomes and timing metadata.

Implementation tasks:
- No schema changes required if behavior already matches.
- Verify safe_summary.status is "hold" and retry_hint is stable.

Tests:
- Validate safe_summary fields for a hold outcome.

Docs/tooltips:
- Update tooltips to remove claims of gate outcomes or evidence values.

### 4) Timeout handling

Decision: Make TimeoutSpec and TimeoutPolicy real (not just in schemas).

Requirements:
- StageSpec.timeout defines timeout_ms relative to stage entry.
- On timeout, apply TimeoutPolicy:
  - fail: DecisionOutcome::Fail with reason "timeout".
  - advance_with_flag: DecisionOutcome::Advance with timeout = true.
  - alternate_branch: advance to the branch target specified by policy.
- Timeouts are evaluated by a trigger event of kind "tick" with caller-supplied time.

Status (codebase):
- Not implemented. TriggerKind::Tick only marks timeout=true on Advance; no timeout evaluation.

Implementation tasks:
- decision-gate-core: record stage entry time.
  - Option A: store stage_entered_at in RunState (preferred).
  - Option B: derive from last decision time + initial started_at.
- decision-gate-core: when TriggerKind::Tick arrives, evaluate timeouts
  before gate evaluation. If expired, apply TimeoutPolicy.
- decision-gate-contract: update schemas if new fields are introduced.
- decision-gate-mcp: ensure tick triggers are accepted and routed correctly.

Tests:
- Timeout fail path (tick past timeout -> run failed).
- Timeout advance_with_flag path (tick past timeout -> advance with timeout flag).
- Timeout alternate_branch path (tick past timeout -> branch to configured stage).
- No timeout if tick before deadline.

Docs/tooltips:
- Update TimeoutSpec, TimeoutPolicy, and on_timeout tooltips to match real behavior.

### 5) Trigger time semantics

Decision: Trigger time is caller-supplied and trusted for determinism.

Requirements:
- trigger.time accepts unix_millis or logical timestamps.
- No monotonic enforcement for logical timestamps (caller responsibility).

Status (codebase):
- Implemented (Timestamp supports unix_millis + logical; no monotonic enforcement).
- Tooltips/docs still claim unix_millis only and monotonic logical timestamps.

Implementation tasks:
- No runtime change unless we decide to enforce monotonic time later.

Docs/tooltips:
- Update trigger_time and logical tooltips to avoid monotonic guarantees.

### 6) EvidenceRef behavior

Decision: EvidenceRef is opaque and not resolved by the runtime.

Requirements:
- Providers may return EvidenceRef; runtime records it.
- Runpack verification is responsible for fetching and verifying referenced content.

Status (codebase):
- Implemented (EvidenceRef is stored; runtime does not resolve refs).
- Tooltips/docs still claim runtime resolves refs.

Implementation tasks:
- No runtime change required; align docs and tooltips.

Docs/tooltips:
- Update EvidenceRef tooltip to remove "runtime resolves refs" language.

### 7) Unify external payload references

Decision: Use PacketPayload (json/bytes/external via ContentRef) everywhere.

Requirements:
- TriggerEvent should use a payload field of PacketPayload, not payload_ref.
- External payloads always represented as ContentRef (uri + content_hash + encryption).
- If we keep payload_ref, document it as opaque and unverified (not preferred).

Status (codebase):
- Not implemented. TriggerEvent still uses payload_ref; PacketPayload/ContentRef exist.

Implementation tasks:
- decision-gate-core: update TriggerEvent struct and handling.
- decision-gate-contract: update trigger_event schema.
- decision-gate-mcp: update tool contract and examples.
- Update examples in Docs/generated and tests.
- Provide a compatibility shim if any internal code assumes payload_ref.

Tests:
- TriggerEvent with external ContentRef payload works end-to-end.
- TriggerEvent with json payload works end-to-end.

Docs/tooltips:
- Update payload/payload_ref tooltips to reflect the new unified model.

### 8) EvidenceContext contents

Decision: Keep EvidenceContext minimal (tenant_id, run_id, scenario_id, stage_id,
trigger_id, trigger_time, correlation_id). Do not add policy_tags.

Requirements:
- Providers should remain policy-agnostic; policy logic stays in the runtime.

Status (codebase):
- Implemented (EvidenceContext is minimal).
- Tooltips/docs still mention policy_tags.

Implementation tasks:
- No schema changes required.
- Update tooltips to remove policy_tags mention.

### 9) Tooltip and doc alignment

Decision: Tooltips are derived from real behavior, not aspirations.

Requirements:
- Update decision-gate-contract tooltips after behavior changes are merged.
- Regenerate Docs/generated artifacts after updating tooltips.

Status (codebase):
- Deferred until remaining runtime features land.

Implementation tasks:
- Add a checklist in decision-gate AGENTS.md and README.md to confirm tooltip
  alignment after any behavior or schema change.

Tests:
- Tooltips generation should be deterministic and ASCII-safe.

## Acceptance Criteria (Launch-Grade)

- All tooltips match schemas and runtime behavior.
- scenario_submit is idempotent by submission_id with conflict detection.
- Timeout handling is fully implemented and tested.
- Trigger payloads use unified PacketPayload/ContentRef.
- Docs generated from contracts are up to date with no hand edits.
