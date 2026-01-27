<!--
Docs/roadmap/provider_contract_discovery_plan.md
============================================================================
Document: Provider Contract Discovery Plan
Description: World-class plan for provider contract/schema discovery tools.
Purpose: Define scope, security posture, interfaces, and implementation steps
         for MCP + CLI discovery of provider contracts and compiled schemas.
Dependencies:
  - decision-gate-mcp/src/capabilities.rs
  - decision-gate-mcp/src/tools.rs
  - decision-gate-cli/src/lib.rs
  - decision-gate-contract/src/tooling.rs
  - Docs/security/threat_model.md
============================================================================
-->

# Provider Contract Discovery Plan (World-Class)

This plan defines a world-class discovery surface for provider contracts and
compiled schemas. It is written to be actionable by an LLM or engineer and is
expected to evolve as the implementation progresses.

**Status:** Implemented in OSS (MCP + CLI + contract registry). Tests added in
unit + system suites. Bulk export remains out of scope for now (explicitly
deferred).

If you only remember one sentence:
**Discovery must be deterministic, auditable, access-controlled, and thinly
wrapped across MCP and CLI.**

---

## Goals

1) **Enable authoring without out-of-band docs**  
   Allow agents and humans to discover provider params/result schemas and
   comparator allow-lists through first-class tools.

2) **Keep security posture strict**  
   Discovery is read-only but still sensitive. Enforce authz, limits, and audit.

3) **Single source of truth**  
   MCP tools and CLI commands must use the same core capability registry API.

4) **Deterministic + reproducible**  
   Responses should be canonical JSON with stable ordering and hashes.

---

## Decisions (Locked)

### D1) Disclosure Default
**Default-on with denylist**.  
Rationale: discovery is required for safe authoring and can be restricted by
authz + allow/deny lists. Default-off would impair usability and push users to
out-of-band docs, which is worse for safety and correctness.

### D2) Outputs
**Return both raw contract JSON and compiled schema views.**  
Rationale: raw contracts are canonical and auditable; compiled schemas are
ergonomic for tools and LLMs. Both are needed to be world-class.

---

## Scope (What This Means and Why It Matters)

“Scope” answers *how much* data a single query should return and at what
granularity. There are three practical levels:

1) **Per-provider (contract-level)**  
   Return the full provider contract JSON. This is canonical and complete.

2) **Per-predicate (schema-level)**  
   Return a focused view: params schema, result schema, comparators, examples.
   This is what authoring tools actually need.

3) **Bulk export (all providers)**  
   Return all contracts or all compiled schemas at once. This is convenient
   but raises size, performance, and disclosure risks.

**World-class default**:
- Support **per-provider** and **per-predicate** as first-class operations.
- Provide **bulk export** only if it is explicitly enabled and size-limited.

Implications:
- Per-provider is best for audit and offline tooling.
- Per-predicate is best for LLM authoring and UI forms.
- Bulk export must be guarded by size limits and authz to avoid data sprawl.

---

## Tool Surface (MCP) - Implemented

### 1) `provider_contract_get`
Returns the exact provider contract JSON as loaded by MCP.

**Request**
```json
{ "provider_id": "json" }
```

**Response (shape)**
```json
{
  "provider_id": "json",
  "contract": { ... },          // exact contract JSON
  "contract_hash": { ... },     // canonical hash for audit
  "source": "builtin|file",
  "version": "vX.Y"             // if present in contract
}
```

### 2) `provider_schema_get`
Returns compiled, predicate-level schemas.

**Request**
```json
{ "provider_id": "json", "predicate": "path" }
```

**Response (shape)**
```json
{
  "provider_id": "json",
  "predicate": "path",
  "params_schema": { ... },
  "result_schema": { ... },
  "allowed_comparators": [ "equals", "in_set", ... ],
  "examples": [ ... ],
  "contract_hash": { ... }
}
```

### Optional 3) `provider_contracts_export` (deferred)
Bulk export is intentionally deferred. If added later it must be explicitly
enabled and size-limited.

---

## CLI Surface (Thin Wrapper) - Implemented

The CLI must call the same shared registry functions as MCP.

- `decision-gate-cli provider contract get --provider json`
- `decision-gate-cli provider schema get --provider json --predicate path`
- `decision-gate-cli provider contracts export` (optional + gated)

CLI output must be identical (canonical JSON) to MCP output for the same input.

---

## Security Requirements (World-Class) - Implemented

1) **Authz required**  
   Discovery tools must respect tool allowlists and authz roles.

2) **Disclosure policy**  
   Configurable allowlist/denylist for which provider contracts can be exposed.

3) **Size limits**  
   Response size must be bounded to prevent abuse.

4) **Hash-only audit by default**  
   Audit logs must default to hashes rather than full contract payloads.

5) **Deterministic serialization**  
   Canonical JSON (RFC 8785) or an equivalent canonical form.

6) **No secrets in contracts**  
   Contracts are schema + examples only. Any secret-bearing fields are invalid.

---

## Implementation Plan (Phased) - Completed

### Phase 0: Preconditions (done)
- Capability registry exposes raw contract + compiled schemas.
- Disclosure policy and defaults documented in configuration docs.

### Phase 1: Core API (done)
- Registry accessors return:
  - raw contract by provider_id
  - compiled schema by provider_id + predicate
- Canonical hashing added for responses.

### Phase 2: MCP Tools (done)
- Tool schemas + tooltips published via generated tooling artifacts.
- Handlers implemented in `decision-gate-mcp/src/tools.rs`.
- Disclosure policy, size limits, and audit logging enforced.

### Phase 3: CLI Commands (done)
- CLI subcommands call shared registry APIs.
- Deterministic canonical output implemented (RFC 8785 canonical JSON).

### Phase 4: Tests (done, partial on size limits)
- Contract/schema discovery success path (unit + system tests).
- Denylist enforcement (unit + system tests).
- Size limit failures (unit tests). System tests for size limits are pending.
- Canonical hash stability (unit tests via contract schema validation).

---

## Acceptance Criteria (Met)

- MCP and CLI outputs are byte-for-byte identical for equivalent requests.
- Access control and disclosure policy are enforced.
- Large contracts are rejected or truncated deterministically.
- Responses include a stable hash to support offline verification.
- Documentation clearly instructs users and LLMs how to use the tools.

---

## Implementation Notes (Current State)

- **Config surface:** `provider_discovery` supports `allowlist`, `denylist`,
  and `max_response_bytes` (default-on with denylist).
- **Tooling:** `provider_contract_get` and `provider_schema_get` are exposed via
  MCP + CLI with canonical JSON output and contract hash.
- **Disclosure:** Discovery respects tool allowlists + provider discovery
  allowlist/denylist and fails closed when denied.
- **Size limits:** Responses are bounded by canonical JSON bytes.
- **Docs:** README + MCP/CLI docs + configuration and provider development
  guides updated to reflect discovery tooling.

---

## Open Questions (Remaining)

1) **Bulk export**  
   Still deferred. If added, clarify explicit opt-in, pagination strategy, and
   security review for large response disclosure.

2) **System-test size limits**  
   Add a system test that forces `max_response_bytes` failure for a provider
   with intentionally large contract payloads.
