# Decision Gate - Killer Use Cases

**Based on:** Real AI agent testing + architectural understanding
**Validation Status:** 1 tested, 2 proposed
**Last Updated:** 2026-01-30

---

## Use Case 1: AI Research Quality Gates ✅ TESTED

### The Problem
AI agents can generate research, but how do you PROVE it meets quality standards? Prompt engineering alone isn't enough - you need deterministic, auditable quality checks.

### The Solution with Decision Gate
Define quality gates that the AI must satisfy before research is considered complete:
- Minimum source count
- Academic source requirements
- Length/depth requirements
- Citation quality thresholds

### Real-World Scenario
```
Company policy: "All AI-generated research reports must:
- Cite at least 3 sources
- Include at least 2 academic/peer-reviewed sources
- Provide summaries of at least 500 characters
- Be completed after policy approval timestamp"
```

### Implementation
```json
{
  "scenario_id": "research-quality",
  "gates": [
    {
      "gate_id": "quality",
      "requirement": {
        "And": [
          {"Condition": "source_count"},
          {"Condition": "academic_sources"},
          {"Condition": "summary_length"},
          {"Condition": "after_approval"}
        ]
      }
    }
  ]
}
```

### Value Delivered
- ✅ **Provable compliance**: Runpack proves research met standards
- ✅ **Deterministic**: Same report always gets same evaluation
- ✅ **Auditable**: Full trail of iterations and improvements
- ✅ **Replayable**: Can verify offline that standards were met

### Who Needs This?
- Research teams using AI assistants
- Compliance-heavy industries (healthcare, legal, finance)
- Academic institutions using AI for research
- Companies with AI governance policies

### Tested Result
✅ Successfully tested with Claude as AI agent
✅ Agent iterated from failing gates to passing gates
✅ Runpack audit trail is complete and verifiable

---

## Use Case 2: AI Agent Data Access Control 🎯 HIGH VALUE

### The Problem
You want AI agents to access customer PII, but ONLY when multiple conditions are met. You need proof that access was authorized, not just logging that says "we checked."

### The Nightmare Scenario
```
CEO: "Did our AI agent access customer data appropriately?"
Dev: "Well, we log when it accesses data..."
CEO: "But can you PROVE the customer consented?"
Dev: "Uh... we check that in code..."
CEO: "Show me the proof."
Dev: 😰
```

### The Solution with Decision Gate
Before agent accesses PII, it must pass gates:
- Customer has opted in (database check)
- Agent has certification (credential check)
- Access is during business hours (time check)
- Session is being recorded (env var check)
- Purpose is documented (submission check)

### Implementation
```json
{
  "scenario_id": "pii-access",
  "gates": [
    {
      "gate_id": "access-authorized",
      "requirement": {
        "And": [
          {"Condition": "customer_consent"},
          {"Condition": "agent_certified"},
          {"Condition": "business_hours"},
          {"Condition": "session_recorded"},
          {"Condition": "purpose_documented"}
        ]
      }
    }
  ]
}
```

### Value Delivered
- 🔐 **Cryptographic proof**: Evidence is hashed and anchored
- 📜 **Compliance trail**: Runpack proves all conditions were met
- 🚫 **Fail-safe**: If any condition fails, access is denied
- 🔍 **Audit-grade**: Can show regulators exactly what happened

### Who Needs This?
- Healthcare (HIPAA)
- Finance (PCI-DSS, SOX)
- Customer support AI agents
- Any company handling sensitive data

### Why This Beats "Normal" Logging
| Logging | Decision Gate |
|---------|---------------|
| "We checked X" | "Here's cryptographic proof we checked X" |
| Logs can be modified | Hashes are tamper-evident |
| Post-hoc analysis | Real-time gating (if failed, denied) |
| Hard to replay | Runpack is self-contained proof |

### Validation Status
⚠️ **Needs Testing** - Requires external MCP provider for database checks

---

## Use Case 3: Progressive Deployment with Safety Rails 🎯 HIGH VALUE

### The Problem
You want to deploy to production, but only if:
- All tests passed
- No critical CVEs
- Canary deployment shows healthy metrics
- At least 2 team members approved

You could script this, but can you PROVE to your VP that all conditions were met when prod went down?

### The Solution with Decision Gate
Make deployment a gated scenario with verifiable evidence.

### Implementation
```json
{
  "scenario_id": "prod-deploy",
  "stages": [
    {
      "stage_id": "pre-deploy",
      "gates": [
        {
          "gate_id": "quality",
          "requirement": {
            "And": [
              {"Condition": "tests_passed"},
              {"Condition": "no_critical_cves"},
              {"Condition": "coverage_ok"},
              {"Condition": "approvals_met"}
            ]
          }
        }
      ],
      "advance_to": {"kind": "fixed", "stage_id": "canary"}
    },
    {
      "stage_id": "canary",
      "gates": [
        {
          "gate_id": "canary-health",
          "requirement": {
            "And": [
              {"Condition": "error_rate_low"},
              {"Condition": "latency_ok"},
              {"Condition": "after_observation_period"}
            ]
          }
        }
      ],
      "advance_to": {"kind": "fixed", "stage_id": "full-deploy"}
    },
    {
      "stage_id": "full-deploy",
      "gates": [],
      "entry_packets": [
        {
          "packet_id": "deploy-approved",
          "payload": {"deployed_at": "...", "version": "..."}
        }
      ]
    }
  ]
}
```

### Value Delivered
- 📊 **Evidence composition**: Combines test results + CVE scans + metrics + approvals
- ⏱️ **Time-based gates**: Can't deploy until canary runs for X minutes
- 🔄 **Automatic rollback**: If canary fails, stage doesn't advance
- 📋 **Post-incident clarity**: "Did we follow our process?" → Yes, here's the runpack

