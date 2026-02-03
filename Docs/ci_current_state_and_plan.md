# Decision Gate CI Current State and Plan

## Current State (2026-02-03)

- CI workflows are now in place:
  - `.github/workflows/ci_pr.yml` (PR gate)
  - `.github/workflows/ci_main.yml` (main gate: P0 + P1)
  - `.github/workflows/ci_manual.yml` (manual P2 runs)
  - `.github/workflows/release.yml` (tag-driven release validation pipeline)
  - `.github/workflows/publish.yml` (manual publish pipeline)
  - `.github/workflows/golden_runpack_cross_os.yml` (cross-OS golden runpack test)
- Local CI-style entrypoints live under `scripts/ci/`, with supporting helpers in
  `scripts/docs/`, `scripts/system_tests/`, and `scripts/adapters/`.
- System-tests are registry driven. Coverage and infrastructure docs are generated from the registry.
- There are no scheduled (nightly) CI jobs yet.
- `cargo-deny` is configured (`deny.toml`) and enforced in CI.
- Release validation includes a Decision Gate release eligibility runpack.
- Release validation generates a dependency SBOM (Rust deps only).
- There are no long-running fuzzing harnesses in place yet; current "fuzz" coverage is deterministic
  system-tests only.

## Why This Plan Is Rigorous

Rigorous here means: an outside reviewer can see intentional, repeatable, evidence-driven gates
that favor correctness and supply-chain hygiene over convenience.

- **Deterministic and auditable**: generator drift checks and pinned toolchains reduce surprises.
- **Strict quality gates**: warnings are treated as errors; linting is mandatory.
- **System confidence**: required system tests on `main` prove integration health.
- **Supply-chain discipline**: `cargo-deny` catches license/security/source problems early.
- **Deliberate releases**: tags trigger publishing; `main` is continuous integration, not auto-release.

## Decisions (2026-02-03)

- **PR gate (required)** runs: fmt, clippy, generator drift check, unit tests, and `cargo-deny`.
- **Main gate (required)** runs **P0 + P1** system tests.
- **P2 system tests** are **manual** (workflow dispatch); no scheduled/nightly jobs yet.
- **Cross-OS golden runpack** remains a separate, required workflow.
- **Releases are tag-driven** only (e.g., `v0.1.0`), not on every merge to `main`.
- **Docker**: build on PR; push only via manual publish workflow.
- **Multi-arch images**: build `linux/amd64` and `linux/arm64`; smoke-test `amd64` only and document
  the arm64 testing limitation.
- **SBOM**: generate a dependency SBOM on release tags; container SBOM/provenance deferred.

## Existing CI-Capable Scripts

| Script                                    | Purpose                            | Typical usage                                              | Notes                                                                                                                           |
| ----------------------------------------- | ---------------------------------- | ---------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| `scripts/ci/generate_all.sh`              | Contract + SDK generation pipeline | `scripts/ci/generate_all.sh --check`                       | Verifies generated artifacts match committed outputs.                                                                           |
| `scripts/ci/verify_all.sh`                | Primary local CI gate              | `scripts/ci/verify_all.sh`                                 | Runs generation check + `cargo test --workspace --exclude system-tests`; optional system tests, packaging, adapter tests, SBOM. |
| `scripts/ci/sbom.sh`                      | Dependency SBOM generator          | `scripts/ci/sbom.sh`                                       | Generates a dependency SBOM (Rust deps) using cargo-sbom; used in release pipeline.                                             |
| `scripts/ci/package_dry_run.sh`           | Packaging verification             | `scripts/ci/package_dry_run.sh --all`                      | Builds and installs Python + TypeScript SDKs without publishing.                                                                |
| `scripts/adapters/adapter_tests.sh`       | Adapter smoke tests                | `scripts/adapters/adapter_tests.sh --all`                  | Spawns a local MCP server (unless endpoint provided) and runs framework examples in a venv.                                     |
| `scripts/system_tests/test_runner.py`     | System-test runner                 | `python scripts/system_tests/test_runner.py --priority P0` | Registry-driven, supports parallelism, timeouts, and artifact capture.                                                          |
| `scripts/system_tests/coverage_report.py` | Coverage and infra docs            | `python scripts/system_tests/coverage_report.py generate`  | Writes `Docs/testing/decision_gate_test_coverage.md` and `Docs/testing/test_infrastructure_guide.md`.                           |
| `scripts/system_tests/gap_tracker.py`     | Coverage gap management            | `python scripts/system_tests/gap_tracker.py list`          | Lists, closes, or generates task prompts for system-test gaps.                                                                  |

