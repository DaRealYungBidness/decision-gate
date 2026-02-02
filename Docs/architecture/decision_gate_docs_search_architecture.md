<!--
Docs/architecture/decision_gate_docs_search_architecture.md
============================================================================
Document: Decision Gate Docs Search + Resources Architecture
Description: Current-state reference for the embedded docs catalog, search tool,
             and MCP resources surface.
Purpose: Explain how docs are embedded, searched, and served, plus how the
         configuration gates visibility and access.
Dependencies:
  - decision-gate-mcp/src/docs.rs
  - decision-gate-mcp/src/tools.rs
  - decision-gate-mcp/src/server.rs
  - decision-gate-config/src/config.rs
  - decision-gate-contract/src/tooling.rs
  - Docs/configuration/decision-gate.toml.md
============================================================================
Last Updated: 2026-02-02 (UTC)
============================================================================
-->

# Decision Gate Docs Search + Resources Architecture

> **Audience:** Engineers maintaining the MCP docs catalog, search tool, and
> resources surface, plus operators customizing the docs corpus.

---

## Table of Contents

1. [Executive Overview](#executive-overview)
2. [Component Map](#component-map)
3. [Startup Data Flow](#startup-data-flow)
4. [Runtime Request Flow](#runtime-request-flow)
5. [Search Semantics](#search-semantics)
6. [Resources Surface](#resources-surface)
7. [Configuration + Gating](#configuration--gating)
8. [Security + Limits](#security--limits)
9. [File-by-File Cross Reference](#file-by-file-cross-reference)

---

## Executive Overview

Decision Gate exposes a deterministic documentation surface for MCP clients:
`decision_gate_docs_search` provides section-level search, and MCP
`resources/list` + `resources/read` provide full-text document access.
The docs catalog is embedded at compile time and can be extended (or replaced)
with local Markdown files via configuration. No runtime network I/O is used,
and all search ranking is deterministic.

---

## Component Map

**Docs Catalog + Search**
- `decision-gate-mcp/src/docs.rs`
- Embedded default docs via `include_str!`.
- Optional extra docs via `docs.extra_paths`.
- Deterministic section extraction + ranking.

**Tool Routing**
- `decision-gate-mcp/src/tools.rs`
- `decision_gate_docs_search` tool handler.
- Docs search gating tied to `docs.enabled` + `docs.enable_search`.

**Resources Surface**
- `decision-gate-mcp/src/server.rs`
- `resources/list` and `resources/read` mapped to the same catalog.

**Configuration**
- `decision-gate-config/src/config.rs`
- `[docs]` config (enable/disable search/resources, corpus selection, limits).

**Contract**
- `decision-gate-contract/src/tooling.rs`
- `decision_gate_docs_search` input/output schema.

---

## Startup Data Flow

1. **Load config**
   - `DecisionGateConfig` is parsed from `decision-gate.toml`.
   - `[docs]` config determines whether docs are enabled and which corpus to use.

2. **Build docs catalog**
   - `DocsCatalog::from_config` loads embedded docs when
     `docs.include_default_docs = true`.
   - `docs.extra_paths` adds local Markdown files or directories (recursive).
   - Missing paths fail fast with a config error.
   - Oversized docs or empty files are skipped with warnings.

3. **Emit warnings**
   - Catalog warnings are written to stderr at startup.

4. **Attach to tool router**
   - `ToolRouter` receives the catalog and docs config for runtime routing.

---

## Runtime Request Flow

**tools/list**
- Tool visibility is filtered by `[server.tools]`.
- `decision_gate_docs_search` is omitted if:
  - `docs.enabled = false`, or
  - `docs.enable_search = false`, or
  - it is hidden by `[server.tools]`.

**tools/call (decision_gate_docs_search)**
- Routed through `ToolRouter::handle_docs_search`.
- If disabled, returns `UnknownTool` (same behavior as hidden tools).
- If enabled, returns ranked sections from the catalog.

**resources/list + resources/read**
- Routed through `decision-gate-mcp/src/server.rs`.
- If disabled (`docs.enabled = false` or `docs.enable_resources = false`),
  returns `UnknownTool`.
- Uses the same catalog entries as docs search.

---

## Search Semantics

Search is deterministic and bounded:

- Sections are derived from Markdown `##` / `###` headings.
- Heading matches rank higher than body matches.
- Results include `sections`, `docs_covered`, and `suggested_followups`.
- Empty query returns an overview across roles.
- `max_sections` is clamped to the configured limit and hard cap (10).
- Stable ordering is used for tie-breaking.

---

## Resources Surface

Docs resources reuse the same embedded catalog:

- `resources/list` returns metadata for each document.
- `resources/read` returns full Markdown content for a specific URI.
- Default resources use `decision-gate://docs/<id>` URIs.
- Extra docs use `decision-gate://docs/custom/<id>`.
- Unknown URIs return `InvalidParams`.

---

## Configuration + Gating

**Docs config**
- `[docs]` controls enablement and corpus selection.
- `include_default_docs = false` allows a fully custom corpus.
- `extra_paths` accepts files or directories (recursive `.md` scan).

**Tool visibility**
- `[server.tools]` controls which tools appear in `tools/list`.
- Hidden tools return `UnknownTool` when called.
- Tool visibility is distinct from auth (`server.auth.allowed_tools`).

See `Docs/configuration/decision-gate.toml.md` for full details.

---

## Security + Limits

- No runtime network I/O; catalog is local-only.
- Size limits are enforced per document and for total corpus bytes.
- Missing extra paths cause startup failure.
- Oversized or empty files are skipped with warnings.
- Resources + docs search can be disabled independently.

---

## File-by-File Cross Reference

- Catalog + search: `decision-gate-mcp/src/docs.rs`
- Tool routing: `decision-gate-mcp/src/tools.rs`
- Resources routing: `decision-gate-mcp/src/server.rs`
- Config schema: `decision-gate-config/src/config.rs`
- Tool contract: `decision-gate-contract/src/tooling.rs`
- Config docs: `Docs/configuration/decision-gate.toml.md`

