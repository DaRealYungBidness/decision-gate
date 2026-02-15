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
| `debug_mutation_stats_auth_and_schema` | security | Debug mutation stats endpoint is auth-protected and schema-stable. |
| `http_rate_limit_enforced` | operations | Rate limiting rejects excess HTTP requests. |
| `http_tls_handshake_success` | operations | TLS handshake succeeds with test CA. |
| `http_mtls_client_cert_required` | security | mTLS client certs required when configured. |
| `http_audit_log_written` | operations | Audit log emits structured JSON lines. |
| `http_payload_too_large_rejected` | operations | HTTP rejects oversized JSON-RPC payloads before parsing. |
| `sse_payload_too_large_rejected` | operations | SSE rejects oversized JSON-RPC payloads before parsing. |
| `idempotent_trigger` | reliability | Duplicate trigger IDs do not create new decisions. |
| `provider_time_after` | providers | Time provider check executes as expected. |
| `metamorphic_concurrent_runs_identical_runpacks` | reliability | Concurrent runs yield identical runpack hashes. |
| `metamorphic_evidence_order_canonical_in_runpack` | reliability | Gate eval evidence ordering is canonical in runpacks. |
| `evidence_query_fuzz_inputs_fail_closed` | security | Malformed evidence_query payloads fail closed. |
| `log_leak_scan_redacts_secrets` | security | Secrets do not appear in stderr or audit logs. |
| `http_provider_slow_loris_fails_closed` | providers | HTTP provider fails closed on slow-loris responses. |
| `http_provider_truncated_response_fails_closed` | providers | HTTP provider rejects truncated responses. |
| `http_provider_redirect_loop_not_followed` | providers | HTTP provider does not follow redirect loops. |
| `time_provider_timezone_offset_parsing` | providers | Time provider parses timezone offsets. |
| `time_provider_epoch_boundary` | providers | Time provider handles epoch boundary timestamps. |
| `mcp_provider_signature_key_not_authorized` | providers | MCP provider rejects unauthorized signature keys. |
| `mcp_provider_signature_verification_failed` | providers | MCP provider rejects invalid signatures. |

