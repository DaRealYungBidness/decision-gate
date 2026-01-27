<!--
Docs/guides/evidence_flow_and_execution_model.md
============================================================================
Document: Evidence Flow + Execution Model
Description: Narrative, end-to-end explanation of how Decision Gate handles
             evidence, evaluation, and trust.
Purpose: Provide a clear mental model for where data is authored, fetched, and
         evaluated, and why the system is structured this way.
Dependencies:
  - decision-gate-core/src/runtime/engine.rs
  - decision-gate-mcp/src/tools.rs
  - decision-gate-mcp/src/evidence.rs
  - decision-gate-providers/src/registry.rs
============================================================================
-->

# Evidence Flow + Execution Model

This document is a narrative, high-level view of how Decision Gate works and
why it is structured the way it is. It is intentionally explicit and verbose.

If you only remember one sentence:
**Decision Gate evaluates evidence; it does not execute arbitrary tasks.**

---

## Mental Model (One Page)

Decision Gate solves a single question:
**"Has X been done?"**

To answer that, DG evaluates evidence (data) against predicates and gates. If a
problem can be expressed as data, DG can decide whether the requirement is met.
If it cannot be expressed as data, DG is not the right tool.

There are three ways evidence can be supplied:

1) **Provider-pulled evidence (live runs)**  
   DG calls a provider to fetch evidence. Providers are the data producers.

2) **Asserted evidence (precheck only)**  
   A caller supplies evidence payloads directly to precheck. DG validates the
   payloads against schemas and evaluates gates without mutating run state.

3) **Audit submissions (scenario.submit)**  
   Payloads are recorded for audit and hashing, but do not affect evaluation.

DG always uses the same evaluation model:
**evidence -> comparator -> tri-state -> RET -> gate outcome**.

---

## Key Terms

- **Provider**: An evidence source that can answer evidence queries.
- **EvidenceQuery**: `{ provider_id, predicate, params }`.
- **EvidenceResult**: The provider response (value + optional hash/anchor/signature).
- **Comparator**: Compares evidence to an expected value (equals, greater_than, etc).
- **RET (Requirement Evaluation Tree)**: The logic tree that combines predicates.
- **Trust lane**: Whether evidence is `verified` or `asserted`.

---

## Where Information Is Authored

Evidence is authored in one of two places:

1) **Providers**  
   Providers fetch or compute evidence and return an `EvidenceResult`. Providers
   can be:
   - **Built-in** (compiled into the server): `time`, `env`, `json`, `http`.
   - **External MCP** (stdio or HTTP): any custom integration.

2) **Precheck payloads**  
   A caller supplies evidence directly to `precheck`. DG validates it against a
   registered data shape, then evaluates gates without writing run state.

This separation is intentional: it preserves determinism and auditability while
making it easy to adopt DG in low-friction environments.

---

## Where Evaluation Happens (Always the Same Place)

Evaluation always happens in the control plane (`decision-gate-core`).
Providers never decide gate outcomes. They only return evidence.

Evaluation pipeline:

1) **EvidenceResult** is produced (by provider or precheck).
2) **Comparator** evaluates evidence vs expected.
3) **Tri-state** is produced (`true`, `false`, `unknown`).
4) **RET** combines predicate outcomes into a gate outcome.

This ensures that every transport (MCP, HTTP, SDKs, batch) yields the same
results for the same inputs.

---

## Evidence Sourcing Modes (Detailed)

### 1) Provider-Pulled Evidence (Live Runs)

This is the default mode for real runs. DG calls providers to fetch evidence.

Flow:
1. A run is active.
2. A gate references predicates (evidence queries).
3. DG calls providers for each EvidenceQuery.
4. Providers return EvidenceResult.
5. DG evaluates comparators + RET and updates run state.

This gives strong guarantees when providers are trusted, signed, or anchored.

### 2) Asserted Evidence (Precheck Only)

Precheck is a read-only simulation: "What would happen if this evidence were true?"

