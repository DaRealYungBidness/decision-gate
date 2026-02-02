<!--
Docs/guides/docs_corpus_swapping.md
============================================================================
Document: Docs Corpus Swapping Guide
Description: How to replace or extend the documentation corpus used by MCP
             docs search and resources.
Purpose: Provide a practical, step-by-step guide for operators and integrators.
Dependencies:
  - Docs/configuration/decision-gate.toml.md
  - Docs/architecture/decision_gate_docs_search_architecture.md
  - decision-gate-mcp/src/docs.rs
============================================================================
-->

# Docs Corpus Swapping Guide

## At a Glance

**What:** Replace or extend the docs corpus used by `decision_gate_docs_search`
and MCP `resources/list` + `resources/read`.

**Why:** Tailor the LLM runtime guidance to your environment, policies, or
internal documentation.

**Where:** `decision-gate.toml` under `[docs]`.

---

## How the Docs Corpus Is Built

1. **Embedded defaults** (compile-time, no network I/O).
2. **Optional extra docs** loaded from `docs.extra_paths` (files or directories).

Docs are loaded once at server startup and stored in an in-memory catalog.

---

## Step 1: Prepare Your Docs

Requirements:
- Markdown (`.md`) files only.
- Each doc should include a `# Title` heading.
- Use `##` / `###` headings for searchable sections.
- Keep files under the configured `docs.max_doc_bytes` limit.

Notes:
- File names become doc IDs (sanitized to lowercase + underscores).
- Custom docs are given the `pattern` role by default.
- Empty files are skipped with warnings.

---

## Step 2: Update decision-gate.toml

**Replace the default corpus with a custom directory:**

```toml
[docs]
enabled = true
enable_search = true
enable_resources = true
include_default_docs = false
extra_paths = ["./my-docs"]
max_doc_bytes = 262144
max_total_bytes = 1048576
max_docs = 32
max_sections = 10
```

**Extend the default corpus with a few extra files:**

```toml
[docs]
enabled = true
enable_search = true
enable_resources = true
include_default_docs = true
extra_paths = ["./overrides/llm_playbook.md", "./runbooks"]
```

Behavior to expect:
- Missing paths fail startup with a config error.
- Oversized docs are skipped with warnings.
- Total size / count limits are enforced.

---

## Step 3: Ensure Tool Visibility (Optional)

Docs search is a tool. If you filter tools, make sure it is visible:

```toml
[server.tools]
mode = "filter"
allowlist = ["decision_gate_docs_search", "scenario_define", "scenario_start"]
denylist = []
```

If `docs.enabled = false` or `docs.enable_search = false`, the tool is hidden
and calls return `UnknownTool`.

---

## Step 4: Restart the Server

Docs are loaded at startup only. Restart to pick up new content.
Warnings about skipped docs are printed to stderr.

---

## Step 5: Validate the Corpus

**Search (tools/call):**

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "decision_gate_docs_search",
    "arguments": { "query": "precheck vs live", "max_sections": 3 }
  }
}
```

**Resources list (resources/list):**

```json
{ "jsonrpc": "2.0", "id": 2, "method": "resources/list" }
```

**Resources read (resources/read):**

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "resources/read",
  "params": { "uri": "decision-gate://docs/custom/my_doc" }
}
```

---

## Troubleshooting

- **Search returns no results:** empty query returns an overview; otherwise
  confirm headings exist and the corpus is loaded.
- **Server fails on startup:** a path in `docs.extra_paths` is missing.
- **Tool missing from tools/list:** check `[docs]` toggles and `[server.tools]`.
- **Resource read fails:** ensure the URI matches `resources/list`.

