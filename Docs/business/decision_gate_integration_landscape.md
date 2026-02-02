# Decision Gate: Integration Landscape, Priorities, and Work Plan

## 1. Executive Summary

Decision Gate (DG) is a **deterministic, evidence-driven checkpoint engine** deployed
as a sidecar service. It evaluates gates, enforces trust lanes, and produces
audit-grade runpacks. It is not an agent runtime, not a task executor, and not
intended to be embedded as a general-purpose library (though the Rust crate can be
embedded in specialized native deployments).

External research identifies 40+ agentic AI frameworks, orchestration platforms,
and workflow-automation tools that could "integrate" with DG. Reading that list
naturally raises the question: **do we need to build 40 integrations?**

The answer is no. DG needs **five categories of investment**, and those five
categories unlock access to the entire landscape. Most of what the ecosystem
calls "integration" is not custom code per platform --- it is documentation,
thin SDKs, and the MCP protocol we already speak.

This document covers:

1. What DG currently exposes and what is missing.
2. The boundary between DG OSS and DG-E (enterprise), and how platforms become
   DG-E customers through the integration funnel.
3. The five investment areas and what each one unlocks.
4. The actual engineering work involved, including contract-driven SDK generation.
5. Priority sequencing and market-segment mapping.

---

## 2. Current Architectural Stance

### MCP Is a Protocol, Not a Special Server

A common source of confusion: "MCP server" sounds like a specialized piece of
infrastructure. It is not. DG's MCP server is a **regular HTTP server** (built
on Axum) that speaks a specific wire protocol (JSON-RPC 2.0 with MCP
conventions for tool discovery and invocation). Structurally:

```
┌──────────────────────────────────────────────────┐
│  Regular HTTP Server (Axum)                      │
│  Listens on a port, handles TLS, rate limits,    │
│  auth, request routing                           │
├──────────────────────────────────────────────────┤
│  MCP Protocol Layer                              │
│  JSON-RPC 2.0 over POST /rpc                     │
│  tools/list → discover 17 tools                  │
│  tools/call → invoke a tool by name with params  │
│  Standard request/response envelopes             │
└──────────────────────────────────────────────────┘
```

MCP is the **interface convention**, not the deployment model. The "sidecar"
framing means DG runs as a separate process alongside your agent. How that
process is reached depends on the transport:

- **stdio**: Agent spawns DG as a child process, communicates via stdin/stdout.
  No network involved. Truly local.
- **HTTP on localhost**: DG listens on `127.0.0.1:8080`. Agent calls it like
  any local web service.
- **HTTP on network**: DG _can_ listen on `0.0.0.0:8080` when deployed with a
  network-capable launcher. The OSS CLI allows non-loopback binds only with an
  explicit opt-in (`--allow-non-loopback` or `DECISION_GATE_ALLOW_NON_LOOPBACK=1`)
  and TLS (or upstream termination) + non-local auth configured; enterprise or custom launchers can
  enforce their own posture.

In all three cases, DG is a server. MCP is just the protocol it speaks.
Anything that can make an HTTP POST with a JSON body can talk to DG. The "MCP"
label means DG also supports the standardized tool-discovery handshake that
MCP-aware clients (like Claude Desktop) expect, but that is additive ---
non-MCP callers simply POST JSON-RPC directly.

### What We Have

