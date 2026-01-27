# AGENTS.md (enterprise-system-tests)

> **Audience:** Agents and automation writing enterprise system tests.
> **Goal:** Validate enterprise features with deterministic, fail-closed tests.

---

## 0) TL;DR (one screen)

- **No OSS contamination:** enterprise tests stay in `enterprise/`.
- **Fail closed:** tests must never accept optional checks for required behavior.
- **Deterministic:** no wall-clock dependence; use readiness probes.
- **Auditability:** tests must emit artifacts suitable for audit review.
- **Standards:** follow **Docs/standards/codebase_engineering_standards.md** and
  **Docs/standards/codebase_formatting_standards.md**.
- **Threat model:** update **Docs/security/threat_model.md** when boundaries change.
- **Architecture docs:** keep `Docs/architecture/enterprise/*.md` current for new behavior.

---

## 1) In scope
- Tenant isolation, quota enforcement, and usage metering tests.
- SSO and authz policy enforcement for enterprise deployments.
- Audit export validation and retention behavior.

## 2) Out of scope (design approval required)
- Changing OSS system-test patterns or harnesses.
- Weakening fail-closed semantics in test expectations.

## 3) Non-negotiables
- Every test must be deterministic and produce artifacts.
- Do not use sleep for correctness; use explicit readiness checks.
- Failures must be explicit and actionable.

## 4) Test Registry and Gaps
When you add, rename, or remove a test:
- Register it in `enterprise/enterprise-system-tests/test_registry.toml`.
- Add/update gaps in `enterprise/enterprise-system-tests/test_gaps.toml`.
- Regenerate coverage docs: `python scripts/coverage_report.py generate`.
- Update `enterprise/enterprise-system-tests/README.md` if referenced.

## 5) Artifact Contract
Each test writes artifacts under the run root:
- `summary.json` (canonical JSON)
- `summary.md` (human-readable summary)
- `tool_transcript.json` (JSON-RPC transcripts)

## 6) Running Tests
Enterprise system-tests are feature-gated to avoid running by default.

```bash
cargo test -p enterprise-system-tests --features enterprise-system-tests
cargo nextest run -p enterprise-system-tests --features enterprise-system-tests
```

## 7) References
- Docs/standards/codebase_formatting_standards.md
- Docs/standards/codebase_engineering_standards.md
- Docs/security/threat_model.md
- Docs/roadmap/enterprise/enterprise_phasing_plan.md
- Docs/architecture/enterprise/decision_gate_enterprise_system_test_architecture.md