## P1 (High Value)
| Test | Category | Purpose |
| --- | --- | --- |
| `agentic_flow_harness_deterministic` | agentic | Canonical agentic scenarios across projections (raw MCP + SDKs/adapters). |
| `preset_quickstart_dev_http` | operations | Quickstart-Dev preset runs scenario lifecycle over HTTP. |
| `preset_default_recommended_http` | operations | Default-Recommended preset runs scenario + registry write. |
| `preset_hardened_http` | operations | Hardened preset enforces bearer auth + signing requirement. |
| `python_sdk_http_scenario_lifecycle` | functional | Python SDK executes scenario lifecycle over MCP HTTP. |
| `python_sdk_bearer_auth_enforced` | security | Python SDK succeeds with bearer token and fails without. |
| `typescript_sdk_http_scenario_lifecycle` | functional | TypeScript SDK executes scenario lifecycle over MCP HTTP. |
| `typescript_sdk_bearer_auth_enforced` | security | TypeScript SDK succeeds with bearer token and fails without. |
| `python_examples_runnable` | functional | Python repository examples execute end-to-end over MCP HTTP. |
| `typescript_examples_runnable` | functional | TypeScript repository examples execute end-to-end over MCP HTTP. |
| `http_transport_end_to_end` | mcp_transport | HTTP JSON-RPC transport works end-to-end. |
| `sse_transport_end_to_end` | mcp_transport | SSE transport supports tools/list and tools/call end-to-end. |
| `docs_search_http_end_to_end` | functional | Docs search returns deterministic sections and overview over HTTP. |
| `docs_search_sse_end_to_end` | mcp_transport | Docs search works end-to-end over SSE. |
| `docs_resources_http_list_read` | mcp_transport | Resources list/read return embedded docs over HTTP with auth. |
| `docs_resources_sse_list_read` | mcp_transport | Resources list/read return embedded docs over SSE with auth. |
| `federated_provider_echo` | providers | External MCP provider integration works. |
| `provider_template_python` | providers | Python provider template handles tools/list/tools/call and wiring. |
| `provider_template_go` | providers | Go provider template handles tools/list/tools/call and wiring. |
| `provider_template_typescript` | providers | TypeScript provider template handles tools/list/tools/call and wiring. |
| `provider_template_error_fails_closed` | providers | Provider template errors fail closed in Decision Gate flows. |
| `json_provider_missing_jsonpath_returns_error_metadata` | providers | JSON provider emits structured error metadata for missing JSONPath. |
| `json_provider_rejects_path_outside_root` | providers | JSON provider blocks path traversal outside configured root. |
| `json_provider_enforces_size_limit` | providers | JSON provider enforces max_bytes file size limit. |
| `json_provider_rejects_symlink_escape` | providers | JSON provider blocks symlink escapes outside root. |
| `json_provider_invalid_jsonpath_rejected` | providers | JSON provider rejects invalid JSONPath expressions. |
| `json_provider_contains_array_succeeds` | providers | JSON provider path + contains comparator evaluates end-to-end. |
| `http_provider_blocks_http_scheme_by_default` | providers | HTTP provider blocks cleartext HTTP by default. |
| `http_provider_enforces_allowlist` | providers | HTTP provider enforces host allowlist. |
| `http_provider_redirect_not_followed` | providers | HTTP provider returns redirect status without following. |
| `http_provider_body_hash_matches` | providers | HTTP provider body_hash returns canonical hash. |
| `http_provider_response_size_limit_enforced` | providers | HTTP provider enforces response size limits. |
| `http_provider_timeout_enforced` | providers | HTTP provider request timeouts are enforced. |
| `http_provider_tls_failure_fails_closed` | providers | HTTP provider fails closed on TLS errors. |
| `env_provider_missing_key_returns_empty` | providers | Env provider returns empty result for missing keys. |
| `env_provider_denylist_blocks` | providers | Env provider denylist blocks access. |
| `env_provider_allowlist_blocks_unlisted` | providers | Env provider allowlist blocks unlisted keys. |
| `env_provider_value_size_limit_enforced` | providers | Env provider enforces value size limits. |
| `env_provider_key_size_limit_enforced` | providers | Env provider enforces key size limits. |
| `time_provider_rejects_logical_when_disabled` | providers | Time provider rejects logical timestamps when disabled. |
| `time_provider_rfc3339_parsing` | providers | Time provider parses RFC3339 timestamps. |
| `time_provider_invalid_rfc3339_rejected` | providers | Time provider rejects invalid RFC3339 strings. |
| `mcp_provider_malformed_jsonrpc_response` | providers | MCP provider malformed responses fail closed. |
| `mcp_provider_text_content_rejected` | providers | MCP provider text responses are rejected. |
| `mcp_provider_empty_result_rejected` | providers | MCP provider empty results fail closed. |
| `mcp_provider_flaky_response` | providers | Flaky MCP providers fail closed and recover. |
| `mcp_provider_wrong_namespace_rejected` | providers | MCP provider rejects wrong namespace. |
| `mcp_provider_missing_signature_rejected` | providers | Signature-required MCP providers reject unsigned evidence. |
| `mcp_provider_contract_mismatch_rejected` | providers | MCP provider contract mismatches are rejected. |
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
| `invalid_correlation_id_rejected` | security | Invalid correlation IDs are rejected with server correlation headers. |
| `dev_permissive_emits_warning` | operations | Dev-permissive mode emits explicit warning. |
| `precheck_audit_hash_only` | operations | Precheck audit logs are hash-only by default. |
| `cli_workflows_end_to_end` | operations | CLI serve/runpack/authoring/config/provider/interop flows succeed. |
| `cli_rejects_non_loopback_bind` | operations | CLI rejects non-loopback binds without explicit allow flag. |
| `cli_size_limits_enforced` | operations | CLI enforces input/output size limits with fail-closed behavior. |
| `cli_auth_profile_bearer_token` | security | CLI auth profile supplies bearer token for MCP calls. |
| `cli_auth_profile_cli_override` | security | CLI bearer token flag overrides auth profile credentials. |
| `cli_config_env_override` | operations | CLI config validate honors DECISION_GATE_CONFIG env override. |
| `precheck_read_only_does_not_mutate_run_state` | functional | Precheck leaves run state unchanged. |
| `policy_denies_dispatch_targets` | security | Static policy engine denies disclosure and fails the run. |
| `policy_error_fails_closed` | security | Policy engine errors fail closed with explicit reason. |
| `runpack_tamper_detection` | runpack | Tampered runpack fails verification. |
| `strict_validation_precheck_rejects_comparator_mismatch` | functional | Precheck rejects comparator/schema mismatches. |
| `strict_validation_precheck_allows_permissive` | functional | Permissive mode allows precheck to proceed. |
| `strict_validation_rejects_disabled_comparators` | functional | Disabled comparator families are rejected. |
| `strict_validation_allows_enabled_comparators` | functional | Enabled comparator families are accepted. |
| `strict_validation_rejects_in_set_non_array` | functional | in_set requires expected array values. |
| `strict_validation_precheck_allows_union_contains` | functional | Union string/null schema permits contains. |
| `json_evidence_playbook_templates_pass` | functional | JSON evidence playbook templates pass via JSON provider. |
| `llm_native_precheck_payload_flow` | functional | LLM-native precheck payload flow succeeds with asserted evidence. |
| `asc_auth_mapping_matrix` | security | ASC role/policy mapping enforced via auth proxy. |
| `assetcore_determinism_replay` | reliability | Identical AssetCore fixtures yield identical runpacks. |
| `registry_acl_builtin_matrix` | security | Builtin registry ACL matrix enforced across roles. |
| `registry_acl_principal_subject_mapping` | security | Registry ACL principal mapping for stdio/loopback/bearer/mTLS. |
| `registry_acl_signing_required_memory_and_sqlite` | security | Schema signing required for memory + sqlite registries. |
| `default_namespace_allowlist_enforced` | security | Default namespace allowlist blocks non-allowlisted tenants. |
| `dev_permissive_assetcore_rejected` | security | Dev-permissive rejected when AssetCore authority configured. |
| `registry_security_audit_events` | security | Registry/security audit events emitted for allow/deny. |
| `schema_registry_cursor_rejects_invalid_inputs` | security | Malformed registry cursors/limits are rejected. |
| `schema_registry_invalid_schema_and_precheck_rejected` | security | Invalid schemas and precheck payloads fail closed. |
| `cli_auth_matrix` | security | CLI MCP client enforces bearer + mTLS subject auth. |
| `sqlite_registry_and_runpack_persist_across_restart` | reliability | SQLite registry + run state persist across restarts with runpack export. |
| `docs_config_toggles` | operations | Docs enable/disable toggles enforce visibility and availability. |
| `server_tools_visibility_filtering` | security | Tool visibility allowlist/denylist filtering hides tool calls. |
| `server_tools_visibility_defaults_and_auth_separation` | security | Auth allowlist does not alter tools/list visibility. |
| `cli_mcp_tool_wrappers_conformance` | functional | CLI MCP tool wrappers execute against a live MCP server. |

