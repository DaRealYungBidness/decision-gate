# decision-gate-provider-sdk/python/provider.py
# ============================================================================
# Module: Python Evidence Provider Template
# Description: Minimal MCP stdio server for Decision Gate evidence queries.
# Purpose: Provide a starter implementation for `evidence_query` providers.
# Dependencies: Python standard library (json, sys, typing).
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

# ============================================================================
# SECTION: Limits and Constants
# ============================================================================

HEADER_SEPARATOR = b"\r\n\r\n"
MAX_HEADER_BYTES = 8 * 1024
MAX_BODY_BYTES = 1024 * 1024
DISCARD_CHUNK_BYTES = 8 * 1024

TOOL_LIST_RESULT = {
    "tools": [
        {
            "name": "evidence_query",
            "description": "Resolve a Decision Gate evidence query.",
            "input_schema": {"type": "object"},
        }
    ]
}

# ============================================================================
# SECTION: Exceptions
# ============================================================================


class FrameError(Exception):
    """Represents framing errors for Content-Length payloads."""

    def __init__(self, message: str, fatal: bool) -> None:
        super().__init__(message)
        self.fatal = fatal


# ============================================================================
# SECTION: Entry Point
# ============================================================================


def main() -> None:
    """Entry point for the MCP stdio loop."""
    stdin = sys.stdin.buffer
    stdout = sys.stdout.buffer
    while True:
        try:
            payload = read_frame(stdin)
        except FrameError as exc:
            write_frame(stdout, build_error_response(None, -32600, str(exc)))
            if exc.fatal:
                return
            continue
        if payload is None:
            return
        request = parse_request(payload)
        if request is None:
            write_frame(stdout, build_error_response(None, -32700, "invalid json"))
            continue
        response = handle_request(request)
        write_frame(stdout, response)


# ============================================================================
# SECTION: Framing
# ============================================================================


def read_frame(stream: Any) -> Optional[bytes]:
    """Reads a Content-Length framed payload from stdio."""
    content_length = None
    header_bytes = 0
    while True:
        line = stream.readline()
        if not line:
            return None
        header_bytes += len(line)
        if header_bytes > MAX_HEADER_BYTES:
            raise FrameError("headers too large", True)
        if line in (b"\r\n", b"\n"):
            break
        if line.lower().startswith(b"content-length:"):
            value = line.split(b":", 1)[1].strip()
            try:
                content_length = int(value)
            except ValueError as exc:
                raise FrameError("invalid content length", True) from exc
    if content_length is None:
        raise FrameError("missing content length", True)
    if content_length <= 0:
        raise FrameError("invalid content length", True)
    if content_length > MAX_BODY_BYTES:
        discard_bytes(stream, content_length)
        raise FrameError("payload too large", False)
    payload = stream.read(content_length)
    if len(payload) != content_length:
        raise FrameError("unexpected eof", True)
    return payload


def discard_bytes(stream: Any, count: int) -> None:
    """Drains oversized payloads without buffering them in memory."""
    remaining = count
    while remaining > 0:
        chunk = stream.read(min(remaining, DISCARD_CHUNK_BYTES))
        if not chunk:
            raise FrameError("unexpected eof", True)
        remaining -= len(chunk)


# ============================================================================
# SECTION: JSON-RPC Handling
# ============================================================================


def parse_request(payload: bytes) -> Optional[Dict[str, Any]]:
    """Parses a JSON-RPC request payload."""
    try:
        text = payload.decode("utf-8")
    except UnicodeDecodeError:
        return None
    try:
        parsed = json.loads(text)
    except json.JSONDecodeError:
        return None
    if not isinstance(parsed, dict):
        return None
    return parsed


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


# ============================================================================
# SECTION: Evidence Logic
# ============================================================================


def handle_tool_call(request: Dict[str, Any]) -> Dict[str, Any]:
    """Handles the evidence_query tool call."""
    params = request.get("params")
    if not isinstance(params, dict):
        return build_error_response(request.get("id"), -32602, "invalid tool params")
    if params.get("name") != "evidence_query":
        return build_error_response(request.get("id"), -32602, "invalid tool params")

    arguments = params.get("arguments")
    if not isinstance(arguments, dict):
        return build_error_response(request.get("id"), -32602, "invalid tool params")
    query = arguments.get("query")
    context = arguments.get("context")
    if not isinstance(query, dict) or not isinstance(context, dict):
        return build_error_response(request.get("id"), -32602, "missing query or context")

    result = handle_evidence_query(query, context)
    error = result.get("error")
    if error:
        return build_error_response(request.get("id"), -32000, str(error))

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

    if params.get("value") == "error":
        return {"error": "forced error"}

    return {
        "value": {"kind": "json", "value": params["value"]},
        "lane": "verified",
        "error": None,
        "evidence_hash": None,
        "evidence_ref": None,
        "evidence_anchor": None,
        "signature": None,
        "content_type": "application/json",
    }


# ============================================================================
# SECTION: JSON-RPC Output
# ============================================================================


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