Flow:
1. Caller supplies evidence payloads for predicates.
2. DG validates payloads against registered data shapes.
3. DG evaluates comparators + RET.
4. No run state is mutated.

This is useful for:
- trusted agents,
- rapid iteration,
- hypothetical "what if" checks.

### 3) Audit Submissions (scenario.submit)

Submissions store payloads and hashes for audit, but do not drive evaluation.

Flow:
1. Caller submits a payload with `scenario_submit`.
2. DG stores hashes and metadata in run state.
3. Gates are unaffected; this is an audit trail, not evidence input.

---

## Why Built-ins + JSON Go Far

The built-in providers are intentionally narrow and safe:

- **time**: compares against trigger time.
- **env**: reads environment variables.
- **json**: reads JSON/YAML files and evaluates JSONPath.
- **http**: bounded HTTP checks.

The **json** provider is the main bridge for local workflows:
If a tool can emit JSON, DG can gate it.

Examples:
- Lint/format: tool emits JSON -> `json.path` checks `errors == 0`.
- Tests: test runner emits JSON -> check `failed == 0`.
- Coverage: JSON report -> check `coverage >= 80`.
- Scanners: JSON output -> check severity counts.

This avoids arbitrary execution inside DG while still covering most workflows.

---

## Why DG Does Not Execute Arbitrary Tasks

Executing arbitrary tasks inside DG expands the attack surface:
- arbitrary code execution,
- secrets exposure,
- filesystem/network risks,
- unpredictable performance.

DG’s design keeps execution outside the core:
- Run tasks in CI or local workflows.
- Emit JSON artifacts as evidence.
- Let DG evaluate them deterministically.

This makes DG safe-by-default and reduces reputational risk.

---

## Live Run vs Precheck (Side-by-Side)

| Aspect | Live Run | Precheck |
| --- | --- | --- |
| Evidence source | Providers | Caller-supplied |
| Trust lane | Verified (or policy) | Asserted |
| State mutation | Yes | No |
| Audit artifacts | Run state + runpack | Optional audit log |
| Use case | Production gating | Fast iteration |

---

## How Providers Plug In

Providers are registered by `provider_id` and handle EvidenceQuery:

```json
{
  "provider_id": "json",
  "predicate": "path",
  "params": { "file": "report.json", "jsonpath": "$.summary.failed" }
}
```

The provider returns:

```json
{
  "value": { "kind": "json", "value": 0 },
  "evidence_hash": null,
  "evidence_anchor": null,
  "signature": null,
  "content_type": "application/json"
}
```

DG applies the comparator (for example `equals` with expected `0`), produces a
tri-state, then evaluates gates via RET.

---

## Evidence Flow Diagram (Text)

```
Trigger or Next
  |
  | Scenario gates reference predicates
  v
EvidenceQuery (provider_id, predicate, params)
  |
  | Provider fetches/computes evidence
  v
EvidenceResult (value + hash/anchor/signature?)
  |
  | Comparator + tri-state
  v
RET evaluation (gate outcome)
  |
  | Decision + run state update
  v
Runpack artifacts (optional)
```

---

## Common Misconceptions

**"DG runs the tools."**  
No. Providers or external workflows run tools and emit evidence. DG evaluates.

**"Precheck is the same as a live run."**  
No. Precheck is read-only and uses asserted evidence.

**"MCP is required."**  
No. Built-ins + JSON cover many workflows without external MCP providers.

---

## When to Add a New Provider

Add a provider only when evidence cannot be expressed via:
- JSON artifacts,
- HTTP checks,
- time/env signals.

If your workflow can emit JSON, prefer the `json` provider. This keeps the
system simple and avoids new attack surfaces.

---

## Summary

Decision Gate is an evaluation engine:
- It **does not execute arbitrary tasks**.
- It **evaluates evidence** produced by providers or supplied in precheck.
- It is **deterministic, auditable, and safe-by-default**.

Use built-ins and JSON artifacts first. Add providers only when necessary.
