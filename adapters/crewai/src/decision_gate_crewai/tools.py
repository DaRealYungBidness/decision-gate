# adapters/crewai/src/decision_gate_crewai/tools.py
# ============================================================================
# Module: Decision Gate CrewAI Tools
# Description: Build CrewAI tools that call Decision Gate via the SDK.
# ============================================================================

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Dict, Mapping, Optional, Type

from crewai.tools import BaseTool
from pydantic import BaseModel, Field, PrivateAttr

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


def _as_json(result: Dict[str, Any]) -> str:
    return json.dumps(result, separators=(",", ":"), sort_keys=True)


@dataclass(frozen=True)
class DecisionGateToolConfig:
    """Configuration helper for constructing Decision Gate CrewAI tools."""

    endpoint: str = "http://127.0.0.1:8080/rpc"
    auth_token: Optional[str] = None
    timeout_sec: Optional[float] = 10.0
    headers: Optional[Mapping[str, str]] = None
    user_agent: Optional[str] = "decision-gate-crewai/0.1.0"
    validate: bool = False

    def create_client(self) -> DecisionGateClient:
        return DecisionGateClient(
            endpoint=self.endpoint,
            auth_token=self.auth_token,
            timeout_sec=self.timeout_sec,
            headers=self.headers,
            user_agent=self.user_agent,
        )


class _PrecheckInput(BaseModel):
    request: Dict[str, Any] = Field(..., description="Decision Gate precheck request.")


class _ScenarioNextInput(BaseModel):
    request: Dict[str, Any] = Field(..., description="Decision Gate scenario_next request.")


class _ScenarioStatusInput(BaseModel):
    request: Dict[str, Any] = Field(..., description="Decision Gate scenario_status request.")


class _ScenarioDefineInput(BaseModel):
    request: Dict[str, Any] = Field(..., description="Decision Gate scenario_define request.")


class _ScenarioStartInput(BaseModel):
    request: Dict[str, Any] = Field(..., description="Decision Gate scenario_start request.")


class _ScenarioTriggerInput(BaseModel):
    request: Dict[str, Any] = Field(..., description="Decision Gate scenario_trigger request.")


class _RunpackExportInput(BaseModel):
    request: Dict[str, Any] = Field(..., description="Decision Gate runpack_export request.")


class DecisionGatePrecheckTool(BaseTool):
    name: str = "decision_gate_precheck"
    description: str = "Run a Decision Gate precheck without mutating run state."
    args_schema: Type[BaseModel] = _PrecheckInput

    _client: DecisionGateClient = PrivateAttr()
    _validate: bool = PrivateAttr(default=False)

    def __init__(self, client: DecisionGateClient, validate: bool = False, **kwargs):
        super().__init__(**kwargs)
        self._client = client
        self._validate = validate

    def _run(self, request: Dict[str, Any]) -> str:
        _maybe_validate(self._validate, validate_precheck_request, request)
        return _as_json(self._client.precheck(request))


class DecisionGateScenarioNextTool(BaseTool):
    name: str = "decision_gate_scenario_next"
    description: str = "Advance a Decision Gate run to the next stage if gates pass."
    args_schema: Type[BaseModel] = _ScenarioNextInput

    _client: DecisionGateClient = PrivateAttr()
    _validate: bool = PrivateAttr(default=False)

    def __init__(self, client: DecisionGateClient, validate: bool = False, **kwargs):
        super().__init__(**kwargs)
        self._client = client
        self._validate = validate

    def _run(self, request: Dict[str, Any]) -> str:
        _maybe_validate(self._validate, validate_scenario_next_request, request)
        return _as_json(self._client.scenario_next(request))


class DecisionGateScenarioStatusTool(BaseTool):
    name: str = "decision_gate_scenario_status"
    description: str = "Fetch current Decision Gate run status without mutation."
    args_schema: Type[BaseModel] = _ScenarioStatusInput

    _client: DecisionGateClient = PrivateAttr()
    _validate: bool = PrivateAttr(default=False)

    def __init__(self, client: DecisionGateClient, validate: bool = False, **kwargs):
        super().__init__(**kwargs)
        self._client = client
        self._validate = validate

    def _run(self, request: Dict[str, Any]) -> str:
        _maybe_validate(self._validate, validate_scenario_status_request, request)
        return _as_json(self._client.scenario_status(request))


