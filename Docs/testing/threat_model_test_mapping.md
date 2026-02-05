# Threat Model to Test Mapping

This document maps threat model identifiers (TM-*) to unit and system tests.
It is a living index; update alongside new threat models or tests.

## TM-HTTP-001 (SSRF)
- crates/decision-gate-providers/tests/http_provider.rs
- crates/decision-gate-providers/tests/http_provider_unit.rs
- system-tests/tests/suites/providers.rs

## TM-HTTP-002 (TLS)
- system-tests/tests/suites/providers.rs
- crates/decision-gate-providers/tests/http_provider_unit.rs (ignored TLS scaffolding)

## TM-HTTP-003 (resource exhaustion)
- crates/decision-gate-providers/tests/http_provider_unit.rs
- system-tests/tests/suites/providers.rs

## TM-FILE-001 (path traversal)
- crates/decision-gate-providers/tests/json_provider.rs
- crates/decision-gate-providers/tests/json_provider_path_unit.rs

## TM-FILE-002 (symlink escape)
- crates/decision-gate-providers/tests/json_provider_path_unit.rs

## TM-JSON-001 (JSONPath injection)
- crates/decision-gate-providers/tests/json_provider.rs
- crates/decision-gate-providers/tests/json_provider_path_unit.rs
- system-tests/tests/suites/providers.rs

## TM-ENV-001 (env disclosure)
- crates/decision-gate-providers/tests/env_provider.rs

## TM-COMP-001 (comparator bypass)
- crates/decision-gate-core/tests/comparator.rs
- crates/decision-gate-core/tests/comparator_numeric_precision.rs
- crates/decision-gate-core/tests/proptest_comparator.rs

## TM-COMP-002 (numeric precision)
- crates/decision-gate-core/tests/comparator_numeric_precision.rs
- crates/decision-gate-core/tests/decimal_precision_extended.rs
- crates/decision-gate-core/tests/proptest_comparator.rs

## TM-TRUST-001 (trust lane bypass)
- crates/decision-gate-core/tests/trust_lane.rs
- crates/decision-gate-core/tests/precheck.rs
- crates/decision-gate-core/tests/trust_lane_runtime.rs

## TM-TRUST-002 (signature forgery)
- crates/decision-gate-core/tests/trust_lane_runtime.rs

## TM-STORE-001 (store tampering)
- crates/decision-gate-core/tests/store.rs
- crates/decision-gate-store-sqlite/tests/sqlite_store.rs
- crates/decision-gate-store-sqlite/tests/sqlite_store_unit.rs

## TM-STORE-002 (store corruption)
- crates/decision-gate-store-sqlite/tests/sqlite_store.rs
- crates/decision-gate-store-sqlite/tests/sqlite_store_unit.rs

## TM-STORE-003 (concurrency)
- crates/decision-gate-store-sqlite/tests/sqlite_store_unit.rs

## TM-PROV-001 (provider DoS)
- crates/decision-gate-core/tests/provider_orchestration_unit.rs
- crates/decision-gate-providers/tests/registry.rs

## TM-PROV-002 (provider confusion)
- crates/decision-gate-core/tests/provider_orchestration_unit.rs
- crates/decision-gate-providers/tests/registry.rs

## TM-STAGE-001 (stage bypass)
- crates/decision-gate-core/tests/control_plane.rs
- crates/decision-gate-core/tests/multi_stage_unit.rs
- crates/decision-gate-core/tests/timeouts.rs

## TM-STAGE-002 (timeout manipulation)
- crates/decision-gate-core/tests/timeouts.rs
- crates/decision-gate-core/tests/multi_stage_unit.rs

## TM-EVID-001 (correlation attack)
- crates/decision-gate-core/tests/evidence_correlation_unit.rs

## TM-EVID-002 (replay)
- crates/decision-gate-core/tests/evidence_correlation_unit.rs

## TM-REG-001 (provider access control bypass)
- crates/decision-gate-providers/tests/registry.rs

## TM-VAL-001 (schema/comparator manipulation)
- crates/decision-gate-mcp/tests/validation.rs

## TM-RUNPACK-001 (path traversal / length abuse)
- crates/decision-gate-mcp/tests/runpack_io.rs
- crates/decision-gate-core/tests/runpack.rs