| Capability                     | Details                                                                                                                                                                                                                                                                                                                                  |
| ------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **HTTP Server (MCP Protocol)** | Axum-based server exposing 17 tools via JSON-RPC 2.0. MCP conventions for tool discovery (`tools/list`) and invocation (`tools/call`). Any HTTP client can call it directly; MCP-aware clients get automatic tool discovery.                                                                                                             |
| **Three Transports**           | stdio (Content-Length framing, local only), HTTP (Axum, JSON-RPC POST `/rpc`, loopback-only by default in OSS CLI), SSE (streaming over HTTP, loopback-only by default in OSS CLI)                                                                                                                                                        |
| **Authentication**             | `local_only`, `bearer_token`, `mtls` modes; per-tool authorization allowlists                                                                                                                                                                                                                                                            |
| **Multi-Tenancy**              | Namespace isolation, default-namespace guard, pluggable `TenantAuthorizer`, optional AssetCore namespace authority                                                                                                                                                                                                                       |
| **RBAC / ACL**                 | Schema registry access control (builtin role-based or custom rule-based), principal resolution from auth context                                                                                                                                                                                                                         |
| **Audit & Observability**      | JSON-lines audit sinks for auth and MCP events, per-tool metrics (latency/outcome), correlation IDs                                                                                                                                                                                                                                      |
| **Usage Metering**             | Pluggable `UsageMeter` trait (quota enforcement hook)                                                                                                                                                                                                                                                                                    |
| **Provider SDK Templates**     | TypeScript, Python, Go --- for building **evidence sources** that DG queries                                                                                                                                                                                                                                                             |
| **Client SDKs**                | Generated Python + TypeScript client SDKs in `sdks/` (JSON-RPC transport, generated tool methods/types, field-level docs, optional runtime validation helpers)                                                                                                                                                                            |
| **Framework Adapters**         | In-repo Python adapters for LangChain, CrewAI, AutoGen, and OpenAI Agents SDK under `adapters/` (thin wrappers around the client SDK; not published yet)                                                                                                                                                                                |
| **Adapter Test Harness**       | Opt-in adapter validation via `scripts/adapter_tests.sh` (spawns local server + runs framework examples in an isolated venv)                                                                                                                                                                                                           |
| **Agentic Flow Harness**       | Canonical scenario library + deterministic multi-projection harness (`system-tests/tests/suites/agentic_harness.rs`, registry + packs under `system-tests/tests/fixtures/agentic/`, mirrored to `examples/agentic/`, entrypoint `scripts/agentic_harness.sh`)                                                                                                                                  |
| **Built-in Providers**         | `time`, `env`, `json`, `http`                                                                                                                                                                                                                                                                                                            |
| **CLI**                        | `serve`, `runpack export/verify`, `authoring validate/normalize`, `config validate`, `interop eval`                                                                                                                                                                                                                                      |
| **Runpacks**                   | Deterministic artifact bundles with canonical hashing (RFC 8785 / JCS), manifest integrity, offline verification                                                                                                                                                                                                                         |
| **Run State Stores**           | In-memory (ephemeral) and SQLite (WAL by default, durable; configurable)                                                                                                                                                                                                                                                                 |
| **Runpack Storage**            | Filesystem and object-store (S3-compatible) backends                                                                                                                                                                                                                                                                                     |
| **Configuration**              | Comprehensive `decision-gate.toml` covering server, auth, providers, trust policy, evidence disclosure, schema registry, validation, rate limits                                                                                                                                                                                         |
| **Examples**                   | 6 runnable Rust examples plus Python + TypeScript SDK examples under `examples/` (basic lifecycle, agent loop, CI gate, precheck), backed by system tests.                                                                                                                                                                                  |
| **Documentation**              | Architecture docs (auth, namespaces, trust anchors, comparators, providers, runpacks, scenario state, system tests), guides (getting started, provider development, protocol, conditions, integration patterns, security, evidence flow, RET logic), generated artifacts (tool schemas, provider contracts, tooltips, example scenarios) |
| **OpenAPI View**               | Generated OpenAPI document for the JSON-RPC `tools/call` surface (not a REST facade) at `Docs/generated/openapi/decision-gate.json`                                                                                                                                                                                                      |

### What We Do NOT Have

| Gap                                        | Impact                                                                                                                                                                                                                       |
| ------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **No published client SDKs**              | SDKs exist in-repo, but nothing is published to PyPI/npm yet; developers must vendor or install locally.                                                                                                             |
| **No published adapters**                 | Framework adapters exist in-repo but are not published to PyPI yet. Users must install from source for now.                                                                                                                    |
| **No REST facade**                         | JSON-RPC is the protocol. There is no REST translation layer (by design).                                                                                                                                             |
| **No marketplace presence**                | Nothing on PyPI, npm, crates.io (as a client), Zapier App Directory, or any package registry where developers discover tools.                                                                                                |
| **No OpenAI-plugin manifest**              | Semantic Kernel and similar platforms can auto-import REST services that publish an OpenAI-style plugin manifest. DG cannot be consumed this way today.                                                                      |
| **No live-mode agentic harness**           | Deterministic agentic harness exists, but live-mode (real LLMs + allowlisted network + transcripts) is not implemented yet.                                                                                                  |

### The Layer Model

Understanding how the pieces relate is essential for prioritizing work:

```
┌─────────────────────────────────────────────────────┐
│  Documentation & Examples                           │  ← "How to use DG with X"
├─────────────────────────────────────────────────────┤
│  Framework Adapters (LangChain Tool, CrewAI plugin) │  ← Thin wrappers
├─────────────────────────────────────────────────────┤
│  Client SDKs (Python, TypeScript)                   │  ← Generated from tooling.json
├─────────────────────────────────────────────────────┤
│  HTTP Server + MCP Protocol (JSON-RPC 2.0)          │  ← 17 tools, 3 transports
├─────────────────────────────────────────────────────┤
│  decision-gate-core (ControlPlane)                  │  ← Deterministic engine
└─────────────────────────────────────────────────────┘
```

Each layer wraps the one below it. **Nothing above the MCP server reimplements
logic.** Every layer is a thin projection of the same core contract.

### Integration Proof Surface: Agentic Flow Harness

For ecosystem-scale credibility, DG must demonstrate integration **across all
projections** with the same canonical scenarios. The agentic flow harness is
the proof surface: a deterministic, registry-driven scenario library executed
through raw MCP, SDKs, and framework adapters. It turns "we support X" into
"X passes the same audited scenarios as everything else" and is therefore a
core part of the integration strategy.

---

## 3. OSS vs. DG-E: Where the Line Is and Why It Matters for Integration

Understanding the boundary between DG OSS and DG-E (enterprise) is essential
for understanding the integration strategy. The integration work described in
this document (SDKs, adapters, docs) applies to both --- the protocol and
contract are identical. What differs is the operational envelope.

### The Boundary: Operational Scope, Not Features

DG OSS is not a crippled version of DG-E. It is a complete, production-capable
checkpoint engine. The line between them is not "has feature X" vs. "doesn't."
It is about **who operates it and at what organizational scale**.

