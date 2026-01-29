# adapters/langchain/src/decision_gate_langchain/tools.py
# ============================================================================
# Module: Decision Gate LangChain Tools
# Description: Build LangChain tools that call Decision Gate via the SDK.
# ============================================================================

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Dict, List, Mapping, Optional

from langchain_core.tools import BaseTool, tool

from decision_gate import (
    DecisionGateClient,
    validate_precheck_request,
    validate_runpack_export_request,
    validate_scenario_define_request,
    validate_scenario_start_request,
    validate_scenario_trigger_request,
    validate_scenario_next_request,
    validate_scenario_status_request,
)


def _maybe_validate(enabled: bool, validator, payload: Dict[str, Any]) -> None:
    if enabled:
        validator(payload)


@dataclass(frozen=True)
class DecisionGateToolConfig:
    """Configuration helper for constructing Decision Gate LangChain tools."""

    endpoint: str = "http://127.0.0.1:8080/rpc"
    auth_token: Optional[str] = None
    timeout_sec: Optional[float] = 10.0
    headers: Optional[Mapping[str, str]] = None
    user_agent: Optional[str] = "decision-gate-langchain/0.1.0"
    validate: bool = False

    def create_client(self) -> DecisionGateClient:
        return DecisionGateClient(
            endpoint=self.endpoint,
            auth_token=self.auth_token,
            timeout_sec=self.timeout_sec,
            headers=self.headers,
            user_agent=self.user_agent,
        )


def build_decision_gate_tools(
    client: DecisionGateClient,
    *,
    validate: bool = False,
) -> List[BaseTool]:
    """Return LangChain tools for core Decision Gate operations."""

    @tool("decision_gate_precheck")
    def decision_gate_precheck(request: Dict[str, Any]) -> Dict[str, Any]:
        """Run a Decision Gate precheck without mutating run state."""
        _maybe_validate(validate, validate_precheck_request, request)
        return client.precheck(request)

    @tool("decision_gate_scenario_next")
    def decision_gate_scenario_next(request: Dict[str, Any]) -> Dict[str, Any]:
        """Advance a Decision Gate run to the next stage if gates pass."""
        _maybe_validate(validate, validate_scenario_next_request, request)
        return client.scenario_next(request)

    @tool("decision_gate_scenario_status")
    def decision_gate_scenario_status(request: Dict[str, Any]) -> Dict[str, Any]:
        """Fetch current Decision Gate run status without mutation."""
        _maybe_validate(validate, validate_scenario_status_request, request)
        return client.scenario_status(request)

    @tool("decision_gate_scenario_define")
    def decision_gate_scenario_define(request: Dict[str, Any]) -> Dict[str, Any]:
        """Register a ScenarioSpec before starting a run."""
        _maybe_validate(validate, validate_scenario_define_request, request)
        return client.scenario_define(request)

    @tool("decision_gate_scenario_start")
    def decision_gate_scenario_start(request: Dict[str, Any]) -> Dict[str, Any]:
        """Start a new run for a registered scenario."""
        _maybe_validate(validate, validate_scenario_start_request, request)
        return client.scenario_start(request)

    @tool("decision_gate_scenario_trigger")
    def decision_gate_scenario_trigger(request: Dict[str, Any]) -> Dict[str, Any]:
        """Trigger a scenario evaluation with an external event."""
        _maybe_validate(validate, validate_scenario_trigger_request, request)
        return client.scenario_trigger(request)

    @tool("decision_gate_runpack_export")
    def decision_gate_runpack_export(request: Dict[str, Any]) -> Dict[str, Any]:
        """Export an audit-grade runpack for a scenario run."""
        _maybe_validate(validate, validate_runpack_export_request, request)
        return client.runpack_export(request)

    return [
        decision_gate_precheck,
        decision_gate_scenario_next,
        decision_gate_scenario_status,
        decision_gate_scenario_define,
        decision_gate_scenario_start,
        decision_gate_scenario_trigger,
        decision_gate_runpack_export,
    ]


def build_decision_gate_tools_from_config(
    config: DecisionGateToolConfig,
) -> List[BaseTool]:
    """Build tools using a config object instead of a prebuilt client."""
    client = config.create_client()
    return build_decision_gate_tools(client, validate=config.validate)
