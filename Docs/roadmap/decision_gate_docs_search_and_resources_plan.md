<!--
Docs/roadmap/decision_gate_docs_search_and_resources_plan.md
============================================================================
Document: Decision Gate MCP Docs Search + Resources Plan
Description: World-class, deterministic docs search and resources surface for
             Decision Gate OSS MCP.
Purpose: Define what an LLM needs at runtime, how to expose it, and how to
         toggle/hide tools with zero ambiguity.
Dependencies:
  - Docs/standards/codebase_formatting_standards.md
  - Docs/security/threat_model.md
  - decision-gate-mcp/README.md
  - decision-gate-contract/src/tooling.rs
  - Docs/generated/decision-gate/tooling.md
  - Docs/guides/*.md
============================================================================
-->

# Decision Gate MCP Docs Search + Resources Plan

## Executive Intent

Decision Gate OSS should ship a **first-class, deterministic docs surface**
for LLMs that goes beyond tool schemas. The goal is to give an LLM everything
it needs to make correct runtime decisions (precheck vs live, trust lanes,
comparators, provider contracts) while remaining **offline, deterministic,
and fail-closed**.

This plan specifies:
- The default documentation corpus and why each document matters.
- A deterministic docs search tool and MCP resources surface.
- Configuration toggles (enable/disable) and **tool hiding** semantics.
- A full implementation + test plan an agent can execute.

This is a **pre-launch** redesign: backward compatibility is not required.

---

## Current State (OSS)

- `decision-gate-mcp` exposes tool calls + `tools/list` only.
- There is **no** docs search tool and **no** MCP resources surface.
- Tool metadata is sourced from `decision-gate-contract` artifacts
  (`Docs/generated/decision-gate/tooling.md`, `tooling.json`).
- The repo already contains LLM-relevant guides, but they are not surfaced to
  MCP agents.

---

## Goals

1. **Runtime clarity for LLMs**
   - Provide high-signal documentation that directly affects tool usage and
     correctness.

2. **Deterministic and offline**
   - Embed default docs at compile time; no network I/O.

3. **Toggleable and hideable**
   - Docs search and resources can be disabled.
   - Tools can be hidden from `tools/list` and made uncallable.

4. **User-extensible**
   - Allow users to add their own docs with minimal friction.

---

## Default Documentation Corpus (LLM Runtime Set)

This is the **minimal, high-signal** set that materially improves tool use.
Each document is referenced by full path for deterministic embedding.

### 1) Core Runtime Model

| Document | Path | Why it must be searchable |
| --- | --- | --- |
| Evidence Flow + Execution Model | `Docs/guides/evidence_flow_and_execution_model.md` | Clarifies precheck vs live runs, trust lanes, evaluation order, and why evidence is not execution. |
| Security Guide | `Docs/guides/security_guide.md` | Explains trust, disclosure, auth defaults, and fail-closed behavior. |

### 2) Tool Usage + Scenario Authoring

| Document | Path | Why it must be searchable |
| --- | --- | --- |
| Tooling Summary | `Docs/generated/decision-gate/tooling.md` | Descriptions and usage notes beyond JSON schemas. |
| Authoring Formats | `Docs/generated/decision-gate/authoring.md` | Canonical JSON, hashing, and normalization semantics. |
| Condition Authoring | `Docs/guides/condition_authoring.md` | Comparator semantics and tri-state rules (critical for correct gates). |
| RET Logic | `Docs/guides/ret_logic.md` | Explains gate logic algebra used at runtime. |
| LLM-Native Playbook | `Docs/guides/llm_native_playbook.md` | LLM-first workflows and correct MCP tool sequences. |

### 3) Evidence + Providers

| Document | Path | Why it must be searchable |
| --- | --- | --- |
| Built-in Providers | `Docs/generated/decision-gate/providers.md` | Check schemas and expected evidence formats. |
| Provider Protocol | `Docs/guides/provider_protocol.md` | `evidence_query` contract and MCP assumptions. |
| Provider Schema Authoring | `Docs/guides/provider_schema_authoring.md` | How schemas are shaped for providers. |
| Provider Development | `Docs/guides/provider_development.md` | Common mistakes and correct provider wiring. |
| JSON Evidence Playbook | `Docs/guides/json_evidence_playbook.md` | JSONPath behavior and file-based evidence semantics. |

### 4) Optional (Enable by Default if Runpack Tools are Exposed)

| Document | Path | Why it matters |
| --- | --- | --- |
| Runpack Architecture | `Docs/architecture/decision_gate_runpack_architecture.md` | Runpack export/verify semantics and integrity guarantees. |

---

## Documentation Roles (For Search Weighting)

Assign each doc a role to bias results toward the most relevant guidance:

- **Reasoning**: conceptual, mental model, and invariants.
  - `evidence_flow_and_execution_model.md`
  - `security_guide.md`
- **Decision**: tool selection and flow guidance.
  - `tooling.md`
  - `llm_native_playbook.md`
- **Ontology**: schemas, providers, comparator semantics.
  - `providers.md`
  - `condition_authoring.md`
  - `authoring.md`
- **Pattern**: recipes, operational guidance.
  - `json_evidence_playbook.md`
  - `provider_development.md`
  - `provider_protocol.md`

This role taxonomy mirrors the AssetCore approach and improves relevance
without adding nondeterministic ranking.

---

## MCP Surface (New)

### A) Docs Search Tool

**Tool name (recommended):** `decision_gate_docs_search`

**Input:**
- `query` (string, required)
- `max_sections` (integer, optional; default 3; hard cap 10)

**Output:**
- `sections[]`: ranked, role-tagged doc sections
- `docs_covered[]`: which docs the results came from
- `suggested_followups[]`: role-aware refinements

**Behavior:**
- Section boundaries are `##` / `###` headings.
- Heading matches score higher than body matches.
- Role-aware bonus scoring (deterministic).
- Empty query returns an overview across roles (one section per role).

### B) MCP Resources

Add MCP standard resources:
- `resources/list`
- `resources/read`

Resources expose **full-text documents** by URI (for preloading context).
These must reuse the exact same embedded docs as the search index.

---

## Configuration + Toggling (Best Version)

Add a new config section in `decision-gate-config`:

```toml
[docs]
enabled = true
enable_search = true
enable_resources = true
include_default_docs = true
extra_paths = []
max_doc_bytes = 262144
max_total_bytes = 1048576
max_docs = 32
max_sections = 10
```

**Semantics:**
- `enabled = false` disables everything (search + resources) and removes tools.
- `enable_search = false` disables only the search tool.
- `enable_resources = false` disables only resources/list + resources/read.
- `include_default_docs = false` allows a fully user-defined corpus.
- `extra_paths` accepts files and directories. Directories are scanned for
  `.md` files. Paths outside the repo are allowed but must be explicitly
  configured.
- All size and count limits are **fail-closed**. Oversized docs are skipped
  with warnings.

---

## Tool Hiding and Uncallability (All Tools)

**Recommendation:** yes, now is the right time to make **all tools hideable
and uncallable**, and to separate **visibility** from **authorization**,
because:

- It is a direct extension of the docs-toggle pattern.
- It eliminates agent confusion (if a tool is disabled, it must not appear).
- It avoids leaking sensitive or environment-specific capabilities.
- It aligns with pre-launch flexibility (no compatibility burden).
- It preserves clear separation of concerns: auth answers "who can call",
  visibility answers "what exists".
- It makes audits and incident response simpler (one surface for visibility,
  one surface for auth).

### World-Class Mechanism (Visibility Separate From Auth)

Add a dedicated visibility config under `server.tools`, and keep
`server.auth.allowed_tools` for authentication and authorization only.
Do not overload `server.auth.allowed_tools` to change `tools/list` output.

```toml
[server.tools]
mode = "filter" # enum: filter | passthrough
allowlist = []
denylist = []
```

```toml
[server.auth]
allowed_tools = [] # auth allowlist; does not change tools/list
```

**Rules:**
- `mode = "filter"` means tools are filtered from `tools/list`.
- `mode = "passthrough"` means `tools/list` returns full registry (use only
  if a legacy client expects to see all tools).
- `allowlist` defines the explicit visible set when non-empty.
- `denylist` always removes tools (even if allowed).

**Uncallability:**
- Tool calls for filtered tools must return a **generic unknown/unauthorized**
  error. Do not reveal that the tool exists.

This replaces the current behavior where `allowed_tools` affects only auth
but not `tools/list` output, while keeping the auth policy explicit and
auditable.

---

## AssetCore Reference Implementation (Copy Map)

These are the **exact** AssetCore files that implement the mature version of
this system. Use them as the reference for structure, ordering, and tests.

### Core Logic

| AssetCore file path | Role to copy |
| --- | --- |
| `assetcore-adapters/src/mcp/docs.rs` | Embedded docs registry + deterministic search. |
| `assetcore-adapters/src/mcp/resources.rs` | MCP resources list/read backed by embedded docs. |
| `assetcore-adapters/src/executor/mod.rs` | Docs search tool execution path (handled pre-namespace). |
| `assetcore-adapters/src/mcp/tools.rs` | Tool definition + schema wiring for docs search. |
| `assetcore-adapters/src/descriptions/memory_framing.rs` | Canonical docs search description text. |
| `assetcore-adapters/README.md` | Docs resources + search explanation and examples. |

### Tests

| AssetCore file path | Purpose |
| --- | --- |
| `assetcore-adapters/src/tests/mcp_docs.rs` | Validates docs registry + section search behavior. |
| `assetcore-adapters/src/tests/mcp_docs_search.rs` | Role-aware search tests and determinism. |
| `assetcore-adapters/src/tests/mcp_resources.rs` | Ensures resources mirror search docs. |
| `assetcore-adapters/src/tests/mcp_server.rs` | End-to-end docs search through MCP server. |
| `assetcore-adapters/src/tests/mcp_tools.rs` | Tool naming/description/schema parity. |

---

## Decision Gate Implementation Plan

### 1) Add Docs Registry + Search

- New file: `decision-gate-mcp/src/docs.rs`
- Port logic from `assetcore-adapters/src/mcp/docs.rs`.
- Replace AssetCore doc set with the **Decision Gate corpus** listed above.
- Keep deterministic scoring (heading-first + role bonus).

### 2) Add MCP Resources

- New file: `decision-gate-mcp/src/resources.rs`
- Port logic from `assetcore-adapters/src/mcp/resources.rs`.
- Resource URIs should be stable and namespaced, e.g.:
  - `decision-gate://docs/evidence-flow`
  - `decision-gate://docs/tooling`

### 3) Add Docs Search Tool to Contract

- Update `decision-gate-contract/src/tooling.rs` with a new tool definition.
- Regenerate artifacts:
  - `Docs/generated/decision-gate/tooling.json`
  - `Docs/generated/decision-gate/tooling.md`
  - `Docs/generated/decision-gate/tooltips.json`

### 4) Wire Tool Execution

- Update `decision-gate-mcp/src/tools.rs` to handle `decision_gate_docs_search`.
- The docs search should be processed **before** namespace/auth if disabled.
- If docs search disabled, return a generic tool error (do not leak availability).

### 5) Add Resource Routing to MCP Server

- Update `decision-gate-mcp/src/server.rs` to handle:
  - `resources/list`
  - `resources/read`
- Ensure resource calls are gated by docs config.

### 6) Add Config and Validation

- Add `[docs]` config section in `decision-gate-config`.
- Update schema/docs generation and validation tests.
- Extend `Docs/configuration/decision-gate.toml.md`.

### 7) Tool Hiding (Global)

- Add `[server.tools]` config in `decision-gate-config`.
- Update tool list to filter results when enabled.
- Update auth enforcement to deny filtered tools.
- Ensure `tools/list` and `tools/call` share the same filtering logic.

---

## Testing Plan (Exhaustive)

### Unit Tests (decision-gate-mcp)

- Docs registry loads all default docs and roles.
- Docs search:
  - heading match ranking
  - body match ranking
  - deterministic ordering
  - max_sections enforcement
  - empty query returns overview across roles
- Resources:
  - list returns all docs
  - read returns exact doc body
  - resource bodies match docs search registry
- Docs toggles:
  - disabled search tool returns error
  - disabled resources methods return error

### Tool List Filtering Tests

- `tools/list` respects `[server.tools.allowed_tools]`.
- `tools/list` hides docs search when disabled.
- `tools/call` for hidden tool returns generic error.

### Contract Tests (decision-gate-contract)

- Tooling schema includes `decision_gate_docs_search` with correct input/output.
- Generated docs are deterministic.

### System Tests

- HTTP MCP:
  - `tools/list` hides disabled tools
  - `decision_gate_docs_search` returns expected sections
- SSE MCP:
  - `resources/list` and `resources/read` behave identically to HTTP

---

## Threat Model Updates (Required When Implemented)

This change introduces new input surface:

- Search queries (untrusted input)
- Optional local docs ingestion (`extra_paths`)
- New MCP methods (`resources/list`, `resources/read`)

Update `Docs/security/threat_model.md` with:
- Input classification for doc search and resources.
- Path traversal and resource size controls.
- Failure modes and rate limiting assumptions.

---

## Acceptance Criteria

1. Docs search and resources behave deterministically with no runtime I/O.
2. Default docs are embedded and cover the runtime corpus above.
3. Docs and resources can be disabled independently.
4. Hidden tools do not appear in `tools/list` and cannot be called.
5. Tests cover all failure modes and determinism.

---

## Appendices

### A) Default Resource URI Plan

- `decision-gate://docs/evidence-flow`
- `decision-gate://docs/security`
- `decision-gate://docs/tooling`
- `decision-gate://docs/authoring`
- `decision-gate://docs/conditions`
- `decision-gate://docs/ret-logic`
- `decision-gate://docs/llm-playbook`
- `decision-gate://docs/providers`
- `decision-gate://docs/provider-protocol`
- `decision-gate://docs/provider-schema`
- `decision-gate://docs/provider-development`
- `decision-gate://docs/json-evidence`
- `decision-gate://docs/runpack-architecture` (if runpack tools enabled)

### B) Minimal JSON Schema for Docs Search Tool

```json
{
  "name": "decision_gate_docs_search",
  "description": "Search Decision Gate documentation for runtime guidance.",
  "input_schema": {
    "type": "object",
    "required": ["query"],
    "properties": {
      "query": { "type": "string" },
      "max_sections": { "type": "integer", "minimum": 1, "maximum": 10 }
    }
  },
  "output_schema": {
    "type": "object",
    "properties": {
      "sections": { "type": "array" },
      "docs_covered": { "type": "array" },
      "suggested_followups": { "type": "array" }
    }
  }
}
```
