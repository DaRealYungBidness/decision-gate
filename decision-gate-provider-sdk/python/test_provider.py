# decision-gate-provider-sdk/python/test_provider.py
# ============================================================================
# Module: Python Evidence Provider Template Tests
# Description: Unit tests for framing and JSON-RPC helpers.
# Purpose: Validate Content-Length parsing and fail-closed behavior.
# Dependencies: Python standard library (io, unittest).
# ============================================================================

"""
## Overview
Exercise framing and request handling to ensure malformed inputs fail closed and
valid frames parse deterministically.
"""

from __future__ import annotations

import io
import unittest

import provider

# ============================================================================
# SECTION: Framing Tests
# ============================================================================


class FrameTests(unittest.TestCase):
    def test_read_frame_parses_content_length(self) -> None:
        payload = b"hello"
        stream = io.BytesIO(b"Content-Length: 5\r\n\r\n" + payload)

        result = provider.read_frame(stream)

        self.assertEqual(result, payload)

    def test_read_frame_missing_content_length(self) -> None:
        stream = io.BytesIO(b"X-Test: 1\r\n\r\nhello")

        with self.assertRaises(provider.FrameError) as context:
            provider.read_frame(stream)

        self.assertTrue(context.exception.fatal)


# ============================================================================
# SECTION: JSON-RPC Tests
# ============================================================================


class JsonRpcTests(unittest.TestCase):
    def test_parse_request_rejects_non_object(self) -> None:
        payload = b'["not", "object"]'

        result = provider.parse_request(payload)

        self.assertIsNone(result)

    def test_parse_request_rejects_invalid_utf8(self) -> None:
        payload = b"\xff"

        result = provider.parse_request(payload)

        self.assertIsNone(result)

    def test_handle_request_invalid_jsonrpc(self) -> None:
        response = provider.handle_request({"jsonrpc": "1.0", "id": 1})

        self.assertEqual(response["error"]["code"], -32600)

    def test_handle_request_rejects_non_object_params(self) -> None:
        response = provider.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/call",
                "params": "invalid",
            }
        )

        self.assertEqual(response["error"]["code"], -32602)


# ============================================================================
# SECTION: Evidence Tests
# ============================================================================


class EvidenceTests(unittest.TestCase):
    def test_handle_evidence_query_missing_value_sets_error(self) -> None:
        result = provider.handle_evidence_query({"params": {}}, {})

        self.assertIsNone(result["value"])
        self.assertIsNone(result["content_type"])
        self.assertEqual(result["error"]["code"], "invalid_params")


if __name__ == "__main__":
    unittest.main()
