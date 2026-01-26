<!--
README.md
============================================================================
Document: Decision Gate README
Description: Repository overview and quick start for Decision Gate.
Purpose: Introduce the project and link to core documentation.
Dependencies:
  - Docs/guides/getting_started.md
  - Docs/security/threat_model.md
============================================================================
-->

# Decision Gate

Decision Gate is a deterministic, replayable requirement-evaluation system for
gated steps and controlled disclosure. It evaluates evidence-backed gates (or
asserted data in precheck) to decide whether a plan can advance, emits auditable
decisions, and supports offline verification via runpacks. It is backend-agnostic
and integrates via explicit interfaces rather than embedding into agent frameworks.

RET stands for **Requirement Evaluation Tree** and refers to the universal
predicate algebra used by the engine.

## Table of Contents

- [Overview](#overview)
- [Current Status (Accuracy Notes)](#current-status-accuracy-notes)
- [Architecture at a Glance](#architecture-at-a-glance)
- [Repository Layout](#repository-layout)
- [Core Concepts](#core-concepts)
- [How Predicates Are Defined](#how-predicates-are-defined)
- [Scenario Authoring Walkthrough](#scenario-authoring-walkthrough)
- [Built-in Providers (Predicate Reference)](#built-in-providers-predicate-reference)
- [Provider Example: MongoDB](#provider-example-mongodb)
- [MCP Tool Surface](#mcp-tool-surface)
- [Contract Artifacts](#contract-artifacts)
- [Runpacks and Verification](#runpacks-and-verification)
- [Examples](#examples)
- [Glossary](#glossary)
- [Docs](#docs)
- [Security](#security)
- [Formatting](#formatting)
- [Quick Start](#quick-start)
- [References](#references)

## Overview
Decision Gate is a control plane for deterministic checkpoints. It does not run
conversations or agents. It ingests triggers, evaluates evidence-backed
predicates, and emits auditable decisions and disclosures. Evidence can be
provider-pulled (verified) or asserted for precheck; asserted data never mutates
run state. In the operational sense, this is LLM/task evaluation: progress is
gated until explicit requirements are satisfied.

## Current Status (Accuracy Notes)
Implemented:
- Trust lanes (verified vs asserted) with gate/predicate enforcement.
- Schema registry (versioned data shapes) and discovery tools.
- Precheck tool (read-only evaluation of asserted payloads).

Not yet implemented:
- Dev-permissive/untrusted mode toggle with explicit warnings.
- Registry RBAC/ACL beyond tool allowlists.
- Precheck audit hash-only enforcement.
- Default namespace policy for non-Asset-Core deployments.

## Architecture at a Glance
Decision Gate is both an MCP server (tool surface) and an MCP client (evidence
federation). The control plane is always the same codepath.

```text
LLM or client
  |
  | MCP JSON-RPC tools
  v
decision-gate-mcp (tools/list, tools/call)
  |
  | scenario_* -> ControlPlane (decision-gate-core)
  | evidence_query -> EvidenceProvider registry
  | schemas_* / precheck -> Schema registry + validation
  v
Evidence sources
  - built-in providers (time, env, json, http)
  - external MCP providers (stdio or HTTP)

Runpack builder -> deterministic artifacts + manifest
```

### Architecture Diagrams (Mermaid)
High-level topology and roles:

```mermaid
flowchart TB
  Client[LLM or client] -->|MCP JSON-RPC tools| MCP[decision-gate-mcp\n(MCP server + client)]
  MCP -->|scenario_* tools| CP[ControlPlane\n(decision-gate-core)]
  MCP -->|evidence_query| Registry[Evidence provider registry]
  Registry --> BuiltIn[Built-in providers\n(time, env, json, http)]
  Registry --> External[External MCP providers\n(stdio or HTTP)]
  External -->|MCP JSON-RPC| Remote[Other MCP servers]
  CP --> Runpack[Runpack builder]
  Runpack --> Artifacts[Runpack + manifest]
```

Evidence query flow (provider wiring):

```mermaid
sequenceDiagram
  participant Client as LLM or client
  participant MCP as decision-gate-mcp
  participant CP as ControlPlane
  participant Provider as Provider (built-in or external)
  participant Runpack as Runpack builder

  Client->>MCP: evidence_query
  MCP->>CP: validate + route
  CP->>Provider: EvidenceQuery\n(provider_id, predicate, params)
  Provider-->>CP: EvidenceResult\n(value, hash, anchor, signature?)
  CP-->>MCP: normalized result
  MCP-->>Client: tool response
  CP->>Runpack: record evidence + hashes
```

Scenario lifecycle (tools + runpacks):

```mermaid
flowchart TB
  Define[scenario_define] --> Start[scenario_start]
  Start --> Run[Active run state]
  Run -->|agent step| Next[scenario_next]
  Run -->|external event| Trigger[scenario_trigger]
  Next --> Decision[Decision + packets]
  Trigger --> Decision
  Decision -->|advance/hold| Run
  Decision -->|complete/fail| Done[Run finished]
  Status[scenario_status] -. read-only .-> Run
  Submit[scenario_submit] -. attach artifacts .-> Run
  Done --> Export[runpack_export]
  Export --> Verify[runpack_verify]
```

Provider terminology:
- **Provider**: an evidence source (built-in or external MCP server) that answers evidence queries.
- **Provider entry**: a `[[providers]]` config entry in `decision-gate.toml` that registers a provider.

## Repository Layout
- `decision-gate-core`: deterministic engine, schemas, and runpack tooling
- `decision-gate-broker`: reference sources/sinks and composite dispatcher
- `decision-gate-contract`: canonical contract definitions + generator
- `decision-gate-providers`: built-in evidence providers (time, env, json, http)
- `decision-gate-mcp`: MCP server and evidence federation
- `decision-gate-cli`: CLI for MCP server and runpack utilities
- `decision-gate-provider-sdk`: provider templates (TypeScript, Python, Go)
- `ret-logic`: universal predicate evaluation engine (RET)
- `examples/`: runnable examples (`minimal`, `file-disclosure`, `llm-scenario`, `agent-loop`, `ci-gate`, `data-disclosure`)

## Core Concepts
**ScenarioSpec**: The full scenario definition. It contains stages, gates, and
predicates. A scenario is the unit of execution.

**StageSpec**: A scenario stage. Each stage has zero or more gates and an
advance policy (`linear`, `fixed`, `branch`, or `terminal`).

**GateSpec**: A gate with a requirement tree. This is where `ret-logic` applies.

**PredicateSpec**: A named predicate that binds a requirement leaf to an
evidence query and comparator.

**EvidenceQuery**: The canonical shape of a provider query:
`provider_id`, `predicate`, and `params`.

**EvidenceResult**: The provider response containing a value, hash, anchor,
and optional signature metadata.

**TrustLane**: Evidence trust classification (`verified` or `asserted`), enforced
at gate/predicate level. Unmet trust yields Unknown and holds the run.

**Namespace**: Logical partition within a tenant for isolation of scenarios,
schemas, and run state.

**Data Shape**: Versioned JSON Schema used to validate asserted payloads for precheck.

**Runpack**: A deterministic bundle of run artifacts and a manifest for
offline verification.

## How Predicates Are Defined
This is the critical distinction:

- `ret-logic` defines **how predicates are composed** (AND, OR, NOT,
  require-group). It does not define predicate parameters.
- Providers define **what a predicate means** and which parameters are
  accepted. This is implemented inside each provider.

In practical terms, the predicate format is defined by:
1. The `EvidenceQuery` shape in `decision-gate-core` (provider_id, predicate, params).
2. The provider implementation that interprets `predicate` and `params`.

The canonical contract crate (`decision-gate-contract`) defines provider
capabilities as Rust data structures so predicate schemas, docs, and tooltips
are generated (not hand-maintained). Generated artifacts live under
`Docs/generated/decision-gate`. After any behavior or schema change, update the
contract tooltips and regenerate the generated artifacts to keep them aligned.

## Scenario Authoring Walkthrough
This is a full, end-to-end authoring flow using the core model.

### 1) Identify Evidence Sources
Decide where proof comes from. Each source is a provider (built-in or external).
Examples:
- `time` provider for scheduling
- `env` provider for environment gates
- `json` provider for file queries
- `http` provider for endpoint checks
- `mongodb` provider for database checks (external MCP provider)

### 2) Define Predicates
Predicates bind a provider query to a comparator. This is the proof surface.

```json
{
  "predicate": "deploy_env",
  "query": {
    "provider_id": "env",
    "predicate": "get",
    "params": { "key": "DEPLOY_ENV" }
  },
  "comparator": "equals",
  "expected": "production",
  "policy_tags": []
}
```

### 3) Compose Gates with ret-logic
Gates are requirement trees built from predicate keys.

```json
{
  "gate_id": "ready",
  "requirement": {
    "And": [
      { "Predicate": "deploy_env" },
      { "Predicate": "build_passed" }
    ]
  }
}
```

### 4) Build Stages
Stages hold gates and define where the run goes next.

```json
{
  "stage_id": "main",
  "gates": [ { "gate_id": "ready", "requirement": { "Predicate": "deploy_env" } } ],
  "advance_to": { "kind": "terminal" },
  "entry_packets": [],
  "timeout": null,
  "on_timeout": "fail"
}
```

### 5) Run the Scenario
Use MCP tools or the CLI to define, start, and advance the run:
- `scenario_define`
- `scenario_start`
- `scenario_next`
- `scenario_status`
- `scenario_submit`
- `scenario_trigger`

Runpacks can be exported and verified offline after execution.

## Built-in Providers (Predicate Reference)
These are the default providers shipped in `decision-gate-providers/src`.

### time
- `now`: returns the trigger timestamp as JSON.
- `after`: compares trigger time to a threshold.
- `before`: compares trigger time to a threshold.

Params:
```json
{ "timestamp": 1710000000000 }
```
or:
```json
{ "timestamp": "2024-01-01T00:00:00Z" }
```

### env
- `get`: fetches an environment variable.

Params:
```json
{ "key": "DEPLOY_ENV" }
```

### json
- `path`: read a JSON or YAML file and optionally select a JSONPath.

Params:
```json
{ "file": "/config.json", "jsonpath": "$.version" }
```

### http
- `status`: returns HTTP status code for a URL.
- `body_hash`: returns a hash of the response body.

Params:
```json
{ "url": "https://api.example.com/health" }
```

## Provider Example: MongoDB
MongoDB is not built-in. It would be implemented as an external MCP provider.

### Predicate Example
```json
{
  "predicate": "user_status",
  "query": {
    "provider_id": "mongodb",
    "predicate": "field_equals",
    "params": {
      "database": "app",
      "collection": "users",
      "filter": { "_id": "user-123" },
      "field": "status",
      "expected": "active"
    }
  },
  "comparator": "equals",
  "expected": true,
  "policy_tags": []
}
```

### What This Means
- The predicate format (`field_equals` and its params) is defined by the
  MongoDB provider, not by `ret-logic`.
- Decision Gate treats it as a query to the `mongodb` provider and evaluates
  the returned evidence with the comparator.

## MCP Tool Surface
Decision Gate exposes MCP tools that map directly to the control plane:
- `scenario_define`
- `scenario_start`
- `scenario_status`
- `scenario_next`
- `scenario_submit`
- `scenario_trigger`
- `evidence_query`
- `providers_list`
- `schemas_register`
- `schemas_list`
- `schemas_get`
- `scenarios_list`
- `precheck`
- `runpack_export`
- `runpack_verify`

These are thin wrappers over the same core engine and are intended to be
code-generated into docs and SDKs.

## Contract Artifacts
The contract generator emits deterministic artifacts for docs and SDKs:
- `Docs/generated/decision-gate/tooling.json`: MCP tool schemas
- `Docs/generated/decision-gate/providers.json`: provider predicate schemas
- `Docs/generated/decision-gate/schemas/`: scenario + config JSON schemas
- `Docs/generated/decision-gate/examples/`: canonical examples

Generate or verify artifacts:
```sh
cargo run -p decision-gate-contract -- generate
cargo run -p decision-gate-contract -- check
```

Schema validation tests (contract + runtime conformance):
```sh
cargo test -p decision-gate-contract --test schema_validation
cargo test -p decision-gate-mcp --test contract_schema_e2e
```

## Runpacks and Verification
Runpacks are deterministic bundles containing the scenario spec, trigger log,
gate evaluations, decisions, submissions, and tool calls. A manifest with hashes
enables offline verification of integrity and tamper detection.

## Examples
- `examples/minimal`: core scenario lifecycle
- `examples/file-disclosure`: packet disclosure flow
- `examples/llm-scenario`: LLM-style scenario
- `examples/agent-loop`: multi-step gate satisfaction
- `examples/ci-gate`: CI approval gate
- `examples/data-disclosure`: disclosure stage with packets

## Glossary
**Provider**: An evidence source (built-in or external MCP server) that supplies predicates.

**Provider entry**: The `[[providers]]` configuration entry that registers a provider.

**Adapter**: A generic term for a provider; use "provider" in Decision Gate.

**Predicate**: A named evidence check, defined by a provider query.

**Requirement**: A logical composition of predicates (AND, OR, NOT, group).

**Scenario**: The full definition of stages, gates, and predicates.

**Gate**: A requirement tree that must pass to advance a stage.

**Evidence**: Provider output recorded with hashes and anchors.

**Runpack**: A deterministic artifact bundle used for offline verification.

## Docs
- Getting started: `Docs/guides/getting_started.md`
- Configuration: `Docs/configuration/decision-gate.toml.md`
- Provider development: `Docs/guides/provider_development.md`
- Security guide: `Docs/guides/security_guide.md`
- Integration patterns: `Docs/guides/integration_patterns.md`

## Security
Decision Gate assumes hostile inputs and fails closed on missing or invalid
evidence. See `Docs/security/threat_model.md` for the full threat model.

## Formatting
Formatting requires nightly rustfmt. Use:
```sh
cargo +nightly fmt --all
```
Do not use `cargo fmt` in this repo.

## Quick Start
- Run core tests: `cargo test -p decision-gate-core`
- Run broker tests: `cargo test -p decision-gate-broker`
- Run examples:
  - `cargo run -p decision-gate-example-minimal`
  - `cargo run -p decision-gate-example-file-disclosure`
  - `cargo run -p decision-gate-example-llm-scenario`
  - `cargo run -p decision-gate-example-agent-loop`
  - `cargo run -p decision-gate-example-ci-gate`
  - `cargo run -p decision-gate-example-data-disclosure`
- Run the CLI:
  - `cargo run -p decision-gate-cli -- serve --config decision-gate.toml`
- Durable run state: configure `run_state_store` in `decision-gate.toml` to use
  the SQLite backend (see `Docs/configuration/decision-gate.toml.md`).

## References

Kublai Khan. (2017). _The Hammer_ [Audio recording]. YouTube. https://www.youtube.com/watch?v=8GGMdMo61_o

Paleface Swiss. (2023). _The Gallow_ [Audio recording]. YouTube. https://www.youtube.com/watch?v=ThvEJXMeYOA

The Amity Affliction. (2014). _Pittsburgh_ [Audio recording]. YouTube. https://www.youtube.com/watch?v=vu3xGr-lNVI
