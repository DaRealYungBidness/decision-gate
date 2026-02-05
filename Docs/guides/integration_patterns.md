<!--
Docs/guides/integration_patterns.md
============================================================================
Document: Integration Patterns
Description: Common deployment patterns for Decision Gate
Purpose: Show how to integrate DG into CI/CD, agents, and compliance workflows
Dependencies:
  - examples/
============================================================================
-->

# Integration Patterns

## At a Glance

**What:** Common deployment patterns for CI/CD, agent loops, and compliance workflows
**Why:** Choose the right integration strategy for trust and audit needs
**Who:** Architects and developers planning Decision Gate deployments
**Prerequisites:** [getting_started.md](getting_started.md)

---

## Pattern Overview

| Pattern | Trust Lane | Speed | Audit Trail | Use Case |
|---------|------------|-------|-------------|----------|
| **CI/CD Gate** | Verified | Slower | Full runpack | Production deployments |
| **Agent Loop** | Asserted -> Verified | Fast -> Slow | Optional -> Full | LLM planning toward gates |
| **Controlled Disclosure** | Verified | Slow | Full | Data release with audit |
| **Compliance Workflow** | Verified | Slow | Full | Multi-stage gates |
| **MCP Federation** | Verified | Varies | Full | External evidence sources |

---

## Pattern 1: CI/CD Gate

**When to use:** Automated quality gates in CI/CD pipelines

**Flow:**
1. Run tools -> emit JSON
2. `scenario_start` -> create run
3. `scenario_next` -> DG reads JSON via `json` provider
4. Decision outcome drives deploy

Configure the `json` provider with a root that points at your evidence
workspace, and keep file paths **relative** to that root.

**Scenario (minimal, accurate fields):**
```json dg-parse dg-level=fast
{
  "scenario_id": "ci-gate",
  "namespace_id": 1,
  "spec_version": "v1",
  "stages": [
    {
      "stage_id": "quality",
      "entry_packets": [],
      "gates": [
        {
          "gate_id": "quality-checks",
          "requirement": {
            "And": [
              { "Condition": "tests_ok" },
              { "Condition": "coverage_ok" },
              { "Condition": "scan_ok" }
            ]
          }
        }
      ],
      "advance_to": { "kind": "terminal" },
      "timeout": null,
      "on_timeout": "fail"
    }
  ],
  "conditions": [
    {
      "condition_id": "tests_ok",
      "query": {
        "provider_id": "json",
        "check_id": "path",
        "params": {
          "file": "test-results.json",
          "jsonpath": "$.summary.failed"
        }
      },
      "comparator": "equals",
      "expected": 0,
      "policy_tags": []
    },
    {
      "condition_id": "coverage_ok",
      "query": {
        "provider_id": "json",
        "check_id": "path",
        "params": {
          "file": "coverage.json",
          "jsonpath": "$.total.lines.percent"
        }
      },
      "comparator": "greater_than_or_equal",
      "expected": 85,
      "policy_tags": []
    },
    {
      "condition_id": "scan_ok",
      "query": {
        "provider_id": "json",
        "check_id": "path",
        "params": {
          "file": "scan.json",
          "jsonpath": "$.summary.critical"
        }
      },
      "comparator": "equals",
      "expected": 0,
      "policy_tags": []
    }
  ],
  "policies": [],
  "schemas": [],
  "default_tenant_id": 1
}
```

**Interpreting `scenario_next` result:**
- Look at `result.decision.outcome.kind`:
  - `advance` / `complete` -> gates passed
  - `hold` -> gates not satisfied
  - `fail` -> run failed
- Optional: `feedback: "trace"` can return gate + condition status (if permitted by server feedback policy).

---

## Pattern 2: Agent Loop

**When to use:** LLM agents iterating toward gate satisfaction

**Flow:**
1. Agent runs tools -> extracts values
2. `precheck` -> fast evaluation (asserted evidence)
3. If gates pass -> run live `scenario_next`

**Precheck output is limited:**
- Returns `{ decision, gate_evaluations }`.
- `gate_evaluations` contain `gate_id`, `status`, and condition trace only.
- It **does not** include evidence values or errors.

If you need evidence errors, use `evidence_query` or `runpack_export` in a live run.

---

## Pattern 3: Controlled Disclosure

**When to use:** Data release with audit trail

Typical flow:
1. Gate approvals using `RequireGroup`.
2. On pass, use `scenario_submit` to record metadata.
3. Dispatch data packets (policy-controlled).

`scenario_submit` is audit-only and requires:
- `run_id`, `tenant_id`, `namespace_id`, `submission_id`
- `payload`, `content_type`, `submitted_at`

---

## Pattern 4: Compliance Workflow (Multi-Stage)

Use stage ordering plus `advance_to.kind = "linear"` to move to the next stage in spec order:

```json dg-parse dg-level=fast
{
  "stage_id": "dev",
  "advance_to": { "kind": "linear" }
}
```

Use `branch` when you need different destinations based on gate outcome.

---

## Pattern 5: MCP Federation (External Providers)

**When to use:** Evidence sources outside built-ins

**Config (exact):**
```toml dg-parse dg-level=fast
[[providers]]
name = "git"
type = "mcp"
command = ["/usr/local/bin/git-provider"]
capabilities_path = "contracts/git.json"

[[providers]]
name = "cloud"
type = "mcp"
url = "https://cloud.example.com/rpc"
capabilities_path = "contracts/cloud.json"
allow_insecure_http = false
timeouts = { connect_timeout_ms = 2000, request_timeout_ms = 10000 }

[trust]
# Require signatures from these key files
default_policy = { require_signature = { keys = ["/etc/decision-gate/keys/cloud.pub"] } }
```

---

## Deployment Checklist

- [ ] Config validates (`decision-gate config validate`)
- [ ] Providers have valid `capabilities_path` files
- [ ] `namespace.allow_default` set correctly for local-only usage
- [ ] Auth + TLS (or `tls_termination = "upstream"`) configured for non-loopback binds
- [ ] `trust.default_policy` and `min_lane` set for production
- [ ] Evidence disclosure policy set (`allow_raw_values`, `require_provider_opt_in`)

---

## Cross-References

- [json_evidence_playbook.md](json_evidence_playbook.md)
- [llm_native_playbook.md](llm_native_playbook.md)
- [security_guide.md](security_guide.md)