| Concern           | DG OSS                                               | DG-E / DG Cloud                                                         |
| ----------------- | ---------------------------------------------------- | ----------------------------------------------------------------------- |
| **Who runs it**   | You deploy and operate it yourself                   | We run it (DG Cloud) or you run with enterprise operational tooling     |
| **Auth**          | Bearer tokens, mTLS (you manage keys)                | SSO (Okta, Azure AD), org accounts, API key management UI               |
| **Multi-tenancy** | Namespace isolation (config-driven, single operator) | Org-level isolation, per-tenant billing, tenant provisioning UI         |
| **Storage**       | SQLite (single node, durable)                        | Managed Postgres/S3, automated backups, disaster recovery, multi-region |
| **Availability**  | Single process (you restart it if it dies)           | HA deployment, health checks, auto-scaling, SLO guarantees              |
| **Compliance**    | Runpacks + audit logs (you archive and manage them)  | SOC2 artifacts, FIPS builds, immutable logs, SIEM export, legal hold    |
| **Rate limiting** | Per-IP (basic)                                       | Per-tenant, per-org, quota-aware, billing-integrated                    |
| **Monitoring**    | Stderr metrics (you scrape them)                     | Dashboards, alerting, observability integrations                        |
| **Support**       | GitHub issues, community                             | SLA-backed support, security advisories, LTS releases                   |

**Analogy:** PostgreSQL vs. Amazon RDS. PostgreSQL is fully open source and
production-capable. RDS is the same engine wrapped in managed operations,
backups, monitoring, scaling, and compliance. You don't need RDS to run
PostgreSQL in production --- but organizations with compliance requirements and
operations teams prefer it.

DG OSS answers: _"Can I embed this in my system?"_
DG-E answers: _"Can my company standardize on this and audit it?"_

### The Revenue Model: Platforms Are Customers, Not Passthrough

The integration strategy has a direct revenue implication. When a platform
company (CrewAI, Dust, etc.) integrates DG into their product, the integration
itself uses the OSS SDK and protocol. But the platform company becomes a **DG-E
customer** --- not just a conduit to their end-users.

Why: if CrewAI sells agent workflows to enterprise customers (banks, healthcare,
etc.), those customers expect compliance, uptime, audit trails, and SLAs.
CrewAI cannot deliver those guarantees while running an unmanaged OSS binary
internally. CrewAI itself needs the enterprise operational envelope.

The revenue model is therefore:

| Customer Type                                                                | What They Use    | Why They Pay                                                                                                                                                                                                      |
| ---------------------------------------------------------------------------- | ---------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Individual developer** building an agent                                   | DG OSS (free)    | They don't. OSS is ecosystem gravity and adoption funnel.                                                                                                                                                         |
| **Platform/SaaS company** embedding DG in their product (CrewAI, Dust, etc.) | DG-E or DG Cloud | They run DG as multi-tenant infrastructure for their paying customers. They need HA, tenant isolation, compliance artifacts, SLAs. They cannot sell enterprise agent workflows backed by an unmanaged OSS binary. |
| **Enterprise** using DG directly (not through a platform)                    | DG-E or DG Cloud | Their internal agents need governance. Same compliance/ops needs, without the platform middleman.                                                                                                                 |

The platform row is the key insight. Platforms don't tell their customers "go
buy DG-E separately." Platforms need DG-E _themselves_ because they are
operating DG on behalf of many customers. The forms this could take:

- **DG Cloud (hosted by us):** Platform's backend calls our hosted DG endpoint.
  We handle ops. They pay per usage or per tenant. Lowest friction.
- **DG-E self-hosted:** Platform runs DG-E in their own infrastructure. They
  get the enterprise binary with HA, multi-tenant features, compliance. They pay
  a license. More control, more ops burden on their side.
- **OEM / embedded:** Platform licenses DG-E to bundle inside their product.
  Their customers never see DG directly --- it's "CrewAI's governance engine."
  Premium pricing, deeper partnership.

### The OSS-to-Enterprise Funnel

```
Developer tries DG OSS (free, local, prototype)
        │
        ▼
Developer's company builds product using DG
        │
        ▼
Product goes to production → needs enterprise capabilities
        │
        ├── Direct enterprise: company buys DG-E / DG Cloud
        │
        └── Platform embedding: platform company buys DG-E / DG Cloud
            to run DG as infrastructure for THEIR customers
```

### What This Means for Integration Priorities

This revenue model affects how we prioritize integration work:

1. **The Python/TypeScript SDKs** are the top of the funnel. Developers discover
   DG through the SDK, prototype with OSS, and then their organizations or
   platforms upgrade to DG-E. The SDK is both a developer tool and a sales
   pipeline.

2. **Framework adapters** (LangChain, CrewAI) serve a dual purpose: they drive
   developer adoption _and_ they put DG in front of platform companies who are
   potential DG-E customers. An adapter for CrewAI is not just developer
   convenience --- it's a business development channel.

3. **Platform companies that embed DG are higher-value, stickier customers**
   than individual enterprises, because DG becomes load-bearing infrastructure
   in their product. They cannot rip it out without rebuilding their governance
   story. Prioritizing integrations that reach platform companies (CrewAI, Dust,
   Emergent) has disproportionate revenue potential.