## P2 (Non-Gated / Extended Coverage)
| Test | Category | Purpose |
| --- | --- | --- |
| `authoring_ron_normalize_and_execute` | functional | RON authoring normalizes and executes through Decision Gate. |
| `authoring_invalid_ron_rejected` | functional | Invalid RON authoring input is rejected. |
| `authoring_dsl_evaluates_and_rejects_deep_inputs` | functional | DSL authoring executes and deep inputs are rejected. |
| `stdio_transport_end_to_end` | mcp_transport | Stdio JSON-RPC transport handles tools/list and tools/call. |
| `sse_transport_bearer_rejects_missing_token` | mcp_transport | SSE transport rejects missing bearer token. |
| `multi_transport_parity` | mcp_transport | HTTP, stdio, and CLI interop parity for decisions/runpacks. |
| `runpack_export_includes_security_context` | runpack | Runpack manifests include security context metadata. |
| `anchor_validation_fuzz_cases_fail_closed` | security | Malformed/oversized anchors fail closed with explicit errors. |
| `perf_core_mcp_throughput_release` | performance | Release-profile throughput + latency SLO gate for scenario start/trigger. |
| `perf_precheck_throughput_release` | performance | Release-profile throughput + latency SLO gate for precheck workflow. |
| `perf_registry_mixed_throughput_release` | performance | Release-profile throughput + latency SLO gate for schemas register/list mix. |
| `perf_sqlite_core_mcp_throughput_release` | performance_sqlite | Release-profile SQLite WAL+FULL scenario start/trigger throughput diagnostics (report-only) with MCP mutation coordinator diagnostics. |
| `perf_sqlite_precheck_throughput_release` | performance_sqlite | Release-profile SQLite WAL+FULL precheck throughput diagnostics (report-only) with measured-window throughput accounting. |
| `perf_sqlite_registry_mixed_throughput_release` | performance_sqlite | Release-profile SQLite WAL+FULL schemas register/list throughput diagnostics (report-only) with MCP mutation coordinator diagnostics. |
| `perf_sqlite_store_run_state_contention_release` | performance_sqlite | Direct SQLite run-state save/load contention microbench with latency, DB error, and writer batch/queue diagnostics. |
| `perf_sqlite_store_registry_contention_release` | performance_sqlite | Direct SQLite registry register/list contention microbench with latency, DB error, and writer batch/queue diagnostics. |
| `stress_registry_concurrent_writes` | reliability | Concurrent schema registry writes remain deterministic and ordered (not a throughput SLA test). |
| `stress_schema_list_paging_concurrent_reads` | reliability | Schemas list paging stays deterministic under concurrent reads (not a throughput SLA test). |
| `stress_precheck_request_storm` | reliability | Precheck storms remain fail-closed and deterministic (not a throughput SLA test). |
| `sdk_gen_cli_generate_and_check` | operations | SDK generator CLI generate/check and drift detection. |
| `contract_cli_generate_and_check` | operations | Contract CLI generate/check and drift detection. |
| `broker_composite_sources_and_sinks` | operations | CompositeBroker resolves file/http/inline sources and dispatches via sink. |
| `provider_discovery_denylist_and_size_limits` | contract | Provider discovery denylist and size limits enforced. |
| `docs_extra_paths_ingestion_limits` | operations | Docs extra_paths ingestion honors size and count limits. |
| `cli_smoke_version` | smoke | CLI --version output is available and well-formed. |
| `cli_transport_matrix` | mcp_transport | CLI MCP client parity across HTTP/SSE/stdio. |
| `cli_golden_provider_list` | operations | CLI provider list output matches golden JSON fixture. |
| `cli_i18n_catalan_disclaimer` | operations | CLI Catalan output includes machine-translation disclaimer. |
