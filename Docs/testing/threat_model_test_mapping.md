# Threat Model to Test Mapping

This document maps threat model identifiers (TM-*) to unit and system tests.
It is a living index; update alongside new threat models or tests.

## TM-HTTP-001 (SSRF)
- decision-gate-providers/tests/http_provider.rs
- decision-gate-providers/tests/http_provider_unit.rs
- system-tests/tests/suites/providers.rs

## TM-HTTP-002 (TLS)
- system-tests/tests/suites/providers.rs
- decision-gate-providers/tests/http_provider_unit.rs (ignored TLS scaffolding)

## TM-HTTP-003 (resource exhaustion)
- decision-gate-providers/tests/http_provider_unit.rs
- system-tests/tests/suites/providers.rs

## TM-FILE-001 (path traversal)
- decision-gate-providers/tests/json_provider.rs
- decision-gate-providers/tests/json_provider_path_unit.rs

## TM-FILE-002 (symlink escape)
- decision-gate-providers/tests/json_provider_path_unit.rs

## TM-JSON-001 (JSONPath injection)
- decision-gate-providers/tests/json_provider.rs
- decision-gate-providers/tests/json_provider_path_unit.rs
- system-tests/tests/suites/providers.rs

## TM-ENV-001 (env disclosure)
- decision-gate-providers/tests/env_provider.rs

## TM-COMP-001 (comparator bypass)
- decision-gate-core/tests/comparator.rs
- decision-gate-core/tests/comparator_numeric_precision.rs
- decision-gate-core/tests/proptest_comparator.rs

## TM-COMP-002 (numeric precision)
- decision-gate-core/tests/comparator_numeric_precision.rs
- decision-gate-core/tests/decimal_precision_extended.rs
- decision-gate-core/tests/proptest_comparator.rs

## TM-TRUST-001 (trust lane bypass)
- decision-gate-core/tests/trust_lane.rs
- decision-gate-core/tests/precheck.rs
- decision-gate-core/tests/trust_lane_runtime.rs

## TM-TRUST-002 (signature forgery)
- decision-gate-core/tests/trust_lane_runtime.rs

## TM-STORE-001 (store tampering)
- decision-gate-core/tests/store.rs
- decision-gate-store-sqlite/tests/sqlite_store.rs
- decision-gate-store-sqlite/tests/sqlite_store_unit.rs

## TM-STORE-002 (store corruption)
- decision-gate-store-sqlite/tests/sqlite_store.rs
- decision-gate-store-sqlite/tests/sqlite_store_unit.rs

## TM-STORE-003 (concurrency)
- decision-gate-store-sqlite/tests/sqlite_store_unit.rs

## TM-PROV-001 (provider DoS)
- decision-gate-core/tests/provider_orchestration_unit.rs
- decision-gate-providers/tests/registry.rs

## TM-PROV-002 (provider confusion)
- decision-gate-core/tests/provider_orchestration_unit.rs
- decision-gate-providers/tests/registry.rs

## TM-STAGE-001 (stage bypass)
- decision-gate-core/tests/control_plane.rs
- decision-gate-core/tests/multi_stage_unit.rs
- decision-gate-core/tests/timeouts.rs

## TM-STAGE-002 (timeout manipulation)
- decision-gate-core/tests/timeouts.rs
- decision-gate-core/tests/multi_stage_unit.rs

## TM-EVID-001 (correlation attack)
- decision-gate-core/tests/evidence_correlation_unit.rs

## TM-EVID-002 (replay)
- decision-gate-core/tests/evidence_correlation_unit.rs

## TM-REG-001 (provider access control bypass)
- decision-gate-providers/tests/registry.rs

## TM-VAL-001 (schema/comparator manipulation)
- decision-gate-mcp/tests/validation.rs

## TM-RUNPACK-001 (path traversal / length abuse)
- decision-gate-mcp/tests/runpack_io.rs
- decision-gate-core/tests/runpack.rs
