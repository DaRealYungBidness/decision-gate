# sdks/python/decision_gate/client.py
# ============================================================================
# Module: Decision Gate Client
# Description: HTTP JSON-RPC client for Decision Gate MCP server.
# Purpose: Provide authenticated, structured access to Decision Gate tools.
# Dependencies: stdlib, decision-gate-sdk generated types
# ============================================================================

from __future__ import annotations

import json
import threading
import urllib.error
import urllib.request
from itertools import count
from typing import Dict, List, Literal, Mapping, Optional, TypedDict, Union, cast

from ._generated import GeneratedDecisionGateClient, JsonValue
from .errors import (
    DecisionGateProtocolError,
    DecisionGateRpcError,
    DecisionGateTransportError,
)

_DEFAULT_ENDPOINT = "http://127.0.0.1:8080/rpc"
_DEFAULT_TIMEOUT_SEC = 10.0

# JSON-RPC payload shapes used for strict typing and defensive validation.
JsonObject = Dict[str, JsonValue]
JsonRpcId = Union[int, str, None]


class JsonRpcRequestParams(TypedDict):
    name: str
    arguments: JsonValue


class JsonRpcRequest(TypedDict):
    jsonrpc: Literal["2.0"]
    id: int
    method: Literal["tools/call"]
    params: JsonRpcRequestParams


class JsonRpcError(TypedDict, total=False):
    code: int
    message: str
    data: JsonObject


class JsonRpcContentItem(TypedDict):
    type: Literal["json"]
    json: JsonValue


class JsonRpcResult(TypedDict):
    content: List[JsonRpcContentItem]


class JsonRpcResponse(TypedDict, total=False):
    jsonrpc: Literal["2.0"]
    id: JsonRpcId
    result: JsonRpcResult
    error: JsonRpcError


def _ensure_json_object(value: object, *, error_message: str) -> JsonObject:
    if not isinstance(value, dict):
        raise DecisionGateProtocolError(error_message)
    value_dict: Dict[object, object] = cast(Dict[object, object], value)
    for key in value_dict.keys():
        if not isinstance(key, str):
            raise DecisionGateProtocolError(error_message)
    return cast(JsonObject, value_dict)


def _ensure_content_item(value: JsonValue) -> JsonRpcContentItem:
    item = _ensure_json_object(value, error_message="invalid JSON-RPC content item")
    if item.get("type") != "json":
        raise DecisionGateProtocolError("unsupported JSON-RPC content type")
    if "json" not in item:
        raise DecisionGateProtocolError("missing JSON payload in content item")
    return cast(JsonRpcContentItem, item)


class DecisionGateClient(GeneratedDecisionGateClient):
    """HTTP JSON-RPC client for Decision Gate MCP server."""

    def __init__(
        self,
        *,
        endpoint: str = _DEFAULT_ENDPOINT,
        auth_token: Optional[str] = None,
        timeout_sec: Optional[float] = _DEFAULT_TIMEOUT_SEC,
        headers: Optional[Mapping[str, str]] = None,
        user_agent: Optional[str] = "decision-gate-python-sdk/0.1.0",
    ) -> None:
        self._endpoint = endpoint
        self._auth_token = auth_token
        self._timeout_sec = timeout_sec
        self._headers = dict(headers) if headers else {}
        self._user_agent = user_agent
        self._request_id = count(1)
        self._lock = threading.Lock()

    def _call_tool(self, name: str, arguments: JsonValue) -> JsonValue:
        request_id = self._next_request_id()
        payload: JsonRpcRequest = {
            "jsonrpc": "2.0",
            "id": request_id,
            "method": "tools/call",
            "params": {"name": name, "arguments": arguments},
        }
        data = json.dumps(payload).encode("utf-8")
        headers = self._build_headers()
        request = urllib.request.Request(self._endpoint, data=data, headers=headers, method="POST")
        try:
            with urllib.request.urlopen(request, timeout=self._timeout_sec) as response:
                status_code = response.status
                body = response.read().decode("utf-8")
        except urllib.error.HTTPError as exc:
            body = exc.read().decode("utf-8", errors="replace")
            raise DecisionGateTransportError(
                f"HTTP {exc.code} from Decision Gate",
                status_code=exc.code,
                body=body,
                cause=exc,
            ) from exc
        except urllib.error.URLError as exc:
            raise DecisionGateTransportError(
                "Decision Gate transport error",
                cause=exc,
            ) from exc

        if status_code < 200 or status_code >= 300:
            raise DecisionGateTransportError(
                f"HTTP {status_code} from Decision Gate",
                status_code=status_code,
                body=body,
            )

        try:
            response_payload_raw = json.loads(body)
        except json.JSONDecodeError as exc:
            raise DecisionGateProtocolError("invalid JSON-RPC response") from exc

        response_payload = _ensure_json_object(
            response_payload_raw,
            error_message="invalid JSON-RPC response shape",
        )

        jsonrpc_value = response_payload.get("jsonrpc")
        if jsonrpc_value is not None and jsonrpc_value != "2.0":
            raise DecisionGateProtocolError("invalid JSON-RPC version")

        if response_payload.get("error") is not None:
            raise self._rpc_error_from_payload(response_payload)

        result_value = response_payload.get("result")
        if not isinstance(result_value, dict):
            raise DecisionGateProtocolError("missing JSON-RPC result")
        result = _ensure_json_object(result_value, error_message="invalid JSON-RPC result shape")
        content = result.get("content")
        if not isinstance(content, list) or not content:
            raise DecisionGateProtocolError("missing JSON-RPC content")
        first = _ensure_content_item(content[0])
        return first["json"]

    def _build_headers(self) -> Dict[str, str]:
        headers: Dict[str, str] = {
            "Content-Type": "application/json",
        }
        if self._user_agent:
            headers["User-Agent"] = self._user_agent
        if self._auth_token:
            headers["Authorization"] = f"Bearer {self._auth_token}"
        headers.update(self._headers)
        return headers

    def _next_request_id(self) -> int:
        with self._lock:
            return next(self._request_id)

    def _rpc_error_from_payload(self, payload: Mapping[str, JsonValue]) -> DecisionGateRpcError:
        error_value = payload.get("error")
        error = _ensure_json_object(error_value, error_message="invalid JSON-RPC error shape")
        code_value = error.get("code")
        if not isinstance(code_value, int):
            raise DecisionGateProtocolError("invalid JSON-RPC error code")
        message_value = error.get("message")
        if not isinstance(message_value, str):
            raise DecisionGateProtocolError("invalid JSON-RPC error message")
        data_value = error.get("data")
        data = (
            _ensure_json_object(data_value, error_message="invalid JSON-RPC error data")
            if isinstance(data_value, dict)
            else None
        )
        request_id = payload.get("id")
        return DecisionGateRpcError(
            code_value,
            message_value,
            data=data,
            request_id=str(request_id) if request_id is not None else None,
        )