### Who Needs This?
- Platform engineering teams
- Companies with change management requirements
- Regulated industries with deployment policies
- Teams that want GitOps-style audit trails

### Why This Beats GitHub Actions / CI/CD
| CI/CD | Decision Gate |
|-------|---------------|
| Pass/fail steps | Composable evidence from multiple sources |
| Approval gates | Cryptographically-verifiable gates |
| Logs | Tamper-evident audit trail |
| Re-run to see what happened | Runpack is self-contained proof |

**Decision Gate doesn't replace CI/CD - it's the audit layer on top.**

### Validation Status
⚠️ **Needs Testing** - Requires:
- JSON reports from test runner / scanner
- HTTP provider for metrics endpoint
- MCP provider for approval system

---

## Common Objections & Responses

### "Isn't this just overkill for most cases?"

**Answer:** Yes, for simple checks. But for:
- Regulated industries
- AI agents with sensitive access
- High-stakes deployments
- Scenarios requiring proof (not just logs)

...it's not overkill, it's the minimum bar.

### "Can't I just do this with bash scripts?"

**Answer:** You can check conditions in bash. You can't:
- Generate tamper-evident audit trails
- Compose evidence from multiple trust domains
- Replay verification offline
- Cryptographically prove conditions were met

### "Why not just use <other tool>?"

**Answer:**
- **Temporal/Prefect/Airflow**: Workflow engines, not gating systems. They run tasks, not evaluate evidence.
- **OPA/Cedar**: Policy engines, not evidence composers. They evaluate predicates, not fetch and hash evidence.
- **GitHub Actions**: CI/CD orchestrator, not an audit system. Logs can be modified.
- **Custom logging**: Works until you need to prove compliance to regulators.

Decision Gate is the "control plane for deterministic checkpoints."

---

## When NOT to Use Decision Gate

Be honest about the anti-patterns:

❌ **Simple validation** - "Is this email valid?" → Just use a regex
❌ **Low-stakes decisions** - "Should I send this notification?" → Not worth the overhead
❌ **Real-time critical path** - High-frequency, low-latency checks → Too slow
❌ **One-off scripts** - "Did my script finish?" → Overkill
❌ **No audit requirements** - If you don't need proof, you don't need this

---

## Value Proposition Matrix

| Scenario | Evidence Sources | Audit Need | DG Fit |
|----------|------------------|------------|--------|
| AI research quality | JSON reports, time | Compliance | ✅ High |
| AI PII access | Database, creds, env | Legal/Regulatory | ✅ High |
| Prod deployment | Tests, CVEs, metrics, approvals | Post-incident analysis | ✅ High |
| Data disclosure | Approvals, time, env | Audit/Compliance | ✅ High |
| Multi-agent handoff | Agent outputs, state checks | Coordination proof | ✅ High |
| Form validation | Input fields | None | ❌ Low |
| Notification send | Simple true/false | None | ❌ Low |
| Health check | Single endpoint | None | ❌ Low |

---

## Demo Scenarios (Copy-Paste Ready)

### Coming Soon
- [ ] Research quality gate (JSON provider)
- [ ] CI gate (tests + coverage + CVEs)
- [ ] Time-based release gate
- [ ] Multi-stage deployment gate
- [ ] Data access control gate (requires DB provider)

---

## PMF Validation Checklist

For each use case, we need to answer:

✅ **Use Case 1 (AI Research Quality)**
- [x] Does it solve a painful problem? YES - provable AI quality standards
- [x] Is DG the right tool? YES - audit trails + deterministic evaluation
- [?] Would someone pay for it? MAYBE - depends on compliance requirements
- [x] Is value prop obvious? YES - "prove AI met standards"

⏳ **Use Case 2 (AI PII Access)**
- [x] Does it solve a painful problem? YES - regulatory compliance for AI
- [x] Is DG the right tool? YES - cryptographic proof of authorization
- [ ] Would someone pay for it? UNKNOWN - needs customer interviews
- [x] Is value prop obvious? YES - "prove AI access was authorized"

⏳ **Use Case 3 (Progressive Deployment)**
- [?] Does it solve a painful problem? MAYBE - depends on change management culture
- [?] Is DG the right tool? MAYBE - could be overkill vs GitHub Actions
- [ ] Would someone pay for it? UNKNOWN - needs validation
- [?] Is value prop obvious? SOMEWHAT - needs "beyond CI/CD" messaging

---

## Next Steps for Validation

1. **Test Use Case 2** - Build a database MCP provider and run the PII access scenario
2. **Test Use Case 3** - Integrate with real CI/CD pipeline
3. **Customer Interviews** - Talk to 5 companies about AI governance needs
4. **Pricing Research** - What would companies pay for audit-grade AI gates?
5. **Competitive Analysis** - What do OPA, HashiCorp Boundary, etc. cost?

---

## Messaging Framework

### Elevator Pitch (30 seconds)
"Decision Gate is a deterministic checkpoint system for AI agents and critical workflows. It evaluates evidence-backed gates, generates tamper-evident audit trails, and provides cryptographic proof that requirements were met. Think: 'GitOps for AI governance' or 'OPA with audit trails.'"

### Why Now? (Market Timing)
- AI agents are accessing sensitive data (PII, financial, health)
- Regulators are asking "how do you govern AI?"
- Companies need audit trails, not just logs
- No existing tool bridges evidence composition + proof generation

### Differentiation
- **vs OPA**: We compose evidence from multiple sources, not just evaluate policies
- **vs Workflow Engines**: We gate decisions, not orchestrate tasks
- **vs CI/CD**: We provide cryptographic proof, not just pass/fail signals
- **vs Logging**: We generate tamper-evident trails, not append-only logs

---

**End of Document**
