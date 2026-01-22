# decision-gate-provider-sdk/python/provider.py
# ============================================================================
# Module: Python Evidence Provider Template
# Description: Minimal MCP stdio server for Decision Gate evidence queries.
# Purpose: Provide a starter implementation for `evidence_query` providers.
# Dependencies: Python standard library (json, sys).
# ============================================================================

"""
## Overview
This template implements the MCP `tools/list` and `tools/call` handlers over
stdio. It parses Content-Length framed JSON-RPC messages and replies with a
JSON EvidenceResult. Security posture: inputs are untrusted and must be
validated; see Docs/security/threat_model.md.
"""

from __future__ import annotations

import json
import sys
from typing import Any, Dict, Optional

HEADER_SEPARATOR = b"\r\n\r\n"

TOOL_LIST_RESULT = {
    "tools": [
        {
            "name": "evidence_query",
            "description": "Resolve a Decision Gate evidence query.",
            "input_schema": {"type": "object"},
        }
    ]
}


def main() -> None:
    """Entry point for the MCP stdio loop."""
    stdin = sys.stdin.buffer
    stdout = sys.stdout.buffer
    while True:
        payload = read_frame(stdin)
        if payload is None:
            return
        request = parse_request(payload)
        if request is None:
            write_frame(stdout, build_error_response(None, -32700, "invalid json"))
            continue
        response = handle_request(request)
        write_frame(stdout, response)


def read_frame(stream: Any) -> Optional[bytes]:
    """Reads a Content-Length framed payload from stdio."""
    content_length = None
    while True:
        line = stream.readline()
        if not line:
            return None
        if line in (b"\r\n", b"\n"):
            break
        if line.lower().startswith(b"content-length:"):
            value = line.split(b":", 1)[1].strip()
            content_length = int(value)
    if content_length is None:
        return None
    return stream.read(content_length)


def parse_request(payload: bytes) -> Optional[Dict[str, Any]]:
    """Parses a JSON-RPC request payload."""
    try:
        return json.loads(payload.decode("utf-8"))
    except json.JSONDecodeError:
        return None


def handle_request(request: Dict[str, Any]) -> Dict[str, Any]:
    """Dispatches JSON-RPC requests for tools/list and tools/call."""
    if request.get("jsonrpc") != "2.0":
        return build_error_response(request.get("id"), -32600, "invalid json-rpc version")

    method = request.get("method")
    if method == "tools/list":
        return {"jsonrpc": "2.0", "id": request.get("id"), "result": TOOL_LIST_RESULT}
    if method == "tools/call":
        return handle_tool_call(request)
    return build_error_response(request.get("id"), -32601, "method not found")


def handle_tool_call(request: Dict[str, Any]) -> Dict[str, Any]:
    """Handles the evidence_query tool call."""
    params = request.get("params") or {}
    if params.get("name") != "evidence_query":
        return build_error_response(request.get("id"), -32602, "invalid tool params")

    arguments = params.get("arguments") or {}
    query = arguments.get("query")
    context = arguments.get("context")
    if query is None or context is None:
        return build_error_response(request.get("id"), -32602, "missing query or context")

    result = handle_evidence_query(query, context)
    if "error" in result:
        return build_error_response(request.get("id"), -32000, result["error"])

    return {
        "jsonrpc": "2.0",
        "id": request.get("id"),
        "result": {"content": [{"type": "json", "json": result}]},
    }


def handle_evidence_query(query: Dict[str, Any], _context: Dict[str, Any]) -> Dict[str, Any]:
    """Resolves the evidence query (template logic)."""
    params = query.get("params") or {}
    if "value" not in params:
        return {"error": "params.value is required"}

    return {
        "value": {"kind": "json", "value": params["value"]},
        "evidence_hash": None,
        "evidence_ref": None,
        "evidence_anchor": None,
        "signature": None,
        "content_type": "application/json",
    }


def build_error_response(request_id: Any, code: int, message: str) -> Dict[str, Any]:
    """Builds a JSON-RPC error response."""
    return {"jsonrpc": "2.0", "id": request_id, "error": {"code": code, "message": message}}


def write_frame(stream: Any, response: Dict[str, Any]) -> None:
    """Writes a JSON-RPC response using Content-Length framing."""
    payload = json.dumps(response).encode("utf-8")
    header = f"Content-Length: {len(payload)}\r\n\r\n".encode("utf-8")
    stream.write(header)
    stream.write(payload)
    stream.flush()


if __name__ == "__main__":
    main()

