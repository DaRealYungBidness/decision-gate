# CI and Release Scripts

Local CI-style gates and release tooling.

- `generate_all.sh`: Contract + SDK generation pipeline (`--check` verifies drift).
- `verify_all.sh`: Primary local gate (generation check, unit tests, optional docs, Python formatting, system tests, packaging, adapters, SBOM).
- `package_dry_run.sh`: Build and install Python and TypeScript SDKs without publishing.
- `sbom.sh`: Generate a dependency SBOM via cargo-sbom.
- `ci_release_gate.sh`: Evaluate the CI release gate scenario against an evidence bundle.

Examples:

- `scripts/ci/verify_all.sh --system-tests=p0`
- `scripts/ci/verify_all.sh --python-format`
- `scripts/ci/package_dry_run.sh --all`
- `scripts/ci/sbom.sh --output .tmp/ci/sbom/decision-gate.sbom.spdx.json`
