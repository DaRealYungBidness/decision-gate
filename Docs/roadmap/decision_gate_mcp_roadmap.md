<!--
Docs/roadmap/decision_gate_mcp_roadmap.md
============================================================================
Document: Decision Gate MCP Development Roadmap
Description: Implementation roadmap and acceptance criteria for Decision Gate MCP.
Purpose: Define phases, requirements, and quality bar for MCP delivery.
Dependencies:
  - Docs/standards/codebase_engineering_standards.md
============================================================================
-->

# Decision Gate MCP Development Roadmap

*Reference Document - January 2026*

---

## Quality Bar (Non-Negotiable)

This roadmap is a build spec intended for full implementation by automated agents. Every decision
must meet hyperscaler-grade standards: deterministic behavior, fail-closed semantics, and explicit
auditability. When in doubt during implementation, **choose the stricter, safer interpretation**.

## Normative Language

This document uses **MUST**, **SHOULD**, and **MAY** as defined in RFC 2119. Requirements marked
MUST are non-negotiable.

## Verified Core Reality (decision-gate-core, current repo state)

The following statements are verified against the current `decision-gate-core` implementation:

- `ControlPlane` methods: `new`, `start_run`, `scenario_status`, `scenario_next`, `scenario_submit`,
  and `trigger`.
- `scenario_status` **appends a tool-call record** (it is not side-effect free).
- Runpack artifacts are written under `artifacts/` with these filenames:
  - `artifacts/scenario_spec.json`
  - `artifacts/triggers.json`
  - `artifacts/gate_evals.json`
  - `artifacts/decisions.json`
  - `artifacts/packets.json`
  - `artifacts/submissions.json`
  - `artifacts/tool_calls.json`
  - `artifacts/verifier_report.json` (only when using `build_with_verification`)
- `RunpackBuilder` exposes `build` and `build_with_verification`; verification is
  `RunpackVerifier::verify_manifest`.
- `EvidenceQuery` is a struct in `decision-gate-core` with `provider_id`, `predicate`, and `params`.

Any roadmap item that deviates from the above **requires an explicit core schema/API change**.

---

## Design Alignment Notes (Implementation Requirements)

### 1) MCP Tools Are Thin Wrappers (No Divergent Logic)

MCP tools MUST be thin wrappers over the canonical control-plane engine. The same codepath used by
decision-gate-core internally MUST be used by MCP tools externally.

**Enforcement**:
- Adapter tests MUST compare MCP results to direct core calls for identical inputs.
- MCP layer MUST contain only serialization/deserialization and transport.
- Document explicitly: "MCP tools invoke `ControlPlane::*` methods directly."

### 2) Canonical Tool Surface: `scenario_*`

The canonical verb set is `scenario_*`, matching decision-gate-core's existing API surface. There
is no `gate_*` alternative. Any gate-centric tools MAY be aliases but MUST map to `scenario_*`
methods with identical semantics.

### 3) Scenario Definition and Run Lifecycle

Decision Gate is scenario-centric. A scenario MUST be defined as a full `ScenarioSpec`, even for
single-gate use cases. Run lifecycle is explicit and MUST be modeled separately from spec creation.

**Tool-to-Core Mapping (Verified)**:

| MCP Tool | Core Method | Required Behavior |
|----------|-------------|-------------------|
| `scenario_define` | `ControlPlane::new(spec, ...)` | Validate spec and register for execution |
| `scenario_start` | `ControlPlane::start_run(...)` | Create run state and optionally issue entry packets |
| `scenario_status` | `ControlPlane::scenario_status(...)` | Returns status **and logs tool call** |
| `scenario_next` | `ControlPlane::scenario_next(...)` | Evaluates + advances state if gates pass |
| `scenario_submit` | `ControlPlane::scenario_submit(...)` | Records external submissions |
| `scenario_trigger` | `ControlPlane::trigger(...)` | Push-mode trigger ingestion |
| `runpack_export` | `RunpackBuilder::build(...)` | Generate runpack artifacts + manifest |
| `runpack_verify` | `RunpackVerifier::verify_manifest(...)` | Offline verification |

