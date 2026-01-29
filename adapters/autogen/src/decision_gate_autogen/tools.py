# adapters/autogen/src/decision_gate_autogen/tools.py
# ============================================================================
# Module: Decision Gate AutoGen Tools
# Description: Build AutoGen FunctionTool entries that call Decision Gate.
# ============================================================================

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Dict, List, Mapping, Optional

from autogen_core.tools import FunctionTool

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
    """Configuration helper for constructing Decision Gate AutoGen tools."""

    endpoint: str = "http://127.0.0.1:8080/rpc"
    auth_token: Optional[str] = None
    timeout_sec: Optional[float] = 10.0
    headers: Optional[Mapping[str, str]] = None
    user_agent: Optional[str] = "decision-gate-autogen/0.1.0"
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
) -> List[FunctionTool]:
    """Return AutoGen FunctionTool entries for core Decision Gate operations."""

    def decision_gate_precheck(request: Dict[str, Any]) -> Dict[str, Any]:
        """Run a Decision Gate precheck without mutating run state."""
        _maybe_validate(validate, validate_precheck_request, request)
        return client.precheck(request)

    def decision_gate_scenario_next(request: Dict[str, Any]) -> Dict[str, Any]:
        """Advance a Decision Gate run to the next stage if gates pass."""
        _maybe_validate(validate, validate_scenario_next_request, request)
        return client.scenario_next(request)

    def decision_gate_scenario_status(request: Dict[str, Any]) -> Dict[str, Any]:
        """Fetch current Decision Gate run status without mutation."""
        _maybe_validate(validate, validate_scenario_status_request, request)
        return client.scenario_status(request)

    def decision_gate_scenario_define(request: Dict[str, Any]) -> Dict[str, Any]:
        """Register a ScenarioSpec before starting a run."""
        _maybe_validate(validate, validate_scenario_define_request, request)
        return client.scenario_define(request)

    def decision_gate_scenario_start(request: Dict[str, Any]) -> Dict[str, Any]:
        """Start a new run for a registered scenario."""
        _maybe_validate(validate, validate_scenario_start_request, request)
        return client.scenario_start(request)

    def decision_gate_scenario_trigger(request: Dict[str, Any]) -> Dict[str, Any]:
        """Trigger a scenario evaluation with an external event."""
        _maybe_validate(validate, validate_scenario_trigger_request, request)
        return client.scenario_trigger(request)

    def decision_gate_runpack_export(request: Dict[str, Any]) -> Dict[str, Any]:
        """Export an audit-grade runpack for a scenario run."""
        _maybe_validate(validate, validate_runpack_export_request, request)
        return client.runpack_export(request)

    return [
        FunctionTool(
            decision_gate_precheck,
            name="decision_gate_precheck",
            description="Run a Decision Gate precheck without mutating run state.",
        ),
        FunctionTool(
            decision_gate_scenario_next,
            name="decision_gate_scenario_next",
            description="Advance a Decision Gate run to the next stage if gates pass.",
        ),
        FunctionTool(
            decision_gate_scenario_status,
            name="decision_gate_scenario_status",
            description="Fetch current Decision Gate run status without mutation.",
        ),
        FunctionTool(
            decision_gate_scenario_define,
            name="decision_gate_scenario_define",
            description="Register a ScenarioSpec before starting a run.",
        ),
        FunctionTool(
            decision_gate_scenario_start,
            name="decision_gate_scenario_start",
            description="Start a new run for a registered scenario.",
        ),
        FunctionTool(
            decision_gate_scenario_trigger,
            name="decision_gate_scenario_trigger",
            description="Trigger a scenario evaluation with an external event.",
        ),
        FunctionTool(
            decision_gate_runpack_export,
            name="decision_gate_runpack_export",
            description="Export an audit-grade runpack for a scenario run.",
        ),
    ]


def build_decision_gate_tools_from_config(
    config: DecisionGateToolConfig,
) -> List[FunctionTool]:
    """Build tools using a config object instead of a prebuilt client."""
    client = config.create_client()
    return build_decision_gate_tools(client, validate=config.validate)
