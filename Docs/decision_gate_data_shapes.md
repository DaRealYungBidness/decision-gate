# Decision Gate data shapes, data sources, and schema questions (Q1-Q7)

This note answers the seven questions by mapping the code paths, contracts, and
config that exist today. Each section cites the most relevant files.

## 1) Where does the data for checking the scenario come from?

Short answer: from EvidenceProviders referenced by predicates in the ScenarioSpec.
At runtime, the control plane builds an EvidenceContext from the trigger/run and
asks providers for EvidenceResults. Those results (plus hashes) are recorded in
run state and exported in runpacks.

Key code path:
- ScenarioSpec predicates include EvidenceQuery (provider_id/predicate/params),
  which binds gate requirements to provider queries in the spec.
  `decision-gate-core/src/core/spec.rs:47`
- EvidenceQuery and EvidenceResult are the canonical data contracts.
  `decision-gate-core/src/core/evidence.rs:30`
- The runtime constructs EvidenceContext from run/trigger metadata and evaluates
  predicates by calling the EvidenceProvider for each predicate.
  `decision-gate-core/src/runtime/engine.rs:506`
- Evidence is routed through the provider registry (built-ins and external MCP
  providers) and policy enforcement.
  `decision-gate-providers/src/registry.rs:86`
- The MCP layer uses a federated provider that wraps built-ins and external MCP
  providers (stdio or HTTP) and applies trust policy.
  `decision-gate-mcp/src/evidence.rs:133`
- Evidence records are stored in run state and included in gate eval logs (which
  are exported into runpacks).
  `decision-gate-core/src/core/state.rs:126`
  `decision-gate-core/src/runtime/runpack.rs:47`

Docs overview of the same flow and roles:
- `README.md:43`

## 2) What did the "provider builder system / data shape" idea mean?

In the current codebase, "data shape" is the provider contract for predicates:
- The provider contract defines predicate parameter schemas, result schemas,
  allowed comparators, determinism class, and examples.
  `decision-gate-contract/src/types.rs:231`
- External MCP providers must ship capability contracts and reference them via
  `capabilities_path` in config. This is the explicit, versioned shape.
  `Docs/guides/provider_development.md:90`
  `Docs/configuration/decision-gate.toml.md:92`

"Data source" is the provider implementation:
- Built-in providers read from environment variables, files, HTTP endpoints, or
  trigger time. They are registered in the provider registry by provider_id.
  `decision-gate-providers/src/registry.rs:124`
- External providers are separate MCP servers and are registered in
  `decision-gate.toml` as `type = "mcp"` with a command/url.
  `decision-gate-mcp/src/config.rs:632`

So for something like ESPN:
- Data shape: define predicates with param/result schemas in a capability
  contract JSON (the schema that your agent uses to compose predicates).
- Data source: implement the MCP provider that calls ESPN and returns evidence.

The scenario author works "backwards" from the API by defining provider
predicates that match the API's input/output. The ScenarioSpec then binds those
predicates to gates via EvidenceQuery.

## 3) How does the agent know the data shape? How does DG MCP validate it?

DG MCP uses a capability registry built at startup:
- The server loads provider contracts from built-ins or from
  `capabilities_path` in config, then compiles JSON Schema for params/results.
  `decision-gate-mcp/src/server.rs:134`
  `decision-gate-mcp/src/capabilities.rs:238`
- When a scenario is defined, DG MCP validates all predicate params/expected
  values against those compiled schemas, and checks allowed comparators.
  `decision-gate-mcp/src/tools.rs:366`
  `decision-gate-mcp/src/capabilities.rs:277`
- Evidence queries are also validated against predicate param schemas.
  `decision-gate-mcp/src/tools.rs:454`
  `decision-gate-mcp/src/capabilities.rs:318`

How the agent learns the shape today:
- There is no MCP tool for provider schema discovery. `tools/list` only returns
  core tool schemas (scenario_*, evidence_query, runpack_*).
  `decision-gate-mcp/src/tools.rs:126`
  `decision-gate-contract/src/tooling.rs:35`
