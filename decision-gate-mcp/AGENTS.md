# AGENTS.md (decision-gate-mcp)

> **Audience:** Agents and automation working on Decision Gate MCP.
> **Goal:** Preserve a secure, deterministic MCP tool surface that is a thin
> wrapper over `decision-gate-core`.

---

## 0) TL;DR

- **No divergent logic:** MCP must call the core control plane.
- **Fail closed:** invalid inputs and auth failures must return errors.
- **Security first:** auth, allowlists, limits, and audit are non-negotiable.
- **Schema registry + precheck:** validate asserted payloads, no state mutation.
- **Docs/tooling sync:** update contract schemas after behavior changes.

---

## 1) In scope
- Tool routing, request validation, and error mapping.
- Authn/authz, rate limiting, inflight limits, and audit logging.
- Schema registry tool wiring and precheck behavior.
- Unit/system tests for all tool failure modes.

## 2) Out of scope (design approval required)
- Changing core decision semantics.
- Removing Zero Trust defaults.
- Implementing RBAC/ACL without design review.

## 3) Non-negotiables
- Strict input validation and deterministic responses.
- No fail-open behavior for auth or limits.
- Precheck must never mutate run state.

## 4) Testing
- Unit tests in `decision-gate-mcp/tests` for each tool and error path.
- System tests in `system-tests/` for end-to-end auth and rate limits.

## 5) References
- Docs/security/threat_model.md
- Docs/roadmap/trust_lanes_registry_plan.md
- decision-gate-core/AGENTS.md