## Formal CI Flow (Implemented)

### PR Gate (Fast, Required)

- **Format:** `cargo +nightly fmt --all -- --check`
- **Lint:** `cargo clippy --all-targets --all-features -D warnings`
- **Supply-chain policy:** `cargo deny check`
- **Generation drift:** `scripts/ci/generate_all.sh --check`
- **Unit tests:** `cargo test --workspace --exclude system-tests`
- **Docs run (fast):** `python scripts/docs/docs_verify.py --run --level=fast` (guides + SDK READMEs; requires `PyYAML`)
- **Cross-OS gate:** keep `golden_runpack_cross_os` as a dedicated required workflow

### Main Gate (Required)

- **System tests P0:** `python scripts/system_tests/test_runner.py --priority P0`
- **System tests P1:** `python scripts/system_tests/test_runner.py --priority P1`

### Manual Heavy Gate (On Demand)

- **System tests P2:** `python scripts/system_tests/test_runner.py --priority P2`

### Release Pipeline (Tag-Driven)

- Triggered by pushing a version tag (e.g., `v0.1.0`).
- Re-run main-level checks to ensure release readiness.
- Build artifacts locally (packaging dry-runs) to validate release readiness.
- Generate a dependency SBOM (Rust deps only; container SBOM/provenance deferred).
- Evaluate release eligibility with Decision Gate and export a runpack artifact.
- Publishing is delegated to the manual publish workflow.

### Manual Publish (On Demand)

- Triggered manually on a tag ref via `.github/workflows/publish.yml`.
- Supports publishing Rust crates, Python SDK, TypeScript SDK, and Docker images.
- Requires explicit registry tokens; missing tokens fail the workflow early.
- Verifies the tag release validation workflow completed successfully before publishing.

## Docker + Multi-Arch Guidance

- **PRs:** build only (no push) to validate Dockerfile integrity.
- **Publish workflow:** build + push with buildx for `linux/amd64` and `linux/arm64`.
- **Smoke tests:** run on `amd64` only; explicitly document that arm64 is built in CI but not fully
  integration-tested yet.

## Cross-Architecture Reality

Local development currently targets Windows. CI provides Linux/Windows runners, and buildx handles
arm64 builds without local ARM hardware. The repo should clearly document architecture guarantees:

- **Tested:** `amd64` (Linux/Windows), `golden_runpack_cross_os` parity tests
- **Built but not fully tested:** `arm64` (CI buildx)

## CI Policy Goals

- Treat warnings as errors in CI (Clippy with `-D warnings`).
- Enforce generator drift checks on every PR.
- Require supply-chain policy checks (`cargo-deny`).
- Require system-test P0 + P1 on `main`.
- Keep system-test artifacts as CI outputs (manifest + per-test logs).
- Align CI behavior with `scripts/ci/verify_all.sh` to keep local and CI gates consistent.

## Optional Fast Linkers (Local Opt-In)

Rust build times can improve with faster linkers (clang + lld on Linux/macOS, or lld-link on
Windows), but OSS repos should avoid forcing a toolchain download on contributors. Recommended
approach is **opt-in** via local configuration:

- **Local only:** Document optional settings in developer docs; do not require them in CI.
- **Common patterns:** `.cargo/config.toml` or environment variables such as
  `RUSTFLAGS="-C linker=clang -C link-arg=-fuse-ld=lld"` on Linux/macOS, or
  `RUSTFLAGS="-C linker=lld-link"` on Windows.
- **Graceful fallback:** Builds should succeed without these settings; CI should not depend on them.

## Implementation Map (Implemented)

- Keep: `.github/workflows/golden_runpack_cross_os.yml` (cross-OS required check)
- Add: `.github/workflows/ci_pr.yml` (PR gate)
- Add: `.github/workflows/ci_main.yml` (main gate: P0 + P1)
- Add: `.github/workflows/ci_manual.yml` (manual P2 runs)
- Add: `.github/workflows/release.yml` (tag-driven release validation)
- Add: `.github/workflows/publish.yml` (manual publish pipeline)

## Deferred Work (Explicit)

- Scheduled/nightly CI runs for P2, adapter tests, and packaging checks.
- Extended system-test coverage and additional adapters in CI.
- Fuzzing: only add once high-leverage fuzz targets are identified.
- Full supply-chain attestations (container SBOM, signatures, provenance).
