# sdks/python/decision_gate/__init__.py
# ============================================================================
# Module: Decision Gate Python SDK
# Description: Public exports for Decision Gate client SDK.
# Purpose: Provide a stable import surface for SDK consumers.
# Dependencies: decision-gate-sdk generated types
# ============================================================================

from .client import DecisionGateClient
from .errors import (
    DecisionGateError,
    DecisionGateProtocolError,
    DecisionGateRpcError,
    DecisionGateTransportError,
)
from ._generated import *  # noqa: F401,F403
from ._generated import __all__ as _generated_all

__all__ = [
    "DecisionGateClient",
    "DecisionGateError",
    "DecisionGateProtocolError",
    "DecisionGateRpcError",
    "DecisionGateTransportError",
] + _generated_all