4. **The integration code itself is always OSS.** SDKs, adapters, and examples
   are Apache 2.0. The revenue comes from the operational envelope (DG-E / DG
   Cloud), not from the integration layer. This is intentional: making
   integration free maximizes adoption, which maximizes the pool of potential
   DG-E customers.

---

## 4. The Five Investment Areas

### Area 1 --- Client SDKs (Python + TypeScript)

**What it is.**
Idiomatic client libraries that wrap DG's MCP JSON-RPC calls in language-native
interfaces. A Python developer would write:

```python
from decision_gate import DecisionGateClient

dg = DecisionGateClient(endpoint="http://127.0.0.1:8080/rpc")
run = dg.scenario_start({"run_config": {...}, "scenario_id": "...", "started_at": {...}, "issue_entry_packets": True})
status = dg.scenario_status({"tenant_id": 1, "namespace_id": 1, "run_id": "run-001"})
```

instead of hand-crafting JSON-RPC payloads.

**What it unlocks.**
Every Python and TypeScript agent framework simultaneously. The SDK is the
foundation for all framework adapters, all non-Rust examples, and all
marketplace packages. Without it, each developer must reverse-engineer the
JSON-RPC protocol from our MCP tool schemas.

**Market reach.**
Python and TypeScript together cover approximately 90% of the agentic AI
ecosystem. Python dominates agent frameworks (LangChain, AutoGPT, CrewAI,
AutoGen, BabyAGI, MetaGPT, Camel, HuggingFace Agents, Marvin). TypeScript
covers workflow platforms (Flowise, Pipedream), browser-based tools (AgentGPT),
and Node.js orchestrators.

**Contract-driven generation (invariance doctrine).**

SDKs must not be hand-authored in isolation. Following the invariance doctrine
(`Asset-Core/Docs/standards/invariance_doctrine.md`), every external projection
--- including client SDKs --- must be derived from the canonical contract. DG
already has the upstream half of this pipeline:

```
decision-gate-core types (Rust)
        │
        ▼
decision-gate-contract crate
   tooling.rs    ← 17 tool contracts (name, input schema, output schema, examples)
   schemas.rs    ← JSON Schemas for ScenarioSpec, config, etc.
   tooltips.rs   ← IDE hover text
   examples.rs   ← canonical example payloads
   providers.rs  ← provider capability contracts
        │
        ▼
decision-gate-contract CLI
   `generate` → Docs/generated/decision-gate/tooling.json
                 Docs/generated/decision-gate/schemas/*
                 Docs/generated/decision-gate/examples/*
                 ...
   `check`   → verifies on-disk artifacts match (drift detection)
```

**Final arrow (implemented): `tooling.json` → SDK codegen.**

```
tooling.json  (canonical, machine-generated from Rust)
     │
     ▼  decision-gate-sdk-gen (Rust)
     │
     ├──► sdks/python/decision_gate/_generated.py
     │       - One method per tool (typed params, typed return)
     │       - Schema constants + validation helpers for runtime validation
     │       - Machine-generated, do-not-edit
     │
     ├──► sdks/typescript/src/_generated.ts
     │       - One method per tool (typed params, typed return)
     │       - Schema constants + validation helpers for runtime validation
     │       - Machine-generated, do-not-edit
     │
     └──► Docs/generated/openapi/decision-gate.json
             - OpenAPI view of JSON-RPC tools/call surface
```

**What is generated vs. hand-written:**

| Layer                                                                                            | Source                                                         | Changes when tools change?         |
| ------------------------------------------------------------------------------------------------ | -------------------------------------------------------------- | ---------------------------------- |
| Tool method signatures, parameter types, return types                                            | Generated from `tooling.json`                                  | Yes --- automatically, via codegen |
| HTTP transport plumbing (connect, send JSON-RPC, parse response, error handling, async patterns) | Hand-written (~100--150 lines per language)                    | No --- stable across tool changes  |
| Re-exports and package metadata                                                                  | Hand-written (`__init__.py`, `pyproject.toml`, `package.json`) | No                                 |

**Drift prevention:** `decision-gate-sdk-gen check` and the generator tests
assert that the committed SDK outputs match `tooling.json`. Drift is therefore
caught by CI via `cargo test -p decision-gate-sdk-gen`.

**Local CI entrypoints:** `scripts/generate_all.sh` regenerates artifacts (or
`--check` to verify). `scripts/verify_all.sh` runs the generation checks plus
`cargo test --workspace --exclude system-tests`, with optional system test
selection via `--system-tests[=p0|p1|quick|all]`, plus optional packaging
verification via `--package-dry-run[=python|typescript|all]`. Standalone package
verification is available via `scripts/package_dry_run.sh`. Adapter smoke
testing (external deps) is opt-in via `scripts/adapter_tests.sh` or
`scripts/verify_all.sh --adapter-tests[=langchain,crewai,autogen,openai_agents]`.

**Actual deliverables.**