**Optional**:
- `scenario_evaluate` is NOT present in core today. If added, it MUST be pure (no state mutation)
  and MUST be explicitly documented as a new core API.

### 4) Evidence Provider Routing: Explicit Provider Identity

Provider identity MUST be explicit in the spec. Two acceptable models:

1. Add `provider_id` to `PredicateSpec` or `EvidenceQuery` (preferred for determinism).
2. Use `provider::predicate` namespacing **only** if the delimiter and validation rules are
   formalized and enforced.

**Current core uses an `EvidenceQuery` enum**, so implementing explicit provider fields requires a
schema change in decision-gate-core. This MUST be called out in the implementation plan.

### 5) `evidence_query` Tool: Policy-Gated, Anchors by Default

`evidence_query` is a debug/introspection surface and MUST respect the fail-closed security
posture. Default behavior MUST return anchors/hashes only (no raw values). Raw values MAY be
returned only with explicit policy and provider opt-in.

### 6) Runpack Format: Align to Existing Artifacts

Runpack output MUST align with the current artifact layout in decision-gate-core unless a new
version is explicitly introduced. The canonical layout is:

```
-- manifest.json
-- artifacts/
    +-- scenario_spec.json
    +-- triggers.json
    +-- gate_evals.json
    +-- decisions.json
    +-- packets.json
    +-- submissions.json
    +-- tool_calls.json
    +-- verifier_report.json (optional)
```

If a new runpack format is introduced, it MUST be versioned and documented as such.

### 7) Soundness: What It Means

"Logically sound" includes four layers:

| Layer | Validation | Responsibility |
|-------|------------|----------------|
| **Structural** | Spec parses; unique IDs; gates reference defined predicates | `ScenarioSpec::validate()` |
| **Semantic** | Comparator vs value type compatibility; well-formed queries | New validation helpers (not in core today) |
| **Determinism** | Evidence sources snapshot-anchored or flagged volatile | `EvidenceProvider` contract |
| **Security** | No silent trust; missing evidence = Unknown = hold | Core evaluation semantics |

Ret-logic handles boolean algebra. Decision Gate owns spec validation, determinism enforcement,
and security checks.

### 8) Missing Providers: Fail Fast with Diagnostics

If evaluation requires providers that are not registered, the system MUST fail immediately with a
typed error, before evaluation begins:

```rust
pub struct ProviderMissingError {
    pub missing_providers: Vec<String>,
    pub required_capabilities: Vec<String>,
    pub blocked_by_policy: bool,
}
```

This is a preflight check. If a run exists, the failure MUST be logged in run state for auditability.

### 9) CLI: Follow acctl i18n Standards

If a CLI is built, it MUST follow the AssetCore acctl i18n pattern:

- All user-facing strings through the `t!()` macro with hierarchical keys.
- Message catalog in a dedicated `i18n.rs`.
- Named placeholders only; no inline English strings in new code.

**Reference files (Windows paths)**:
- `C:\Users\Micha\Documents\GitHub\Asset-Core\acctl-core\src\i18n.rs`
- `C:\Users\Micha\Documents\GitHub\Asset-Core\acctl-core\src\tests\i18n.rs`
- `C:\Users\Micha\Documents\GitHub\Asset-Core\acctl\src\main.rs`
- `C:\Users\Micha\Documents\GitHub\Asset-Core\acctl\AGENTS.md`

---

## Policy/Auth Interface (Current)

The MCP server now enforces authn/authz for tool calls. The default is
local-only (stdio/loopback), and production deployments must enable bearer
token or mTLS subject enforcement.

**Requirements**:
- The architecture exposes explicit auth interfaces and per-tool authorization
  with audit logging.
- The default configuration emits warnings stating "local-only mode" whenever
  the MCP server starts without explicit auth.
