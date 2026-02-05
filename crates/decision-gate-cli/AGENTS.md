# AGENTS.md (decision-gate-cli)

> **Audience:** Agents and automation working on CLI commands.
> **Goal:** Provide safe operational entry points without altering core semantics.

---

## Standards (Read First)
Before making changes, read and follow:
- `Docs/standards/codebase_engineering_standards.md`
- `Docs/standards/codebase_formatting_standards.md`
- `Docs/standards/doc_formatting_standards.md`

## 0) TL;DR

- **CLI is a wrapper:** no business logic divergence from core.
- **Fail closed:** invalid configs or inputs must error.
- **Ergonomic output:** clear errors, no silent defaults.

---

## 1) In scope
- CLI argument parsing and validation.
- Wiring to MCP server and runpack utilities.
- User-facing error messages and docs.

## 2) Out of scope (design approval required)
- Changing core behavior or trust semantics.
- Adding undocumented flags or hidden defaults.

## 3) Non-negotiables
- Deterministic outputs for identical inputs.
- Strict config validation before execution.

## 4) Testing
```bash
cargo test -p decision-gate-cli
```

## 5) References
- Docs/standards/codebase_engineering_standards.md
- Docs/standards/codebase_formatting_standards.md
- Docs/standards/doc_formatting_standards.md
- crates/decision-gate-mcp/README.md
- crates/decision-gate-core/AGENTS.md