| Deliverable                  | Scope                                                                                                                                        |
| ---------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| **Codegen script**           | `decision-gate-sdk-gen` (Rust) reads `tooling.json`, emits `_generated.py`, `_generated.ts`, and OpenAPI.                                     |
| `sdks/python/decision_gate/` | Hand-written: `client.py` (transport), `errors.py`. Generated: `_generated.py` (methods + types). Package: `pyproject.toml`.                 |
| `sdks/typescript/src/`       | Hand-written: `client.ts` (transport), `errors.ts`. Generated: `_generated.ts` (methods + types). Package: `package.json`.                   |
| **CI integration**           | `decision-gate-sdk-gen check` + generator tests enforce drift detection.                                                                      |
| **Local CI scripts**         | `scripts/generate_all.sh` (regen/check), `scripts/verify_all.sh` (checks + tests, optional system tests + packaging + adapters), `scripts/package_dry_run.sh` (package verification), `scripts/adapter_tests.sh` (adapter smoke tests). |
| Tests                        | Generator drift tests in Rust plus SDK system tests (Python + TypeScript) against a live MCP server.                                          |
| Packaging                    | `pyproject.toml` + `package.json` + packaging dry-run (`scripts/package_dry_run.sh`). Publishing to PyPI/npm remains pending.                 |

**Dependency:** None. This is the first thing to build.

---

### Area 2 --- MCP Protocol Native (Already Done)

**What it is.**
DG is an MCP server. Any tool that speaks the Model Context Protocol can connect
to DG immediately --- point it at the binary (stdio) or the HTTP endpoint, and
all 17 tools are available.

**What it unlocks.**

- Claude Desktop (via MCP server config).
- Any agent framework that adopts MCP as a standard protocol.
- Future MCP-native tooling as the protocol gains adoption.

**Market reach.**
MCP is an emerging standard, growing rapidly within the Claude/Anthropic
ecosystem and being adopted by other tool vendors. As MCP adoption grows, DG's
integration surface grows automatically with zero additional work.

**Actual work involved.**
None. This is already done. The investment here is **awareness**: ensuring our
docs, README, and marketing make clear that DG is MCP-native and that MCP
clients can connect immediately.

The one thing worth maintaining is **MCP spec compliance** as the protocol
evolves. Track MCP spec changes and update our server accordingly.

---

### Area 3 --- Framework Adapters (Targeted, 3--4 Maximum)

**What it is.**
Platform-specific wrappers that make DG feel "native" within a specific
framework's ecosystem. For example, the LangChain adapter provides tool
builders that LangChain agents can use directly:

```python
from decision_gate_langchain import build_decision_gate_tools
from decision_gate import DecisionGateClient

client = DecisionGateClient(endpoint="http://127.0.0.1:8080/rpc")
tools = build_decision_gate_tools(client)
agent = AgentExecutor(tools=tools, ...)
```

**What it unlocks.**

