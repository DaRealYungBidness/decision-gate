<!--
Docs/guides/evidence_flow_and_execution_model.md
============================================================================
Document: Evidence Flow + Execution Model
Description: End-to-end explanation of how Decision Gate handles evidence,
             evaluation, and trust.
Purpose: Provide a precise mental model for data flow and evaluation.
Dependencies:
  - decision-gate-core/src/runtime/engine.rs
  - decision-gate-mcp/src/tools.rs
  - decision-gate-mcp/src/evidence.rs
  - decision-gate-providers/src/registry.rs
============================================================================
-->

# Evidence Flow + Execution Model

## At a Glance

**What:** How Decision Gate evaluates evidence and produces decisions
**Why:** Understand exactly what is fetched, validated, and compared
**Who:** Developers, security teams, and architects integrating Decision Gate
**Prerequisites:** [getting_started.md](getting_started.md)

## One Sentence You Must Remember

**Decision Gate evaluates evidence; it does not execute tasks.**

---

## Core Data Flow

```dg-skip dg-reason="output-only" dg-expires=2026-06-30
Trigger (scenario_next / scenario_trigger / precheck)
  |
  +-> Collect or accept evidence
  |     - Live run: call providers
  |     - Precheck: accept asserted payload
  |
  +-> Trust enforcement
  |     - Trust lane minimum (min_lane)
  |     - Signature policy (if required)
  |     - Anchor validation (if configured)
  |
  +-> Comparator evaluation -> TriState
  +-> RET evaluation -> Gate outcomes
  |
  +-> Decision
        - Live run: state mutation + runpack storage
        - Precheck: no state mutation
```

---

## Evidence Sources

### 1) Providers (Live Runs)

Providers fetch or compute evidence and return an `EvidenceResult`.

**Built-in providers:** `time`, `env`, `json`, `http`

**External providers:** MCP servers called via `tools/call` with `evidence_query`.

### 2) Asserted Evidence (Precheck)

Precheck **does not** call providers. The client supplies a payload that is:
1. Validated against a registered **data shape** (JSON Schema).
2. Converted into asserted `EvidenceResult` values.

**Payload mapping is exact:**
- If payload is an object: keys are condition IDs.
- If payload is not an object: it is only accepted when the scenario has exactly one condition.

---

## Trust Enforcement

### Trust Lanes
- `Verified`: evidence returned by providers.
- `Asserted`: evidence supplied via precheck payload.

### Minimum Lane (`min_lane`)
Configured in `decision-gate.toml`:
```toml dg-parse dg-level=fast
[trust]
min_lane = "verified"   # or "asserted"
```

When `min_lane = "verified"`, asserted evidence is rejected (condition becomes `unknown`).

**Dev-permissive:** `min_lane` becomes `asserted` automatically, **except** for providers listed in `dev.permissive_exempt_providers` (those remain strict).

### Per-Condition and Per-Gate Overrides
You can raise the minimum lane in the scenario spec:
```json dg-parse dg-level=fast
{
  "condition_id": "tests_ok",
  "trust": { "min_lane": "verified" }
}
```
Gate-level `trust` can also raise requirements. Effective requirement is the **stricter** of base and overrides.

### Signature Verification
Configured via `trust.default_policy`:
```toml dg-parse dg-level=fast
[trust]
# Audit mode accepts unsigned evidence.
default_policy = "audit"

# Require signatures from key files:
# default_policy = { require_signature = { keys = ["/etc/decision-gate/keys/provider.pub"] } }
```

When `require_signature` is active:
- `EvidenceResult.signature.scheme` must be `"ed25519"`.
- `signature.key_id` must match a configured key entry.
- The signature is verified over **canonical JSON of `evidence_hash`**.

If `evidence_hash` is missing, Decision Gate computes it from the evidence value.
If `evidence_hash` is present, it must match the canonical hash of the evidence
value or the provider response is rejected.

### Anchor Validation
Anchors are enforced via config (not the scenario spec):
```toml dg-parse dg-level=fast
[anchors]
[[anchors.providers]]
provider_id = "json"
anchor_type = "file_path"
required_fields = ["path"]
```

`EvidenceResult.evidence_anchor.anchor_value` must be a **string** containing canonical JSON that parses to an **object**. Required fields must exist and must be scalar (string/number). Violations produce `error.code = "anchor_invalid"` and the condition becomes `unknown`.

---

## Comparator Evaluation

Comparators produce **TriState** results:
- `true`
- `false`
- `unknown`

Important exact behaviors (see [condition_authoring.md](condition_authoring.md)):
- `equals`/`not_equals` return **false/true** on type mismatch (not `unknown`).
- Ordering comparators (`greater_than`, etc.) return `unknown` unless both sides are numbers or RFC3339 date/time strings.
- `exists`/`not_exists` test **presence of `EvidenceResult.value`**; JSON `null` still counts as `exists`.

---

## Live Run Flow (scenario_next)

1. A run exists (`scenario_start`).
2. `scenario_next` is called with `run_id`, `tenant_id`, `namespace_id`, `trigger_id`, `agent_id`, and `time`.
3. Decision Gate calls providers for each condition.
4. Trust requirements are enforced.
5. Comparators and RET produce gate outcomes.
6. A `DecisionRecord` is returned and the run state is updated.

**Output:** `NextResult { decision, packets, status }` (no evidence values).

To inspect evidence and gate details, use `runpack_export`.

---

## Precheck Flow (precheck)

1. Client calls `schemas_register` to register a **data shape**.
2. Client calls `precheck` with:
   - `tenant_id`, `namespace_id`
   - `scenario_id` **or** inline `spec`
   - `data_shape` (schema id + version)
   - `payload`
3. Payload is validated against the schema.
4. Payload is converted into asserted evidence.
5. Gates are evaluated without mutating run state.

**Output:** `PrecheckToolResponse { decision, gate_evaluations }`.

`gate_evaluations` includes only gate status and condition trace (no evidence values).

---

## Audit Submissions (scenario_submit)

`scenario_submit` appends a submission record to the run state **without** affecting gate evaluation. Each submission stores a content hash and metadata for audit.

---

## Provider Interaction (External MCP)

Decision Gate calls external MCP providers with `tools/call` and a single tool: `evidence_query`. It does **not** depend on `tools/list` at runtime; provider capabilities are loaded from `capabilities_path` in config.

---

## Common Misconceptions (Corrected)

- **"DG runs the tools."** -> False. DG evaluates evidence; tools run elsewhere.
- **"Precheck returns evidence errors."** -> False. It returns `decision` + `gate_evaluations` only.
- **"scenario_next response includes evidence."** -> False by default. Local-only requests default to `trace` feedback, but evidence values still require `feedback: "evidence"` (if permitted) or `runpack_export`/`evidence_query`.
- **"Anchor policy is in the scenario."** -> False. It is configured under `[anchors]`.

---

## Glossary

**EvidenceQuery:** `{ provider_id, check_id, params }`.
**EvidenceResult:** `{ value, lane, error, evidence_hash, evidence_ref, evidence_anchor, signature, content_type }`.
**Trust Lane:** `verified` or `asserted`.
**Runpack:** Audit artifact bundle written for live runs.
