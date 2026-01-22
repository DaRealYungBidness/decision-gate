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

Decision Gate is a deterministic, replayable control plane for gated disclosure.
It evaluates evidence-backed gates, emits auditable decisions, and supports
offline verification via runpacks. It is backend-agnostic and integrates via
explicit interfaces rather than embedding into agent frameworks.

RET stands for **Requirement Evaluation Tree** and refers to the universal
predicate algebra used by the engine.

## Table of Contents

- [Overview](#overview)
- [Architecture at a Glance](#architecture-at-a-glance)
- [Repository Layout](#repository-layout)
- [Core Concepts](#core-concepts)
- [How Predicates Are Defined](#how-predicates-are-defined)
- [Scenario Authoring Walkthrough](#scenario-authoring-walkthrough)
- [Built-in Providers (Predicate Reference)](#built-in-providers-predicate-reference)
- [Provider Example: MongoDB](#provider-example-mongodb)
- [MCP Tool Surface](#mcp-tool-surface)
- [Runpacks and Verification](#runpacks-and-verification)
- [Examples](#examples)
- [Glossary](#glossary)
- [Docs](#docs)
- [Security](#security)
- [Quick Start](#quick-start)

## Overview
Decision Gate is a control plane. It does not run conversations or agents.
It ingests triggers, evaluates evidence-backed predicates, and emits auditable
decisions and disclosures. Evidence is always tied to a provider and recorded
in run state to enable offline verification.

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
  v
Evidence sources
  - built-in providers (time, env, json, http)
  - external MCP providers (stdio or HTTP)

Runpack builder -> deterministic artifacts + manifest
```

## Repository Layout
- `decision-gate-core`: deterministic engine, schemas, and runpack tooling
- `decision-gate-broker`: reference sources/sinks and composite dispatcher
- `decision-gate-providers`: built-in evidence providers (time, env, json, http)
- `decision-gate-mcp`: MCP server and evidence federation
- `decision-gate-cli`: CLI for MCP server and runpack utilities
- `decision-gate-provider-sdk`: provider templates (TypeScript, Python, Go)
- `ret-logic`: universal predicate evaluation engine (RET)
- `examples/`: runnable examples (`minimal`, `file-disclosure`, `llm-scenario`, `agent-loop`, `ci-gate`, `data-disclosure`)

## Core Concepts
**ScenarioSpec**: The full scenario definition. It contains stages, gates, and
predicates. A scenario is the unit of execution.

**StageSpec**: A scenario stage. Each stage has one or more gates and an
advance policy (`linear`, `fixed`, `branch`, or `terminal`).

**GateSpec**: A gate with a requirement tree. This is where `ret-logic` applies.

**PredicateSpec**: A named predicate that binds a requirement leaf to an
evidence query and comparator.

**EvidenceQuery**: The canonical shape of a provider query:
`provider_id`, `predicate`, and `params`.

**EvidenceResult**: The provider response containing a value, hash, anchor,
and optional signature metadata.

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

Today, each provider documents its predicate format in code. In the next phase,
the canonical contract crate will define provider capabilities as Rust data
structures so the predicate schemas, docs, and tooltips are generated, not
hand-maintained.

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
  "requirement": { "and": [ { "pred": "deploy_env" }, { "pred": "build_passed" } ] }
}
```

### 4) Build Stages
Stages hold gates and define where the run goes next.

```json
{
  "stage_id": "main",
  "gates": [ { "gate_id": "ready", "requirement": { "pred": "deploy_env" } } ],
  "advance_to": "terminal",
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
- `runpack_export`
- `runpack_verify`

These are thin wrappers over the same core engine and are intended to be
code-generated into docs and SDKs.

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
**Provider**: An MCP server that supplies evidence for predicates.

**Connector**: The configuration entry that registers a provider.

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
