# Decision Gate Interop Test Contract

## Purpose
These tests prove the CLI interop runner enforces the threat model for untrusted
MCP HTTP responses. They are designed to be deterministic under parallel test
execution and to fail closed on malformed or hostile inputs.

## Scope
Applies to: `decision-gate-cli/src/tests/interop.rs` and the interop HTTP client
in `decision-gate-cli/src/interop.rs`.

## Security Invariants (Traceability)
- **Strict size limits before JSON parsing**: responses at or below the limit
  must succeed, responses over the limit must fail closed.
- **Fail closed on malformed responses**: invalid JSON or invalid JSON-RPC
  responses must abort the run.
- **Fail closed on explicit server errors**: non-2xx HTTP status and JSON-RPC
  error payloads must abort the run.
- **Require JSON content in tool payloads**: tool results without JSON content
  must abort the run.
- **Deterministic transcript ordering**: tool calls must be captured in order
  with no missing entries.

These invariants map to the threat model guidance in
`Docs/security/threat_model.md` (untrusted network inputs, strict bounds, and
fail-closed behavior).

## Test Inventory
- `run_interop_executes_full_sequence`: happy path; transcript order and
  argument integrity.
- `run_interop_accepts_response_at_size_limit`: boundary acceptance at the
  maximum response size.
- `run_interop_rejects_oversized_response_body`: boundary rejection over the
  maximum response size.
- `run_interop_rejects_invalid_jsonrpc_response`: malformed JSON response.
- `run_interop_rejects_http_error_status`: non-2xx HTTP response.
- `run_interop_rejects_jsonrpc_error_payload`: JSON-RPC error response.
- `run_interop_rejects_tool_without_json_content`: missing JSON content in tool
  response.
- `run_interop_rejects_define_scenario_mismatch`: integrity check for scenario
  identity.

## Reliability Notes
The test server is fully async with explicit readiness signaling and shutdown,
removing timing-based flakiness under parallel execution.