- Discoverability within the framework's package ecosystem (e.g., `pip install
decision-gate-langchain` shows up when developers search for LangChain tools).
- Reduced friction: developers don't need to write the glue themselves.
- "Blessed" integration status: framework communities list known integrations,
  and being on that list drives adoption.

**Market reach.**
Per-framework. LangChain alone is used by an estimated 70%+ of LLM application
builders. CrewAI targets enterprise multi-agent orchestration. OpenAI Agents SDK
is provider-agnostic and growing.

**Actual work involved per adapter.**

Each adapter is small --- typically a single module with one or two classes or
factory functions:

| Adapter               | What to Build                                                                                                                                                 | Publishes As                          |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------- |
| **LangChain**         | Tool builders wrapping `scenario_status` / `scenario_next` / `precheck` calls via the Python SDK. Optional: `DecisionGateCallback` for automatic logging.     | `decision-gate-langchain` on PyPI     |
| **CrewAI**            | A CrewAI-compatible tool that calls DG before high-impact agent actions.                                                                                      | `decision-gate-crewai` on PyPI        |
| **AutoGen**           | AutoGen `FunctionTool` wrappers that call DG tools via the Python SDK.                                                                                        | `decision-gate-autogen` on PyPI       |
| **OpenAI Agents SDK** | Function tools (and optional guardrails) that invoke DG for policy checks.                                                                                    | `decision-gate-openai-agents` on PyPI |

**Current status:** LangChain, CrewAI, AutoGen, and OpenAI Agents SDK adapters
exist in-repo under `adapters/` and are usable locally. Publishing remains
deferred.

**Dependency:** Each adapter depends on the Python client SDK (Area 1).

**Selection criteria for which adapters to build:**

- Framework has significant active user base.
- Framework's audience overlaps with DG's value proposition (governance,
  compliance, enterprise trust).
- Framework has a clear plugin/tool/extension API.
- Framework team or community is receptive (potential co-marketing).

**What NOT to do:** Do not build adapters for every framework listed in external
research. Most frameworks can use DG via the client SDK directly. Adapters are
a convenience and discoverability play, not a technical necessity.

---

### Area 4 --- Documentation and Non-Rust Examples

**What it is.**
Code examples, integration guides, and tutorials showing how to use DG from
Python and TypeScript with popular frameworks. This is not the same as building
framework adapters --- it is showing developers how to call DG, with or without
a formal adapter.

**What it unlocks.**

- Developer trust: "I can see how this works with my stack before I commit."
- Adoption velocity: copy-paste starting points reduce time-to-first-gate.
- SEO and discoverability: "Decision Gate LangChain integration" becomes a
  findable page.

**Market reach.**
Multiplier on all other investment areas. Without docs and examples, SDKs and
adapters sit unused.

**Actual work involved.**

| Deliverable             | Scope                                                                                                                                                                                                                                                  |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Python examples**     | 50--100 line scripts showing: (1) basic scenario lifecycle via SDK, (2) agent loop with DG gates, (3) CI/CD gating, (4) precheck with asserted evidence. Place in `examples/python/`.                                                                  |
| **TypeScript examples** | Equivalent scripts for Node.js. Place in `examples/typescript/`.                                                                                                                                                                                       |
| **Adapter examples**    | Framework-specific scripts showing how to construct LangChain/CrewAI/AutoGen/OpenAI Agents tools. Place in `examples/frameworks/`.                                                                                                                     |
| **Integration guides**  | Markdown docs in `Docs/guides/` or `Docs/integrations/`: "Using DG with LangChain", "Using DG with Auto-GPT", "Using DG with CrewAI", "Using DG in CI/CD pipelines", "Using DG with Zapier (webhook approach)". Each is 1--3 pages with code snippets. |
| **README updates**      | Add a "Quick Start" section showing Python/TypeScript usage alongside the existing Rust examples.                                                                                                                                                      |

**Dependency:** Requires Area 1 (client SDKs) to exist, since examples use them.

---

### Area 5 --- Marketplace and Distribution Presence

**What it is.**
Publishing packages to registries and app directories where developers discover
tools.

**What it unlocks.**

- `pip install decision-gate` and `npm install decision-gate` as the entry point.
- Visibility to developers who search PyPI/npm for "decision gate", "AI
  governance", "agent guardrails", etc.
- Access to low-code and business-user segments via platform marketplaces
  (Zapier, Flowise).

**Market reach.**
Broad but indirect. Package registries are where developers start. Marketplace
apps (Zapier, Flowise) reach the non-developer and low-code audience.

**Actual work involved.**

| Deliverable                         | Scope                                                                                                                                                                                                          |
| ----------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **PyPI publishing**                 | Publish `decision-gate` (client SDK) and any adapter packages. Requires CI pipeline for automated releases.                                                                                                    |
| **npm publishing**                  | Publish `decision-gate` TypeScript client. Same CI pipeline concern.                                                                                                                                           |
| **Zapier app** (optional, Phase 2+) | Build a Zapier app with actions like "Start Scenario", "Check Gate Status", "Submit Evidence". Requires following Zapier's app submission process and maintaining a Zapier developer account. Moderate effort. |
| **Flowise custom node** (optional)  | A Flowise node wrapping DG calls. Small effort if the TypeScript SDK exists.                                                                                                                                   |

**Dependency:** Requires Area 1 (SDKs). Marketplace apps also benefit from
Area 3 (adapters) and Area 4 (docs).

---

## 5. What You Do NOT Need to Build

The external research mentions many integration patterns that sound expensive
but are either unnecessary or premature for DG:

| Tempting Idea                                                   | Why You Don't Need It                                                                                                                                                                                                                     |
| --------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **A `/integrations` folder with custom code for 40 frameworks** | 90% of frameworks can use DG through the client SDK or raw HTTP. Custom adapters are only needed for 2--3 high-priority frameworks.                                                                                                       |
| **gRPC adapter**                                                | JSON-RPC over HTTP already serves the same purpose. Add gRPC only if a specific high-value customer or partner requires it. No framework in the agentic AI space demands gRPC.                                                            |
| **WASM / FFI bridges**                                          | DG is a sidecar service, not an embedded library. Python/JS developers call it over HTTP. FFI is for rare embedded scenarios and is not needed for the agent ecosystem. DG's Rust crate already serves native embedders.                  |
| **Custom code for each workflow-automation platform**           | Zapier, Pipedream, Make.com, Tray.io, and similar platforms all have generic HTTP/webhook steps. Developers can call DG's HTTP endpoint directly. A formal Zapier app is a nice-to-have for discoverability, not a technical requirement. |
| **OpenAI plugin manifest**                                      | Useful for Semantic Kernel auto-import, but the protocol is evolving and adoption is uncertain. Defer unless a specific customer needs it.                                                                                                |
| **REST facade over JSON-RPC**                                   | JSON-RPC is DG's protocol. Adding a REST translation layer creates a second API surface to maintain. Developers using the client SDK never see the wire protocol anyway.                                                                  |

---

## 6. Priority Sequencing

The five areas form a dependency chain:

```
                    ┌──────────────────────────┐
                    │ Area 5: Marketplace /     │
                    │ Distribution Presence     │
                    └──────────┬───────────────┘
                               │ depends on
              ┌────────────────┼────────────────┐
              │                │                │
   ┌──────────▼──────┐  ┌─────▼──────────┐     │
   │ Area 3: Framework│  │ Area 4: Docs & │     │
   │ Adapters (3-4)   │  │ Examples       │     │
   └──────────┬───────┘  └─────┬──────────┘     │
              │                │                │
              └────────┬───────┘                │
                       │ depends on             │
              ┌────────▼────────┐               │
              │ Area 1: Client  │◄──────────────┘
              │ SDKs (Py + TS)  │
              └────────┬────────┘
                       │ built on
              ┌────────▼────────┐
              │ Area 2: MCP     │
              │ (Already Done)  │
              └─────────────────┘
