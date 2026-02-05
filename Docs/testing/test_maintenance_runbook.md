# Test Maintenance Runbook

## Goals
- Preserve fail-closed behavior and deterministic evaluation
- Prevent test theatre: every test must validate real behavior
- Keep unit tests fast (<100ms per test where feasible)

## When Adding Features
1. Identify affected threat models (TM-*)
2. Add or update unit tests in tests/ directories
3. Update threat_model_test_mapping.md
4. Add or extend property-based tests if input space expands

## Regression Workflow
1. Reproduce with a focused test case
2. Add a minimal unit test that fails on the bug
3. Fix the bug
4. Ensure the test passes and remains deterministic

## Performance Discipline
- Avoid network calls in unit tests (use local servers or stubs)
- Avoid global state; use temporary directories and in-memory stores
- Prefer deterministic seeds in property tests when debugging

## Flakiness Triage
- Rerun failing test with --nocapture and a fixed seed
- Identify time-dependent behavior (timeouts, sleeps)
- Replace with deterministic stubs where possible

## CI Expectations
- Unit tests run on each PR
- System tests run on merge / nightly
- Property tests run with default case count on each PR