- Agents are expected to consume provider contracts out-of-band (e.g.
  `Docs/generated/decision-gate/providers.json` for built-ins, and the external
  provider's contract JSON for custom providers).
  `Docs/guides/provider_development.md:90`

Runtime validation of "arbitrarily supplied schemas" is intentionally constrained:
- Schemas are loaded from local contract files, size-limited, and path-limited.
  `decision-gate-mcp/src/capabilities.rs:42`
- There is no runtime API to register arbitrary schemas on demand.

Also note: ScenarioSpec includes `schemas` (SchemaRef) for packet schemas, but
that is metadata only in core; there is no runtime schema lookup or validation.
`decision-gate-core/src/core/spec.rs:47`
`decision-gate-core/src/core/spec.rs:258`

## 4) Do we need a runtime registry for dynamic data shapes?

Current state: no dynamic registry. Provider contracts are loaded once at
startup from config, with explicit file size and path constraints. This means:
- The number of shapes is bounded by the server configuration and contract
  files, not by runtime user requests.
  `decision-gate-mcp/src/capabilities.rs:238`
  `decision-gate-mcp/src/config.rs:632`
- Provider access can be blocked by policy (allowlist/denylist), and missing
  providers cause scenario validation to fail.
  `decision-gate-providers/src/registry.rs:44`

If you want LLM-driven schema registration, it would require new tools and a
separate registry service (not implemented). The current system avoids runtime
registration to reduce unbounded memory, abuse, and schema sprawl.

## 5) Can agents query DG MCP for available schemas or shapes?

Not currently. The MCP tool surface only lists core tools and their generic
schemas; there is no tool to list providers or capabilities.
- Tool listing surface: `decision-gate-mcp/src/tools.rs:126`
- Canonical tool definitions (no provider discovery tool):
  `decision-gate-contract/src/tooling.rs:35`

The available shapes are discovered out-of-band:
- Built-ins: `Docs/generated/decision-gate/providers.json`
- External: the provider's `capabilities_path` JSON in config
  `Docs/guides/provider_development.md:90`

## 6) Who retrieves the data? How do we detect modification? Hashing and logs?

DG MCP retrieves evidence via providers; the agent supplies triggers or calls
(evidence_query/next/trigger), but evidence values are fetched by providers.
- EvidenceContext is built from trigger/run metadata and passed to providers.
  `decision-gate-core/src/runtime/engine.rs:506`
- Evidence is normalized with canonical hashes at evaluation time.
  `decision-gate-core/src/runtime/engine.rs:1324`
- Evidence records are stored in run state and exported into runpacks.
  `decision-gate-core/src/core/state.rs:126`
  `decision-gate-core/src/runtime/runpack.rs:47`

Agent-supplied artifacts are distinct:
- `scenario_submit` stores external artifacts and hashes their payloads but does
  not drive gate evaluation directly. It is an audit/disclosure mechanism.
  `decision-gate-core/src/runtime/engine.rs:323`

Modification detection mechanisms:
- EvidenceResult includes `evidence_hash` and optional `signature` or anchors
  to verify integrity or re-fetch evidence later.
  `decision-gate-core/src/core/evidence.rs:130`
- Provider trust policy can require signature verification.
  `decision-gate-mcp/src/evidence.rs:117`
  `Docs/guides/security_guide.md:20`
- `evidence_query` responses may redact raw values by policy while still
  returning hashes/anchors.
  `decision-gate-mcp/src/tools.rs:454`
  `Docs/guides/security_guide.md:29`

## 7) "LLM cannot lie", "LLM lied but we can prove it", "we have no idea"

DG can support all three, depending on how you wire providers and policy:

A) LLM cannot lie (strongest)
- DG retrieves evidence from trusted providers directly. If signatures are
  required, unsigned or tampered evidence is rejected. The agent never supplies
  the evidence value itself.
  `decision-gate-mcp/src/evidence.rs:117`
  `Docs/guides/security_guide.md:20`

B) LLM lied but we can prove what it submitted
- Use `scenario_submit` or a provider that simply reflects agent input. DG
  hashes and stores the payload; later you can show the hash (or the payload,
  if disclosed) as proof of what was submitted, but not that it was truthful.
  `decision-gate-core/src/runtime/engine.rs:323`

C) We have no idea if the LLM lied
- If evidence comes from an untrusted provider with audit-only policy, no
  signatures, no stable anchors, and no ability to re-fetch data, DG can only
  show what it observed. The threat model explicitly calls out evidence trust
  boundaries and redaction policies.
  `Docs/security/threat_model.md:20`

In all cases, Decision Gate still fails closed for missing/invalid evidence and
records hashes and logs for audit. The choice of provider trust policy, contract
quality, and data source determines which assurance level you get.