- Evidence access remains policy-gated (raw evidence denied by default).
- External network exposure (non-local transport) requires auth and transport
  hardening.

**Rationale**: This preserves the security posture and keeps the path to
hyperscaler-grade deployments without redesign.

---

## Strategic Context

**Goal**: Make decision-gate the universal standard for LLM data-driven decisions/checks.

**Business Strategy**:
- Fully open source, independent of AssetCore (no shared crates).
- "This is what we run internally" - credibility through dogfooding.
- Long-term: fold into AssetCore as managed service with namespace-scoped plans.

---

## Architecture: MCP-First, Federation-Native

### Core Insight

Decision-gate should be **both** an MCP server **and** an MCP client:

```
+------------------------------------------------------------------+
|                        LLM / Agent                               |
+------------------------------------------------------------------+
                              |
                              | MCP Protocol (JSON-RPC)
                              v
+------------------------------------------------------------------+
|                   decision-gate-mcp                              |
|  +-------------+  +-------------+  +-------------------------+   |
|  | Scenario    |  | Evidence    |  | Runpack                 |   |
|  | Tools       |  | Evaluation  |  | Generation              |   |
|  | (status,    |  | Engine      |  | & Verification          |   |
|  |  next,      |  | (ret-logic) |  |                         |   |
|  |  submit)    |  |             |  |                         |   |
|  +-------------+  +-------------+  +-------------------------+   |
|                          |                                       |
|                          | Evidence Queries                      |
|                          v                                       |
|  +---------------------------------------------------------------+
|  |              Evidence Provider Registry                       |
|  |  +---------+  +---------+  +---------+  +-----------------+   |
|  |  | Built-in|  | MCP     |  | MCP     |  | MCP Server      |   |
|  |  | Sources |  | Server  |  | Server  |  | (Community)     |   |
|  |  | (time,  |  | (files) |  | (db)    |  | Any Language    |   |
|  |  | env)    |  |         |  |         |  |                 |   |
|  |  +---------+  +---------+  +---------+  +-----------------+   |
|  +---------------------------------------------------------------+
+------------------------------------------------------------------+
```

### Why MCP Federation Solves the Connector Problem

1. **Language Agnostic**: Anyone can write an MCP server in TypeScript, Python, Go, etc.
2. **Existing Ecosystem**: Filesystem, database, GitHub MCP servers already exist.
3. **Process Isolation**: Security boundary at process level, not in-process plugins.
4. **Standard Protocol**: JSON-RPC 2.0, well-documented, tooling exists.
5. **No Rust Required**: Community contributes in their preferred language.

---

## Crate Structure (Target)

```
decision-gate/
+-- ret-logic/              # Universal requirement algebra
+-- decision-gate-core/     # Deterministic engine, schemas, runpack
+-- decision-gate-broker/   # Reference sources/sinks (optional)
+-- decision-gate-mcp/      # MCP server + evidence federation
+-- decision-gate-providers/# Built-in evidence providers
|   +-- time/
|   +-- env/
|   +-- json/
|   +-- http/
+-- decision-gate-cli/      # Optional CLI (i18n required)
+-- decision-gate-provider-sdk/ # Provider templates + protocol
+-- examples/
    +-- agent-loop/
    +-- ci-gate/
    +-- data-disclosure/
```

---

## Phase 1: MCP Foundation

### 1.1 decision-gate-mcp Crate

**MCP Server Tools** (exposed to LLMs):

| Tool | Purpose |
|------|---------|
| `scenario_define` | Define a ScenarioSpec (single or multi-gate) |
| `scenario_start` | Start a run and optionally issue entry packets |
| `scenario_status` | Return status (logs tool call) |
| `scenario_next` | Evaluate and advance if gates pass |
| `scenario_submit` | Record external task completion |
| `scenario_trigger` | Push-mode trigger ingestion |
| `evidence_query` | Query evidence source (anchors-only by default) |
| `runpack_export` | Export cryptographic audit trail |
| `runpack_verify` | Verify runpack integrity |