class DecisionGateScenarioDefineTool(BaseTool):
    name: str = "decision_gate_scenario_define"
    description: str = "Register a ScenarioSpec before starting a run."
    args_schema: Type[BaseModel] = _ScenarioDefineInput

    _client: DecisionGateClient = PrivateAttr()
    _validate: bool = PrivateAttr(default=False)

    def __init__(self, client: DecisionGateClient, validate: bool = False, **kwargs):
        super().__init__(**kwargs)
        self._client = client
        self._validate = validate

    def _run(self, request: Dict[str, Any]) -> str:
        _maybe_validate(self._validate, validate_scenario_define_request, request)
        return _as_json(self._client.scenario_define(request))


class DecisionGateScenarioStartTool(BaseTool):
    name: str = "decision_gate_scenario_start"
    description: str = "Start a new run for a registered scenario."
    args_schema: Type[BaseModel] = _ScenarioStartInput

    _client: DecisionGateClient = PrivateAttr()
    _validate: bool = PrivateAttr(default=False)

    def __init__(self, client: DecisionGateClient, validate: bool = False, **kwargs):
        super().__init__(**kwargs)
        self._client = client
        self._validate = validate

    def _run(self, request: Dict[str, Any]) -> str:
        _maybe_validate(self._validate, validate_scenario_start_request, request)
        return _as_json(self._client.scenario_start(request))


class DecisionGateScenarioTriggerTool(BaseTool):
    name: str = "decision_gate_scenario_trigger"
    description: str = "Trigger a scenario evaluation with an external event."
    args_schema: Type[BaseModel] = _ScenarioTriggerInput

    _client: DecisionGateClient = PrivateAttr()
    _validate: bool = PrivateAttr(default=False)

    def __init__(self, client: DecisionGateClient, validate: bool = False, **kwargs):
        super().__init__(**kwargs)
        self._client = client
        self._validate = validate

    def _run(self, request: Dict[str, Any]) -> str:
        _maybe_validate(self._validate, validate_scenario_trigger_request, request)
        return _as_json(self._client.scenario_trigger(request))


class DecisionGateRunpackExportTool(BaseTool):
    name: str = "decision_gate_runpack_export"
    description: str = "Export an audit-grade runpack for a scenario run."
    args_schema: Type[BaseModel] = _RunpackExportInput

    _client: DecisionGateClient = PrivateAttr()
    _validate: bool = PrivateAttr(default=False)

    def __init__(self, client: DecisionGateClient, validate: bool = False, **kwargs):
        super().__init__(**kwargs)
        self._client = client
        self._validate = validate

    def _run(self, request: Dict[str, Any]) -> str:
        _maybe_validate(self._validate, validate_runpack_export_request, request)
        return _as_json(self._client.runpack_export(request))


def build_decision_gate_tools(
    client: DecisionGateClient,
    *,
    validate: bool = False,
):
    """Return CrewAI tools for core Decision Gate operations."""
    return [
        DecisionGatePrecheckTool(client=client, validate=validate),
        DecisionGateScenarioNextTool(client=client, validate=validate),
        DecisionGateScenarioStatusTool(client=client, validate=validate),
        DecisionGateScenarioDefineTool(client=client, validate=validate),
        DecisionGateScenarioStartTool(client=client, validate=validate),
        DecisionGateScenarioTriggerTool(client=client, validate=validate),
        DecisionGateRunpackExportTool(client=client, validate=validate),
    ]


def build_decision_gate_tools_from_config(config: DecisionGateToolConfig):
    """Build tools using a config object instead of a prebuilt client."""
    client = config.create_client()
    return build_decision_gate_tools(client, validate=config.validate)
