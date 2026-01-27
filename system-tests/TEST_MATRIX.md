<!--
System Tests Matrix
============================================================================
Document: Decision Gate System-Tests Matrix
Description: High-level view of system-test coverage by category.
Purpose: Provide a quick scan of system-test coverage and priorities.
============================================================================
-->

# Decision Gate System-Tests Matrix

## P0 (Must Pass)
| Test | Category | Purpose |
| --- | --- | --- |
| `smoke_define_start_next_status` | smoke | End-to-end lifecycle sanity via MCP HTTP. |
| `runpack_export_verify_happy_path` | runpack | Runpack export and verification passes. |
| `schema_conformance_all_tools` | contract | MCP tool outputs validate against schemas. |
| `evidence_redaction_default` | security | Raw evidence is redacted by default. |
| `http_bearer_token_required` | security | Bearer auth required for MCP tool calls. |
| `http_tool_allowlist_enforced` | security | Tool allowlist denies unauthorized MCP calls. |
| `http_mtls_subject_required` | security | mTLS subject required for MCP tool calls. |
| `sse_bearer_token_required` | security | SSE transport enforces bearer auth. |
| `http_rate_limit_enforced` | operations | Rate limiting rejects excess HTTP requests. |
| `http_tls_handshake_success` | operations | TLS handshake succeeds with test CA. |
| `http_mtls_client_cert_required` | security | mTLS client certs required when configured. |
| `http_audit_log_written` | operations | Audit log emits structured JSON lines. |
| `idempotent_trigger` | reliability | Duplicate trigger IDs do not create new decisions. |
| `provider_time_after` | providers | Time provider predicate executes as expected. |

## P1 (High Value)
| Test | Category | Purpose |
| --- | --- | --- |
| `http_transport_end_to_end` | mcp_transport | HTTP JSON-RPC transport works end-to-end. |
| `federated_provider_echo` | providers | External MCP provider integration works. |
| `assetcore_interop_fixtures` | providers | AssetCore interop fixture map executes via provider stub. |
| `assetcore_anchor_missing_fails_closed` | providers | Missing AssetCore anchors fail closed for evidence queries. |
| `assetcore_correlation_id_passthrough` | providers | Correlation IDs are preserved for AssetCore evidence queries. |
| `http_provider_discovery_tools` | contract | HTTP MCP exposes provider contract/schema discovery tools. |
| `stdio_provider_discovery_tools` | contract | Stdio MCP exposes provider contract/schema discovery tools. |
| `namespace_authority_allows_known_namespace` | security | AssetCore namespace authority allows known namespaces. |
| `namespace_authority_denies_unknown_namespace` | security | AssetCore namespace authority denies unknown namespaces. |
| `namespace_mismatch_rejected` | security | Namespace mismatch between spec and run config is rejected. |
| `packet_disclosure_visibility` | security | Packet visibility labels and policy tags persist. |
| `strict_mode_rejects_default_namespace` | security | Strict mode rejects default namespace. |
| `dev_permissive_emits_warning` | operations | Dev-permissive mode emits explicit warning. |
| `precheck_audit_hash_only` | operations | Precheck audit logs are hash-only by default. |
| `policy_denies_dispatch_targets` | security | Static policy engine denies disclosure and fails the run. |
| `policy_error_fails_closed` | security | Policy engine errors fail closed with explicit reason. |
| `runpack_tamper_detection` | runpack | Tampered runpack fails verification. |
| `strict_validation_precheck_rejects_comparator_mismatch` | functional | Precheck rejects comparator/schema mismatches. |
| `strict_validation_precheck_allows_permissive` | functional | Permissive mode allows precheck to proceed. |
| `strict_validation_rejects_disabled_comparators` | functional | Disabled comparator families are rejected. |
| `strict_validation_allows_enabled_comparators` | functional | Enabled comparator families are accepted. |
| `strict_validation_rejects_in_set_non_array` | functional | in_set requires expected array values. |
| `strict_validation_precheck_allows_union_contains` | functional | Union string/null schema permits contains. |
| `asc_auth_mapping_matrix` | security | ASC role/policy mapping enforced via auth proxy. |
| `assetcore_determinism_replay` | reliability | Identical AssetCore fixtures yield identical runpacks. |
| `registry_acl_builtin_matrix` | security | Builtin registry ACL matrix enforced across roles. |
| `registry_acl_principal_subject_mapping` | security | Registry ACL principal mapping for stdio/loopback/bearer/mTLS. |
| `registry_acl_signing_required_memory_and_sqlite` | security | Schema signing required for memory + sqlite registries. |
| `default_namespace_allowlist_enforced` | security | Default namespace allowlist blocks non-allowlisted tenants. |
| `dev_permissive_assetcore_rejected` | security | Dev-permissive rejected when AssetCore authority configured. |
| `registry_security_audit_events` | security | Registry/security audit events emitted for allow/deny. |

## P2 (Non-Gated / Extended Coverage)
| Test | Category | Purpose |
| --- | --- | --- |
| `stdio_transport_end_to_end` | mcp_transport | Stdio JSON-RPC transport handles tools/list and tools/call. |
| `multi_transport_parity` | mcp_transport | HTTP, stdio, and CLI interop parity for decisions/runpacks. |
| `runpack_export_includes_security_context` | runpack | Runpack manifests include security context metadata. |
| `anchor_validation_fuzz_cases_fail_closed` | security | Malformed/oversized anchors fail closed with explicit errors. |
| `performance_smoke` | performance | Non-gated MCP workflow throughput smoke test. |
| `stress_registry_concurrent_writes` | reliability | Concurrent schema registry writes remain stable and ordered. |
| `stress_schema_list_paging_concurrent_reads` | reliability | Schemas list paging stays deterministic under concurrent reads. |
| `stress_precheck_request_storm` | reliability | Precheck request storms fail closed and remain stable. |