**MCP Client Mode** (consuming evidence providers):

```rust
pub trait EvidenceProviderRegistry {
    /// Register an MCP server as an evidence source.
    fn register_mcp_provider(&mut self, config: McpProviderConfig);

    /// Query evidence from the specified provider.
    async fn query(&self, query: EvidenceQuery) -> Result<EvidenceResult, EvidenceError>;

    /// Preflight check: verify all required providers are available.
    fn validate_providers(&self, spec: &ScenarioSpec) -> Result<(), ProviderMissingError>;
}
```

### 1.2 Built-in Evidence Providers

Ship with zero-config providers for common cases:

| provider_id | Evidence Type | Example PredicateSpec |
|-------------|--------------|-----------------------|
| `time` | Current time, scheduling | `PredicateSpec { predicate: "after", query: { provider_id: "time", predicate: "after", params: { "timestamp": "2024-01-01T00:00:00Z" } }, comparator: "equals", expected: true }` |
| `env` | Environment variables | `PredicateSpec { predicate: "deploy_env", query: { provider_id: "env", predicate: "get", params: { "key": "DEPLOY_ENV" } }, comparator: "equals", expected: "production" }` |
| `json` | JSON/YAML file queries | `PredicateSpec { predicate: "version", query: { provider_id: "json", predicate: "path", params: { "file": "/config.json", "jsonpath": "$.version" } }, comparator: "gte", expected: "2.0" }` |
| `http` | HTTP endpoint checks | `PredicateSpec { predicate: "health", query: { provider_id: "http", predicate: "status", params: { "url": "https://api.example.com/health" } }, comparator: "equals", expected: 200 }` |

### 1.3 Configuration Schema

```toml
# decision-gate.toml

[server]
transport = "stdio"  # or "sse", "http"

[trust]
default_policy = "audit"  # "audit" | "require_signature"

[[providers]]
name = "files"
type = "mcp"
command = ["npx", "@modelcontextprotocol/server-filesystem", "/data"]
trust = "audit"

[[providers]]
name = "database"
type = "mcp"
command = ["python", "-m", "mcp_postgres", "--conn", "postgresql://..."]
trust = { require_signature = { keys = ["key1.pub"] } }

[[providers]]
name = "time"
type = "builtin"
```

---

## Phase 2: Gate Definition & Evaluation

### 2.1 Gate Definition DSL

Support multiple authoring methods (already in ret-logic). **These examples are pseudocode** and
must be mapped to the canonical `ScenarioSpec`/`PredicateSpec` types:

**RON Format** (declarative):
```ron
Gate(
    name: "deployment_approved",
    requirement: And([
        Pred(provider_id: "ci", predicate: "status", comparator: Equals, expected: "passed"),
        Pred(provider_id: "review", predicate: "approvals", comparator: GreaterThan, expected: 1),
        Or([
            Pred(provider_id: "env", predicate: "DEPLOY_ENV", comparator: NotEquals, expected: "production"),
            Pred(provider_id: "review", predicate: "approvals", comparator: GreaterThan, expected: 2),
        ]),
    ]),
)
```

### 2.2 LLM Target Mode

The killer feature: gates as **targets** agents plan toward.

```
LLM Tool Call: scenario_define({
    scenario_id: "task_complete",
    stages: [{
        stage_id: "main",
        gates: [{
            gate_id: "all_requirements_met",
            requirement: { "and": [
                { "pred": "file_exists" },
                { "pred": "tests_pass" },
                { "pred": "review_approved" }
            ]}
        }],
        advance_to: "terminal"
    }],
    predicates: [
        { "predicate": "file_exists", "query": { "provider_id": "filesystem", "predicate": "file_exists", "params": { "path": "/output/report.pdf" }}, "comparator": "equals", "expected": true },
        { "predicate": "tests_pass", "query": { "provider_id": "shell", "predicate": "exit_code", "params": { "command": "pytest" }}, "comparator": "equals", "expected": 0 },
        { "predicate": "review_approved", "query": { "provider_id": "github", "predicate": "pr_approvals", "params": { "pr": 123 }}, "comparator": "gte", "expected": 1 }
    ]
})
```

