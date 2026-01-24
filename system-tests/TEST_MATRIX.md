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
| `packet_disclosure_visibility` | security | Packet visibility labels and policy tags persist. |
| `runpack_tamper_detection` | runpack | Tampered runpack fails verification. |
