<!--
ci_release_gate_dogfood.md
============================================================================
Document: Decision Gate CI Release Gate Dogfooding
Description: Rigorous CI release eligibility gating using Decision Gate.
Purpose: Explain the evidence bundle, policy, and runpack artifacts.
Dependencies:
  - scripts/ci/ci_release_gate.sh
  - configs/ci/release_gate_scenario.json
  - configs/presets/ci-release-gate.toml
  - Docs/ci_current_state_and_plan.md
============================================================================
-->

# CI Release Gate Dogfooding

This guide documents how Decision Gate dogfoods itself by gating release
eligibility using deterministic CI evidence. The goal is to demonstrate a
real, auditable policy layer without replacing the CI system.

## Why This Is Rigorous

- **Separation of concerns**: CI executes tests; Decision Gate evaluates the
  policy and emits a deterministic decision.
- **Evidence-driven**: The decision is based on a versioned evidence bundle
  rather than implicit CI logs.
- **Auditable output**: Decision Gate exports a runpack with the full decision
  trail and verification data.
- **Deterministic**: The same evidence bundle yields the same decision.

## What Is Gated

The release tag workflow validates that a release is eligible based on:

- Formatting, lint, and unit tests
- System tests P0 and P1
- `cargo-deny`
- Generator drift checks
- Dependency SBOM (Rust deps only)
- Packaging dry runs (Python + TypeScript)
- Docker smoke test
- Tag/version consistency (tag matches workspace version)

If any requirement is missing or false, the gate denies the release.

## Evidence Bundle

The release workflow writes a JSON evidence bundle to an evidence workspace
root and evaluates it via a root-relative file path. The default example is
`./evidence/release_evidence.json` (relative to the repo root), but the root
can be overridden at runtime with `--json-root`.

Example (shape only):

```json dg-parse
{
  "release": {
    "tag": "v0.1.0",
    "version": "0.1.0",
    "tag_matches_version": true,
    "sha": "<git sha>",
    "generated_at": 1710000000000,
    "sbom_path": ".tmp/ci/sbom/decision-gate.sbom.spdx.json"
  },
  "checks": {
    "fmt": true,
    "clippy": true,
    "cargo_deny": true,
    "generate_all": true,
    "unit_tests": true,
    "system_tests_p0": true,
    "system_tests_p1": true,
    "sbom": true,
    "package_dry_run": true,
    "docker_smoke": true
  }
}
```

## Policy Scenario

The release gate is expressed as a standard Decision Gate scenario:

- **Template**: `configs/ci/release_gate_scenario.json`
- **Policy**: All conditions must be true (`And` gate)
- **Provider**: built-in `json` provider reads the evidence bundle
- **Execution**: live scenario run (not precheck) so a runpack is produced

The scenario is instantiated at runtime by replacing the template placeholders:

- `{{SCENARIO_ID}}` -> unique scenario identifier
- `{{EVIDENCE_FILE}}` -> relative path to the evidence bundle (within the `json` provider root)

## How It Runs in CI

The release workflow performs the following sequence:

1. Runs CI checks (fmt, clippy, tests, deny, packaging, smoke test).
2. Generates a dependency SBOM (Rust deps only; container SBOM/provenance are
   not yet emitted).
3. Writes the evidence bundle.
4. Starts a local MCP server with `configs/presets/ci-release-gate.toml`
   (optionally adding `--json-root <evidence-root>` and
   `--json-root-id <root-id>`).
5. Evaluates the scenario using the evidence bundle.
6. Exports and verifies a runpack.
7. Uploads artifacts:
   - Evidence bundle
   - Runpack
   - Dependency SBOM
   - Decision payload and summary
   - Artifact name: `decision-gate-release-gate`

The implementation lives in `scripts/ci/ci_release_gate.sh` and is called by the
release workflow.

## How Publishing Is Guarded

Publishing is a separate manual workflow (`.github/workflows/publish.yml`).
Before publishing, it verifies that the tag release workflow completed
successfully, which includes the Decision Gate evaluation. This ensures the
publish step is never run without a passing, audited policy decision.

## Running Locally

You can run the same release gate locally with a custom evidence bundle:

```bash dg-skip dg-reason="requires local CI context" dg-expires="2026-12-31"
python3 - <<'PY'
import json
from pathlib import Path

Path("evidence/release_evidence.json").write_text(json.dumps({
    "release": {
        "tag": "v0.1.0",
        "version": "0.1.0",
        "tag_matches_version": True,
        "sha": "local",
        "generated_at": 0,
        "sbom_path": "evidence/sbom/decision-gate.sbom.spdx.json",
    },
    "checks": {
        "fmt": True,
        "clippy": True,
        "cargo_deny": True,
        "generate_all": True,
        "unit_tests": True,
        "system_tests_p0": True,
        "system_tests_p1": True,
        "sbom": True,
        "package_dry_run": True,
        "docker_smoke": True,
    },
}, indent=2))
PY

bash scripts/ci/ci_release_gate.sh \
  --evidence-file evidence/release_evidence.json \
  --output-dir evidence/release-runpack \
  --config configs/presets/ci-release-gate.toml
```

If any check is false, the script exits non-zero and the decision summary will
show the denial reason.

## Artifacts to Inspect

- `decision_payload.json`: raw Decision Gate response payload
- `decision_summary.json`: decision kind + allow/deny
- `runpack/manifest.json`: deterministic runpack manifest
- `runpack_verify.json`: runpack verification output

## Related References

- `Docs/ci_current_state_and_plan.md`
- `configs/ci/release_gate_scenario.json`
- `scripts/ci/ci_release_gate.sh`
