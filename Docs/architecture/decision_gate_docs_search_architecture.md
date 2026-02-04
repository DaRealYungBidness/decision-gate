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
  - decision-gate-mcp/src/auth.rs
  - decision-gate-config/src/config.rs
  - decision-gate-contract/src/tooling.rs
  - Docs/configuration/decision-gate.toml.md
============================================================================
Last Updated: 2026-02-04 (UTC)
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
[F:decision-gate-mcp/src/docs.rs L9-L780](decision-gate-mcp/src/docs.rs#L9-L780)
[F:decision-gate-mcp/src/tools.rs L569-L717](decision-gate-mcp/src/tools.rs#L569-L717)
[F:decision-gate-mcp/src/server.rs L1190-L1365](decision-gate-mcp/src/server.rs#L1190-L1365)

---

## Component Map

**Docs Catalog + Search**
- [F:decision-gate-mcp/src/docs.rs L9-L950](decision-gate-mcp/src/docs.rs#L9-L950)
- Embedded default docs via `include_str!`.
  [F:decision-gate-mcp/src/docs.rs L58-L87](decision-gate-mcp/src/docs.rs#L58-L87)
- Optional extra docs via `docs.extra_paths`.
  [F:decision-gate-mcp/src/docs.rs L479-L515](decision-gate-mcp/src/docs.rs#L479-L515)
  [F:decision-gate-config/src/config.rs L1237-L1271](decision-gate-config/src/config.rs#L1237-L1271)
- Deterministic section extraction + ranking.
  [F:decision-gate-mcp/src/docs.rs L620-L780](decision-gate-mcp/src/docs.rs#L620-L780)

**Tool Routing**
- [F:decision-gate-mcp/src/tools.rs L569-L717](decision-gate-mcp/src/tools.rs#L569-L717)
- `decision_gate_docs_search` tool handler.
  [F:decision-gate-mcp/src/tools.rs L1423-L1433](decision-gate-mcp/src/tools.rs#L1423-L1433)
- Docs search gating tied to `docs.enabled` + `docs.enable_search`.
  [F:decision-gate-mcp/src/tools.rs L469-L487](decision-gate-mcp/src/tools.rs#L469-L487)
  [F:decision-gate-config/src/config.rs L1237-L1271](decision-gate-config/src/config.rs#L1237-L1271)

**Resources Surface**
- [F:decision-gate-mcp/src/server.rs L1190-L1365](decision-gate-mcp/src/server.rs#L1190-L1365)
- `resources/list` and `resources/read` mapped to the same catalog.
  [F:decision-gate-mcp/src/tools.rs L584-L615](decision-gate-mcp/src/tools.rs#L584-L615)
  [F:decision-gate-mcp/src/docs.rs L903-L949](decision-gate-mcp/src/docs.rs#L903-L949)

**Configuration**
- [F:decision-gate-config/src/config.rs L1237-L1311](decision-gate-config/src/config.rs#L1237-L1311)
- `[docs]` config (enable/disable search/resources, corpus selection, limits).
  [F:Docs/configuration/decision-gate.toml.md L481-L497](Docs/configuration/decision-gate.toml.md#L481-L497)

**Contract**
- [F:decision-gate-contract/src/tooling.rs L339-L353](decision-gate-contract/src/tooling.rs#L339-L353)
- `decision_gate_docs_search` input/output schema.
  [F:decision-gate-contract/src/tooling.rs L1725-L1780](decision-gate-contract/src/tooling.rs#L1725-L1780)

---

## Startup Data Flow

1. **Load config**
   - `DecisionGateConfig` provides `[docs]` settings used during MCP server
     initialization.
     [F:decision-gate-config/src/config.rs L1237-L1311](decision-gate-config/src/config.rs#L1237-L1311)
     [F:decision-gate-mcp/src/server.rs L281-L312](decision-gate-mcp/src/server.rs#L281-L312)

2. **Build docs catalog**
   - `DocsCatalog::from_config` loads embedded docs when
     `docs.include_default_docs = true`.
     [F:decision-gate-mcp/src/docs.rs L232-L285](decision-gate-mcp/src/docs.rs#L232-L285)
   - `docs.extra_paths` adds local Markdown files or directories (recursive).
     [F:decision-gate-mcp/src/docs.rs L479-L539](decision-gate-mcp/src/docs.rs#L479-L539)
   - Missing paths fail fast with a config error.
     [F:decision-gate-mcp/src/docs.rs L491-L499](decision-gate-mcp/src/docs.rs#L491-L499)
   - Oversized docs or empty files are skipped with warnings.
     [F:decision-gate-mcp/src/docs.rs L269-L279](decision-gate-mcp/src/docs.rs#L269-L279)
     [F:decision-gate-mcp/src/docs.rs L543-L589](decision-gate-mcp/src/docs.rs#L543-L589)

3. **Emit warnings**
   - Catalog warnings are written to stderr at startup.
     [F:decision-gate-mcp/src/server.rs L1849-L1853](decision-gate-mcp/src/server.rs#L1849-L1853)

4. **Attach to tool router**
   - The router receives the docs catalog and docs config for runtime routing.
     [F:decision-gate-mcp/src/tools.rs L519-L561](decision-gate-mcp/src/tools.rs#L519-L561)

---

## Runtime Request Flow

**tools/list**
- Tool visibility is filtered by `[server.tools]`.
  [F:decision-gate-mcp/src/tools.rs L376-L447](decision-gate-mcp/src/tools.rs#L376-L447)
- `decision_gate_docs_search` is omitted if:
  - `docs.enabled = false`, or
  - `docs.enable_search = false`, or
  - it is hidden by `[server.tools]`.
  [F:decision-gate-mcp/src/tools.rs L617-L630](decision-gate-mcp/src/tools.rs#L617-L630)
  [F:decision-gate-config/src/config.rs L1237-L1271](decision-gate-config/src/config.rs#L1237-L1271)

**tools/call (decision_gate_docs_search)**
- Routed through `ToolRouter::handle_docs_search`.
  [F:decision-gate-mcp/src/tools.rs L659-L717](decision-gate-mcp/src/tools.rs#L659-L717)
  [F:decision-gate-mcp/src/tools.rs L1423-L1433](decision-gate-mcp/src/tools.rs#L1423-L1433)
- If disabled, returns `UnknownTool` (same behavior as hidden tools).
  [F:decision-gate-mcp/src/tools.rs L469-L487](decision-gate-mcp/src/tools.rs#L469-L487)
  [F:decision-gate-mcp/src/tools.rs L670-L674](decision-gate-mcp/src/tools.rs#L670-L674)
- If enabled, returns ranked sections from the catalog.
  [F:decision-gate-mcp/src/docs.rs L319-L673](decision-gate-mcp/src/docs.rs#L319-L673)

**resources/list + resources/read**
- Routed through `decision-gate-mcp/src/server.rs`.
  [F:decision-gate-mcp/src/server.rs L1202-L1214](decision-gate-mcp/src/server.rs#L1202-L1214)
- If disabled (`docs.enabled = false` or `docs.enable_resources = false`),
  returns `method not found` at the JSON-RPC layer.
  [F:decision-gate-mcp/src/server.rs L1202-L1258](decision-gate-mcp/src/server.rs#L1202-L1258)
- If enabled but the docs provider denies access, returns `UnknownTool`.
  [F:decision-gate-mcp/src/tools.rs L584-L615](decision-gate-mcp/src/tools.rs#L584-L615)
- Uses the same catalog entries as docs search.
  [F:decision-gate-mcp/src/tools.rs L584-L615](decision-gate-mcp/src/tools.rs#L584-L615)
  [F:decision-gate-mcp/src/docs.rs L903-L949](decision-gate-mcp/src/docs.rs#L903-L949)

---

## Search Semantics

Search is deterministic and bounded:

- Sections are derived from Markdown `##` / `###` headings.
  [F:decision-gate-mcp/src/docs.rs L723-L747](decision-gate-mcp/src/docs.rs#L723-L747)
- Heading matches rank higher than body matches.
  [F:decision-gate-mcp/src/docs.rs L766-L780](decision-gate-mcp/src/docs.rs#L766-L780)
- Results include `sections`, `docs_covered`, and `suggested_followups`.
  [F:decision-gate-mcp/src/docs.rs L184-L193](decision-gate-mcp/src/docs.rs#L184-L193)
  [F:decision-gate-mcp/src/docs.rs L666-L673](decision-gate-mcp/src/docs.rs#L666-L673)
- Empty query returns an overview across roles.
  [F:decision-gate-mcp/src/docs.rs L676-L714](decision-gate-mcp/src/docs.rs#L676-L714)
- `max_sections` is clamped to the configured limit and hard cap (10).
  [F:decision-gate-mcp/src/docs.rs L49-L52](decision-gate-mcp/src/docs.rs#L49-L52)
  [F:decision-gate-mcp/src/docs.rs L319-L324](decision-gate-mcp/src/docs.rs#L319-L324)
- Stable ordering is used for tie-breaking.
  [F:decision-gate-mcp/src/docs.rs L643-L648](decision-gate-mcp/src/docs.rs#L643-L648)

---

## Resources Surface

Docs resources reuse the same embedded catalog:

- `resources/list` returns metadata for each document.
  [F:decision-gate-mcp/src/docs.rs L903-L939](decision-gate-mcp/src/docs.rs#L903-L939)
  [F:decision-gate-mcp/src/server.rs L1299-L1333](decision-gate-mcp/src/server.rs#L1299-L1333)
- `resources/read` returns full Markdown content for a specific URI.
  [F:decision-gate-mcp/src/docs.rs L941-L949](decision-gate-mcp/src/docs.rs#L941-L949)
  [F:decision-gate-mcp/src/server.rs L1336-L1365](decision-gate-mcp/src/server.rs#L1336-L1365)
- Default resources use `decision-gate://docs/<id>` URIs.
  [F:decision-gate-mcp/src/docs.rs L56-L57](decision-gate-mcp/src/docs.rs#L56-L57)
  [F:decision-gate-mcp/src/docs.rs L352-L457](decision-gate-mcp/src/docs.rs#L352-L457)
- Extra docs use `decision-gate://docs/custom/<id>`.
  [F:decision-gate-mcp/src/docs.rs L583-L589](decision-gate-mcp/src/docs.rs#L583-L589)
- Unknown URIs return `InvalidParams`.
  [F:decision-gate-mcp/src/tools.rs L501-L516](decision-gate-mcp/src/tools.rs#L501-L516)

---

## Configuration + Gating

**Docs config**
- `[docs]` controls enablement and corpus selection.
  [F:decision-gate-config/src/config.rs L1237-L1271](decision-gate-config/src/config.rs#L1237-L1271)
- `include_default_docs = false` allows a fully custom corpus.
  [F:decision-gate-mcp/src/docs.rs L243-L245](decision-gate-mcp/src/docs.rs#L243-L245)
- `extra_paths` accepts files or directories (recursive `.md` scan).
  [F:decision-gate-mcp/src/docs.rs L479-L539](decision-gate-mcp/src/docs.rs#L479-L539)

**Tool visibility**
- `[server.tools]` controls which tools appear in `tools/list`.
  [F:decision-gate-config/src/config.rs L822-L870](decision-gate-config/src/config.rs#L822-L870)
  [F:decision-gate-mcp/src/tools.rs L574-L581](decision-gate-mcp/src/tools.rs#L574-L581)
- Hidden tools return `UnknownTool` when called.
  [F:decision-gate-mcp/src/tools.rs L632-L674](decision-gate-mcp/src/tools.rs#L632-L674)
- Tool visibility is distinct from auth (`server.auth.allowed_tools`).
  [F:decision-gate-config/src/config.rs L802-L820](decision-gate-config/src/config.rs#L802-L820)
  [F:decision-gate-mcp/src/auth.rs L344-L363](decision-gate-mcp/src/auth.rs#L344-L363)

See [F:Docs/configuration/decision-gate.toml.md L481-L497](Docs/configuration/decision-gate.toml.md#L481-L497) for full details.

---

## Security + Limits

- No runtime network I/O; catalog is local-only.
  [F:decision-gate-mcp/src/docs.rs L11-L16](decision-gate-mcp/src/docs.rs#L11-L16)
  [F:decision-gate-mcp/src/docs.rs L479-L589](decision-gate-mcp/src/docs.rs#L479-L589)
- Size limits are enforced per document and for total corpus bytes.
  [F:decision-gate-mcp/src/docs.rs L246-L279](decision-gate-mcp/src/docs.rs#L246-L279)
  [F:decision-gate-config/src/config.rs L1259-L1270](decision-gate-config/src/config.rs#L1259-L1270)
- Missing extra paths cause startup failure.
  [F:decision-gate-mcp/src/docs.rs L491-L499](decision-gate-mcp/src/docs.rs#L491-L499)
- Oversized or empty files are skipped with warnings.
  [F:decision-gate-mcp/src/docs.rs L269-L279](decision-gate-mcp/src/docs.rs#L269-L279)
  [F:decision-gate-mcp/src/docs.rs L543-L589](decision-gate-mcp/src/docs.rs#L543-L589)
- Resources + docs search can be disabled independently.
  [F:decision-gate-config/src/config.rs L1237-L1252](decision-gate-config/src/config.rs#L1237-L1252)
  [F:decision-gate-mcp/src/tools.rs L469-L487](decision-gate-mcp/src/tools.rs#L469-L487)

---

## File-by-File Cross Reference

- Catalog + search: [F:decision-gate-mcp/src/docs.rs L9-L950](decision-gate-mcp/src/docs.rs#L9-L950)
- Tool routing: [F:decision-gate-mcp/src/tools.rs L569-L717](decision-gate-mcp/src/tools.rs#L569-L717)
- Resources routing: [F:decision-gate-mcp/src/server.rs L1190-L1365](decision-gate-mcp/src/server.rs#L1190-L1365)
- Config schema: [F:decision-gate-config/src/config.rs L1237-L1311](decision-gate-config/src/config.rs#L1237-L1311)
- Tool contract: [F:decision-gate-contract/src/tooling.rs L339-L353](decision-gate-contract/src/tooling.rs#L339-L353)
- Config docs: [F:Docs/configuration/decision-gate.toml.md L481-L497](Docs/configuration/decision-gate.toml.md#L481-L497)
