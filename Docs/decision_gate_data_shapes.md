# Decision Gate data shapes and schema FAQ

This FAQ maps the current code paths, contracts, and config. Each section cites
the most relevant files.

## Table of contents

- [Where does the data for checking the scenario come from?](#where-does-the-data-for-checking-the-scenario-come-from)
- [What did the "provider builder system / data shape" idea mean?](#what-did-the-provider-builder-system--data-shape-idea-mean)
- [How does the agent know the data shape? How does DG MCP validate it?](#how-does-the-agent-know-the-data-shape-how-does-dg-mcp-validate-it)
- [Do we need a runtime registry for dynamic data shapes?](#do-we-need-a-runtime-registry-for-dynamic-data-shapes)
- [Can agents query DG MCP for available schemas or shapes?](#can-agents-query-dg-mcp-for-available-schemas-or-shapes)
- [Who retrieves the data? How do we detect modification? Hashing and logs?](#who-retrieves-the-data-how-do-we-detect-modification-hashing-and-logs)
- ["LLM cannot lie", "LLM lied but we can prove it", "we have no idea"](#llm-cannot-lie-llm-lied-but-we-can-prove-it-we-have-no-idea)
- [Follow-up clarifications (out-of-band, agent-supplied data, precheck)](#follow-up-clarifications-out-of-band-agent-supplied-data-precheck)

## Where does the data for checking the scenario come from?

Short answer: live runs pull evidence from EvidenceProviders referenced by
predicates in the ScenarioSpec; precheck evaluates asserted payloads validated
against a registered data shape. Evidence results include a trust lane that is
enforced against global/scenario/gate/predicate requirements.

Key code path:

- ScenarioSpec predicates include EvidenceQuery (provider_id/predicate/params),
  which binds gate requirements to provider checks in the spec.
  `decision-gate-core/src/core/spec.rs`
- EvidenceQuery, EvidenceResult, TrustLane, and TrustRequirement are the
  canonical data contracts.
  `decision-gate-core/src/core/evidence.rs`
- Gate/predicate trust requirements are defined on the spec and applied as
  strictest-wins at evaluation time.
  `decision-gate-core/src/core/spec.rs`
  `decision-gate-core/src/runtime/engine.rs`
- The runtime constructs EvidenceContext from run/trigger metadata and evaluates
  predicates by calling the EvidenceProvider for each predicate.
  `decision-gate-core/src/runtime/engine.rs`
- Evidence is routed through the provider registry (built-ins and external MCP
  providers) and policy enforcement.
  `decision-gate-providers/src/registry.rs`
- The MCP layer uses a federated provider that wraps built-ins and external MCP
  providers (stdio or HTTP) and applies trust policy.
  `decision-gate-mcp/src/evidence.rs`
- Precheck loads a data shape from the registry, validates payloads, builds
  asserted evidence (lane = asserted), and calls the control plane precheck
  (read-only, no run-state mutation).
  `decision-gate-mcp/src/tools.rs`
  `decision-gate-core/src/runtime/engine.rs`
- Evidence records for live runs are stored in run state and exported in
  runpacks.
  `decision-gate-core/src/core/state.rs`
  `decision-gate-core/src/runtime/runpack.rs`

Docs overview of the same flow and roles:

- `README.md`

## What did the "provider builder system / data shape" idea mean?

There are now two distinct "shape" concepts:

Verified lane (provider contracts):

- Provider contracts define provider check parameter schemas, result schemas,
  allowed comparators, determinism class, and examples.
  `decision-gate-contract/src/types.rs`
- External MCP providers must ship provider contracts (capability contracts)
  and reference them via `capabilities_path` in config. This is the explicit,
  versioned shape for provider-pulled evidence.
  `Docs/guides/provider_development.md`
  `Docs/configuration/decision-gate.toml.md`

Asserted lane (data shape registry):

- Data shapes are JSON Schema records stored in a tenant+namespace registry,
  versioned and immutable once registered.
  `decision-gate-core/src/core/data_shape.rs`
  `decision-gate-core/src/runtime/store.rs`
  `decision-gate-store-sqlite/src/store.rs`
- Precheck uses data shapes to validate asserted payloads before evaluating gate
  logic.
  `decision-gate-mcp/src/tools.rs`

"Data source" is the provider implementation or the asserted payload:

- Built-in providers read from environment variables, files, HTTP endpoints, or
  trigger time. They are registered in the provider registry by provider_id.
  `decision-gate-providers/src/registry.rs`
- External providers are separate MCP servers and are registered in
  `decision-gate.toml` as `type = "mcp"` with a command/url.
  `decision-gate-mcp/src/config.rs`
- Asserted data is supplied by the caller to precheck and is never pulled from a
  provider.

So for something like ESPN:

- Verified lane: define provider checks with param/result schemas in a provider
  contract JSON, and implement the MCP provider that calls ESPN.
- Asserted lane: register a data shape schema for the payload you want to
  precheck, then call `precheck` with that payload.

The scenario author works "backwards" from the API by defining provider checks
(predicate names) that match the API's input/output. The ScenarioSpec then binds
those checks to gates via EvidenceQuery.

## How does the agent know the data shape? How does DG MCP validate it?

DG MCP validates in two places: provider contracts for verified evidence and
data shapes for asserted evidence.

Provider contract registry:

- The server loads provider contracts from built-ins or from
  `capabilities_path` in config, then compiles JSON Schema for params/results.
  `decision-gate-mcp/src/server.rs`
  `decision-gate-mcp/src/capabilities.rs`
- When a scenario is defined, DG MCP validates all provider check params and
  expected values against those compiled schemas, checks provider/check presence,
  and enforces strict comparator compatibility.
  `decision-gate-mcp/src/tools.rs`
  `decision-gate-mcp/src/validation.rs`
- Evidence queries are also validated against provider check param schemas.
  `decision-gate-mcp/src/tools.rs`
  `decision-gate-mcp/src/capabilities.rs`

Data shape registry (asserted lane):

- `schemas_register` stores data shapes after schema compilation and size
  limits, scoped by tenant+namespace.
  `decision-gate-mcp/src/tools.rs`
  `decision-gate-mcp/src/config.rs`
- `precheck` validates payloads against the registered data shape and applies
  the same strict comparator rules used for provider-backed scenarios.
  `decision-gate-mcp/src/tools.rs`
  `decision-gate-mcp/src/validation.rs`

How the agent learns the shape today:

- `providers_list` returns provider IDs, transports, and provider check names.
  `decision-gate-mcp/src/tools.rs`
- `schemas_list` and `schemas_get` return registered data shapes.
  `decision-gate-mcp/src/tools.rs`
- There is still no MCP tool that returns full provider param/result schemas;
  agents must obtain provider contracts out-of-band (e.g.
  `Docs/generated/decision-gate/providers.json` for built-ins, and the external
  provider's contract JSON for custom providers).
  `Docs/guides/provider_development.md`

Also note: ScenarioSpec includes `schemas` (SchemaRef) for packet schemas, but
that is still metadata in core; there is no runtime schema lookup or validation
for packet payloads yet.
`decision-gate-core/src/core/spec.rs`

## Do we need a runtime registry for dynamic data shapes?

Current state:

- Yes, for asserted evidence. There is a runtime data shape registry with
  register/list/get tooling and size limits.
  `decision-gate-mcp/src/tools.rs`
  `decision-gate-core/src/runtime/store.rs`
  `decision-gate-store-sqlite/src/store.rs`
- Provider contracts are still loaded once at startup from config; there is no
  runtime API to register new provider contracts on demand.
  `decision-gate-mcp/src/capabilities.rs`
  `decision-gate-mcp/src/config.rs`

If you want LLM-driven provider schema registration, it would require new tools
and a separate provider contract registry service (not implemented).

## Can agents query DG MCP for available schemas or shapes?

Yes, for registry-backed data shapes and provider summaries:

- `providers_list` returns provider IDs, transports, and provider check names.
- `schemas_list` and `schemas_get` return data shape records.
- `scenarios_list` returns registered scenarios by tenant+namespace.
  `decision-gate-mcp/src/tools.rs`
  `decision-gate-contract/src/tooling.rs`

Full provider schemas are still discovered out-of-band:

- Built-ins: `Docs/generated/decision-gate/providers.json`
- External: the provider's `capabilities_path` JSON in config
  `Docs/guides/provider_development.md`

## Who retrieves the data? How do we detect modification? Hashing and logs?

DG MCP retrieves evidence via providers for live runs; precheck uses asserted
payloads without provider calls.

- EvidenceContext is built from trigger/run metadata and passed to providers.
  `decision-gate-core/src/runtime/engine.rs`
- Evidence is normalized with canonical hashes at evaluation time (including
  asserted evidence in precheck).
  `decision-gate-core/src/runtime/engine.rs`
- Evidence records are stored in run state and exported into runpacks for live
  runs. Precheck is read-only and does not mutate run state.
  `decision-gate-core/src/core/state.rs`
  `decision-gate-core/src/runtime/runpack.rs`
  `decision-gate-core/src/runtime/engine.rs`

Agent-supplied artifacts are distinct:

- `scenario_submit` stores external artifacts and hashes their payloads but does
  not drive gate evaluation directly. It is an audit/disclosure mechanism.
  `decision-gate-core/src/runtime/engine.rs`

Modification detection mechanisms:

- EvidenceResult includes `evidence_hash` and optional `signature` or anchors
  to verify integrity or re-fetch evidence later.
  `decision-gate-core/src/core/evidence.rs`
- Provider trust policy can require signature verification.
  `decision-gate-mcp/src/evidence.rs`
  `Docs/guides/security_guide.md`
- `evidence_query` responses may redact raw values by policy while still
  returning hashes/anchors.
  `decision-gate-mcp/src/tools.rs`
  `Docs/guides/security_guide.md`

## "LLM cannot lie", "LLM lied but we can prove it", "we have no idea"

DG supports all three, depending on how you wire providers, data shapes, and
trust requirements:

A) LLM cannot lie (strongest)

- DG retrieves evidence from trusted providers directly and enforces the
  Verified lane (global `trust.min_lane` and optional gate/predicate overrides).
  If signatures are required, unsigned or tampered evidence is rejected.
  `decision-gate-core/src/core/evidence.rs`
  `decision-gate-core/src/runtime/engine.rs`
  `decision-gate-mcp/src/config.rs`
  `decision-gate-mcp/src/evidence.rs`

B) LLM lied but we can prove what it submitted

- Use `scenario_submit` (audit trail). `precheck` evaluates asserted payloads
  but does not persist them by itself, so proof requires external retention.
  `decision-gate-core/src/runtime/engine.rs`
  `decision-gate-mcp/src/tools.rs`

C) We have no idea if the LLM lied

- If evidence comes from an untrusted provider with audit-only policy, or if you
  rely on asserted evidence without retaining logs, DG can only show what it
  observed. The threat model calls out evidence trust boundaries and redaction
  policies.
  `Docs/security/threat_model.md`

In all cases, Decision Gate fails closed for missing/invalid evidence and
records hashes and logs for audit. The choice of provider trust policy, contract
quality, and data source determines which assurance level you get.

## Follow-up clarifications (out-of-band, agent-supplied data, precheck)

Out-of-band means "not fully discoverable via MCP tools":

- Provider contracts (param/result schemas) are still obtained from files or
  docs; the MCP tool surface only exposes provider summaries.
  `Docs/generated/decision-gate/providers.json`
  `Docs/guides/provider_development.md`
- Data shapes are discoverable via `schemas_list`/`schemas_get` once registered.
  `decision-gate-mcp/src/tools.rs`

Agent-supplied artifacts vs evidence:

- `scenario_submit` records submissions (payload + hash) into run state for
  audit, but submissions are not used for gate evaluation.
  `decision-gate-core/src/runtime/engine.rs`
  `decision-gate-core/src/core/state.rs`
- Gate evaluation uses EvidenceProviders for live runs; asserted payloads are
  only used in `precheck` today.
  `decision-gate-mcp/src/tools.rs`

Precheck vs "submit data to see if it would pass":

- The supported precheck tool is `precheck`, which validates an asserted
  payload against a registered data shape and runs gate logic without mutating
  run state.
  `decision-gate-mcp/src/tools.rs`
  `decision-gate-core/src/runtime/engine.rs`
- Strict comparator validation is default-on for both scenario_define and
  precheck; it can be relaxed only via config.
  `decision-gate-mcp/src/validation.rs`
  `decision-gate-mcp/src/config.rs`

Why allow agent-supplied data at all?

- It enables adoption when no MCP connector exists, but the trust model changes:
  asserted data can be evaluated, but it does not provide the same guarantees as
  provider-pulled evidence. DG can hash asserted payloads during evaluation, and
  scenario_submit/runpacks can retain submissions for audit, but it cannot
  guarantee truth without an external verifier.

B vs C (clarified):

- B = "we can prove what was submitted": DG stores SubmissionRecords (payload +
  hash) in run state/runpacks when you use `scenario_submit`, and provider
  EvidenceResults are stored for live runs.
  `decision-gate-core/src/core/state.rs`
  `decision-gate-core/src/runtime/runpack.rs`
- C = "we have no idea": this happens when you do not retain run state/runpacks,
  or when evidence has no verifiable anchors/signatures and you also discard the
  submitted payload/hashes. DG can only show what it observed if those logs are
  retained.