The LLM can then:
- Query `scenario_status` to see which predicates are unsatisfied.
- Take actions to satisfy predicates.
- Query `scenario_next` to re-evaluate and advance if gates pass.

---

## Phase 3: Runpack & Audit Trail

### 3.1 Runpack Structure

Runpacks use the canonical artifact layout from decision-gate-core:

```
runpack/
+-- <manifest path chosen by sink>  # RunpackManifest (examples use run_manifest.json)
+-- artifacts/
    +-- scenario_spec.json
    +-- triggers.json
    +-- gate_evals.json
    +-- decisions.json
    +-- packets.json
    +-- submissions.json
    +-- tool_calls.json
    +-- verifier_report.json (optional)
```

**Key properties**:
- All JSON uses RFC 8785 canonical serialization.
- The runpack manifest contains SHA-256 hashes of all artifacts.
- Evidence values are hashed; raw values stored only if policy permits.
- Entire runpack is offline-verifiable with no external dependencies.

### 3.2 Offline Verification

```bash
# Verify runpack integrity
decision-gate verify runpack.json

# Verify with evidence replay
decision-gate verify runpack.json --replay-evidence

# Export human-readable report
decision-gate verify runpack.json --format markdown > audit-report.md
```

---

## Phase 4: Security Hardening

### 4.1 Zero Trust Evidence Model

| Layer | Protection |
|-------|------------|
| Transport | TLS for remote MCP connections |
| Authentication | Bearer tokens or mTLS for MCP providers |
| Evidence Integrity | SHA-256 hash of all evidence values |
| Evidence Authenticity | Optional Ed25519 signatures |
| Runpack Integrity | RFC 8785 canonical JSON + hash chain |
| Fail-Closed | Unknown evidence = hold/deny advancement |

### 4.2 Threat Model Alignment

Document in `Docs/security/threat_model.md`:
- Evidence provider compromise scenarios.
- Runpack tampering detection.
- Time-of-check vs time-of-use for evidence.
- Resource exhaustion via complex gates.

---

## Phase 5: Community & Ecosystem

### 5.1 Evidence Provider SDK

Provide templates for common languages:

```
decision-gate-provider-template/
+-- typescript/
+-- python/
+-- go/
+-- spec/
```

### 5.2 Documentation

- **Getting Started**: 5-minute tutorial with built-in providers.
- **Provider Development Guide**: How to build custom evidence providers.
- **Security Guide**: Trust policies, signature verification.
- **Integration Patterns**: CI/CD, agent loops, approval workflows.

---

## Implementation Order

0. **Document this plan** - Maintain `Docs/roadmap/decision_gate_mcp_roadmap.md`.
1. **decision-gate-mcp** - Core MCP server with scenario tools.
2. **Built-in providers** - time, env, json, http.
3. **MCP client mode** - Federation with external MCP servers.
4. **Trust policies** - Configurable signature requirements + raw evidence controls.
5. **Provider templates** - TypeScript, Python SDKs.
6. **Examples** - Agent loop, CI gate, data disclosure.

---

## Acceptance Tests (Minimum Bar)

- Deterministic evaluation: same inputs produce identical outcomes and runpacks.
- Idempotence: repeated `scenario_next` with same trigger yields identical decision.
- Safe summary: no evidence values are leaked in hold responses.
- Missing provider: preflight fails fast with typed error.
- Runpack verification: altered artifact fails verification.

---

## Success Criteria

- [ ] LLM can define scenarios as explicit targets.
- [ ] LLM can evaluate scenarios against arbitrary evidence sources.
- [ ] Runpacks provide cryptographically verifiable audit trail.
- [ ] Community can add evidence providers in any language.
- [ ] Zero-friction adoption (works out of box with built-in providers).
- [ ] Scales to high-assurance environments (signature policies).
