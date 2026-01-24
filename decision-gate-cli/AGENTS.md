# AGENTS.md (decision-gate-cli)

> **Audience:** Agents and automation working on CLI commands.
> **Goal:** Provide safe operational entry points without altering core semantics.

---

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
- decision-gate-mcp/README.md
- decision-gate-core/AGENTS.md
