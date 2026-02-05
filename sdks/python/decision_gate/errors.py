# sdks/python/decision_gate/errors.py
# ============================================================================
# Module: SDK Errors
# Description: Error types for Decision Gate Python client SDK.
# Purpose: Provide structured error reporting for transport and JSON-RPC failures.
# Dependencies: stdlib
# ============================================================================

from __future__ import annotations

from typing import Any, Mapping, Optional

from ._generated import JsonValue


class DecisionGateError(Exception):
    """Base class for Decision Gate SDK errors."""


class DecisionGateTransportError(DecisionGateError):
    """Raised when the HTTP transport fails or returns a non-OK status."""

    def __init__(
        self,
        message: str,
        *,
        status_code: Optional[int] = None,
        body: Optional[str] = None,
        cause: Optional[BaseException] = None,
    ) -> None:
        super().__init__(message)
        self.status_code = status_code
        self.body = body
        self.cause = cause


class DecisionGateProtocolError(DecisionGateError):
    """Raised when the JSON-RPC response is malformed or unexpected."""


class DecisionGateRpcError(DecisionGateError):
    """Raised when the server returns a JSON-RPC error."""

    def __init__(
        self,
        code: int,
        message: str,
        *,
        data: Optional[Mapping[str, JsonValue]] = None,
        request_id: Optional[str] = None,
    ) -> None:
        super().__init__(message)
        self.code = code
        self.data = data
        self.request_id = request_id

    @property
    def kind(self) -> Optional[str]:
        if not self.data:
            return None
        value = self.data.get("kind")
        return value if isinstance(value, str) else None

    @property
    def retryable(self) -> Optional[bool]:
        if not self.data:
            return None
        value = self.data.get("retryable")
        return value if isinstance(value, bool) else None

    @property
    def retry_after_ms(self) -> Optional[int]:
        if not self.data:
            return None
        value = self.data.get("retry_after_ms")
        return value if isinstance(value, int) else None

    @property
    def raw_data(self) -> Optional[Mapping[str, Any]]:
        return self.data