```

**Recommended build order:**

| Step  | What                                        | Rationale                                                                                                                     |
| ----- | ------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| **1** | **Python client SDK**                       | Unlocks the Python ecosystem (LangChain, AutoGPT, CrewAI, AutoGen --- the largest segment). Everything above depends on this. |
| **2** | **TypeScript client SDK**                   | Unlocks Node.js ecosystem (Flowise, Pipedream, browser-based tools). Can be built in parallel with Step 1.                    |
| **3** | **Python + TypeScript examples**            | Show developers how to use the SDKs. Quick wins: basic scenario lifecycle, agent loop, CI gate.                               |
| **4** | **Integration guides** (docs)               | "Using DG with LangChain", "Using DG with CrewAI", etc. Prose + code snippets referencing the examples.                       |
| **5** | **Framework adapters** (LangChain, CrewAI, AutoGen, OpenAI Agents SDK) | Native feel in the dominant frameworks. Proves the pattern for future adapters.                                              |
| **6** | **PyPI + npm publishing**                   | Make SDKs and adapters installable via package managers.                                                                      |
| **7** | **Additional adapters** (as demand appears) | AutoGPT, LlamaIndex tooling hooks, etc. Only build when user demand or partnership signals justify the work.                  |
| **8** | **Marketplace apps** (Zapier, Flowise)      | Low-code audience. Defer until core developer adoption is established.                                                        |

---

## 7. Market Segment Mapping

Each market segment maps to specific investment areas. The **Revenue Path**
column reflects the platform-as-customer model described in Section 3.

| Market Segment                   | Example Platforms                                                                  | Required Areas             | Revenue Path                                                                                                        | Notes                                                                                                           |
| -------------------------------- | ---------------------------------------------------------------------------------- | -------------------------- | ------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| **Agent Frameworks (OSS)**       | LangChain, AutoGPT, AutoGen, BabyAGI, SuperAGI, MetaGPT, Camel, HuggingFace Agents | Areas 1 + 3 + 4            | Adoption funnel: developers try DG OSS, companies upgrade to DG-E                                                   | Largest segment. Python-dominant. Client SDK + adapters + examples.                                             |
| **Agent Platforms (Commercial)** | CrewAI, Dust, Emergent, Cognosys, Fixie, AgentGPT                                  | Areas 1 + 3 + partnerships | **Platform is the DG-E customer.** They embed DG in their product and need enterprise ops for their paying clients. | Highest-value segment. Adapters + SDK are the entry point, but the sale is DG-E / DG Cloud.                     |
| **LLM Orchestration**            | LlamaIndex, Semantic Kernel, Haystack, Marvin, ZenML                               | Areas 1 + 4                | Adoption funnel                                                                                                     | SDK + examples sufficient. No formal adapters needed unless demand appears.                                     |
| **Workflow Automation**          | Zapier, Pipedream, Flowise, Make.com, Tray.io                                      | Areas 1 + 2 + 5            | Adoption funnel; platform partnership if they embed DG                                                              | HTTP steps already exist in these platforms. SDK for developers; marketplace apps for discoverability.          |
| **CI/CD and DevOps**             | GitHub Actions, GitLab CI, Jenkins, Argo                                           | Areas 1 + 4                | Direct enterprise: companies using DG in their pipelines upgrade to DG-E                                            | CLI already works. SDK + examples showing pipeline integration.                                                 |
| **RPA / Enterprise Automation**  | UiPath, Automation Anywhere, Power Automate                                        | Area 2 (MCP/HTTP)          | Platform partnership                                                                                                | These platforms have HTTP action steps. No custom code needed from DG. Partnership path if demand materializes. |
| **Native / Embedded**            | Custom Rust orchestrators, C++ systems                                             | Already supported          | Direct enterprise                                                                                                   | `decision-gate-core` Rust crate is available.                                                                   |

**Key takeaways:**

1. The **Python client SDK** (Area 1) is required by every segment except
   native/embedded and MCP-native consumers. It is the single highest-leverage
   investment.

2. **Commercial agent platforms** (CrewAI, Dust, Emergent, etc.) are both
   integration targets _and_ prospective DG-E customers. They embed DG into
   products they sell to enterprises, which means they need the enterprise
   operational envelope for themselves. Integrations that reach these platforms
   have disproportionate revenue potential.

---

## 8. Relationship to the Existing Server

As clarified in Section 2, DG's "MCP server" is a regular Axum HTTP server that
speaks JSON-RPC 2.0 with MCP tool-discovery conventions. This is the
**foundation** for all integration, but it is not a complete integration story.
Here is how the layers relate:

| Layer                                      | What It Does                                                                                                                        | Who Uses It                                                                               |
| ------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| `decision-gate-core` (ControlPlane)        | Deterministic evaluation engine. All gate logic, RET evaluation, runpack generation, trust lane enforcement.                        | Rust embedders via crate dependency.                                                      |
| `decision-gate-mcp` (HTTP Server)          | Axum server with JSON-RPC 2.0 / MCP protocol. Thin wrappers around the ControlPlane. Adds auth, audit, transport. Exposes 17 tools. | MCP clients (Claude Desktop, any MCP-aware tool). Any HTTP client that can POST JSON-RPC. |
| **Client SDKs** (implemented)             | Idiomatic Python/TypeScript wrappers around the JSON-RPC calls. Contract-generated from `tooling.json`.                             | Python and TypeScript developers. All framework adapters.                                 |
| **Framework Adapters** (implemented)       | Platform-specific wrappers (LangChain, CrewAI, AutoGen, OpenAI Agents SDK) around the client SDK.                                   | Developers already using a specific framework who want DG to feel native.                 |
| **Documentation + Examples** (mostly done) | Code samples and guides showing usage patterns (SDK + adapter examples).                                                            | Everyone evaluating or adopting DG.                                                       |

**The HTTP server is necessary but not sufficient.** It handles the protocol.
The remaining missing layers (publishing + integration guides) handle developer
experience. And as described in Section 3, the OSS layers (SDK, adapters, docs)
feed the DG-E revenue funnel: developers prototype with OSS, then their
organizations or platforms upgrade to enterprise.

---

## 9. What "Integration" Actually Means

To ground the terminology:

**For DG, "integration with Platform X" means one of these, in order of
increasing effort:**

1. **Platform X speaks MCP** --- DG works automatically. No code to write.
   (Example: Claude Desktop.)

2. **Platform X can make HTTP calls** --- A developer calls DG's JSON-RPC
   endpoint from Platform X using an HTTP step or code block. No DG-side code
   needed; we provide a documentation page with a code snippet. (Example:
   Zapier webhook, Pipedream HTTP step, any CI/CD pipeline.)

3. **Platform X is a Python/TypeScript framework** --- A developer imports DG's
   client SDK and calls it from their code. DG provides the SDK package.
   (Example: Auto-GPT, HuggingFace Agents, LlamaIndex, Haystack.)

4. **Platform X has a plugin/tool system** --- DG provides a small adapter
   package that wraps the client SDK in the platform's expected interface. DG
   provides the adapter package. (Example: LangChain Tool, CrewAI integration.)

5. **Platform X wants DG embedded in their product** --- A partnership where
   Platform X builds the UI integration using DG's API. DG provides
   documentation, engineering support, and possibly a co-marketing relationship.
   (Example: Dust, Emergent, CrewAI Enterprise.)

**Most platforms fall into categories 2 or 3.** Only a few justify category 4.
Category 5 is a business relationship, not an engineering project.

---

## 10. Provider SDK vs. Client SDK --- Clarifying the Distinction

This is a frequent source of confusion because DG already ships "SDKs."

|                        | Provider SDK (exists)                                         | Client SDK (exists)                                             |
| ---------------------- | ------------------------------------------------------------- | --------------------------------------------------------------- |
| **Purpose**            | Build evidence sources that DG queries                        | Call DG's tools from external code                              |
| **Direction**          | DG calls the provider                                         | External code calls DG                                          |
| **Languages**          | TypeScript, Python, Go templates                              | Python, TypeScript packages                                     |
| **What you implement** | `evidence_query` tool                                         | Nothing --- you consume pre-built methods                       |
| **Example**            | A custom provider that checks a database and returns evidence | A Python script that calls `scenario_start` and `scenario_next` |
| **Location**           | `decision-gate-provider-sdk/`                                 | `sdks/python/`, `sdks/typescript/`                              |

Both are important. The provider SDK lets the ecosystem extend DG's evidence
sources. The client SDK lets the ecosystem invoke DG's gating logic. The
provider SDK exists; the client SDK now exists in-repo.

---

## 11. Summary

DG's integration story reduces to five investment areas, not forty platform
builds:

| #   | Area                                 | Status                     | Unlocks                                                                                                                                    |
| --- | ------------------------------------ | -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| 1   | **Client SDKs** (Python, TypeScript) | Done (in-repo, unpublished) | Every non-Rust consumer. Foundation for all other areas. Top of DG-E sales funnel.                                                         |
| 2   | **MCP Protocol**                     | Done                       | Claude Desktop, MCP-native tools, future MCP adopters.                                                                                     |
| 3   | **Framework Adapters** (3--4 max)    | Done (in-repo, unpublished) | Native feel in LangChain, CrewAI, AutoGen, OpenAI Agents SDK. Also a business development channel to platform companies (potential DG-E customers). |
| 4   | **Documentation + Examples**         | Mostly done (Rust + SDK + adapter examples) | Developer trust, adoption velocity, SEO.                                                                                                   |
| 5   | **Marketplace Presence**             | Not started                | Discoverability via PyPI, npm, Zapier.                                                                                                     |

The client SDKs are the critical path. Everything else either already exists
(MCP) or depends on the SDKs being available first.

Three structural principles govern this work:

1. **MCP is a protocol, not a deployment model.** DG is a regular HTTP server
   that speaks JSON-RPC with MCP conventions. Anything that can make an HTTP
   POST can talk to DG.

2. **SDKs are contract-generated projections, not hand-authored code.** The
   invariance doctrine applies: `decision-gate-contract` generates
   `tooling.json`, and `decision-gate-sdk-gen` derives SDK source from that
   artifact. Drift is enforced by generator checks in CI.

3. **Integration code is always OSS; revenue comes from operations.** SDKs,
   adapters, and examples are Apache 2.0. The revenue comes from DG-E / DG
   Cloud --- the enterprise operational envelope that platform companies and
   enterprises need when running DG in production at organizational scale.
   Platforms that embed DG become DG-E customers themselves.
