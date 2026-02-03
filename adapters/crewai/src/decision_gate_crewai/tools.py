# adapters/crewai/src/decision_gate_crewai/tools.py
# ============================================================================
# Module: Decision Gate CrewAI Tools
# Description: Build CrewAI tools that call Decision Gate via the SDK.
# ============================================================================

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import TYPE_CHECKING, Callable, Mapping, Optional, Type, TypeVar, TypedDict, cast

from typing_extensions import Unpack

from crewai.tools import BaseTool
from pydantic import BaseModel, Field, PrivateAttr

from decision_gate import (
    DecisionGateClient,
    DecisionGateDocsSearchRequest,
    EvidenceQueryRequest,
    JsonValue,
    PrecheckRequest,
    ProviderCheckSchemaGetRequest,
    ProviderContractGetRequest,
    ProvidersListRequest,
    RunpackExportRequest,
    RunpackVerifyRequest,
    ScenarioDefineRequest,
    ScenarioNextRequest,
    ScenarioStartRequest,
    ScenarioStatusRequest,
    ScenarioSubmitRequest,
    ScenarioTriggerRequest,
    ScenariosListRequest,
    SchemasGetRequest,
    SchemasListRequest,
    SchemasRegisterRequest,
    validate_decision_gate_docs_search_request,
    validate_evidence_query_request,
    validate_precheck_request,
    validate_provider_check_schema_get_request,
    validate_provider_contract_get_request,
    validate_providers_list_request,
    validate_runpack_export_request,
    validate_runpack_verify_request,
    validate_scenario_define_request,
    validate_scenario_next_request,
    validate_scenario_start_request,
    validate_scenario_status_request,
    validate_scenario_submit_request,
    validate_scenario_trigger_request,
    validate_scenarios_list_request,
    validate_schemas_get_request,
    validate_schemas_list_request,
    validate_schemas_register_request,
)

TRequest = TypeVar("TRequest")

if TYPE_CHECKING:
    from crewai.tools import EnvVar
else:
    EnvVar = object


class _BaseToolKwargs(TypedDict, total=False):
    name: str
    description: str
    env_vars: list["EnvVar"]
    args_schema: Type[BaseModel]
    description_updated: bool
    cache_function: Callable[..., bool]
    result_as_answer: bool
    max_usage_count: int | None
    current_usage_count: int


def _maybe_validate(
    enabled: bool,
    validator: Callable[[TRequest], None],
    payload: TRequest,
) -> None:
    if enabled:
        validator(payload)


def _as_json(result: JsonValue) -> str:
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
    request: dict[str, JsonValue] = Field(..., description="Decision Gate precheck request.")


class _ScenarioNextInput(BaseModel):
    request: dict[str, JsonValue] = Field(..., description="Decision Gate scenario_next request.")


class _ScenarioStatusInput(BaseModel):
    request: dict[str, JsonValue] = Field(..., description="Decision Gate scenario_status request.")


class _ScenarioDefineInput(BaseModel):
    request: dict[str, JsonValue] = Field(..., description="Decision Gate scenario_define request.")


class _ScenarioStartInput(BaseModel):
    request: dict[str, JsonValue] = Field(..., description="Decision Gate scenario_start request.")


class _ScenarioTriggerInput(BaseModel):
    request: dict[str, JsonValue] = Field(
        ..., description="Decision Gate scenario_trigger request."
    )


class _ScenarioSubmitInput(BaseModel):
    request: dict[str, JsonValue] = Field(..., description="Decision Gate scenario_submit request.")


class _ScenariosListInput(BaseModel):
    request: dict[str, JsonValue] = Field(..., description="Decision Gate scenarios_list request.")


class _EvidenceQueryInput(BaseModel):
    request: dict[str, JsonValue] = Field(..., description="Decision Gate evidence_query request.")


class _RunpackExportInput(BaseModel):
    request: dict[str, JsonValue] = Field(..., description="Decision Gate runpack_export request.")


class _RunpackVerifyInput(BaseModel):
    request: dict[str, JsonValue] = Field(..., description="Decision Gate runpack_verify request.")


class _ProvidersListInput(BaseModel):
    request: dict[str, JsonValue] = Field(..., description="Decision Gate providers_list request.")


class _ProviderContractGetInput(BaseModel):
    request: dict[str, JsonValue] = Field(
        ..., description="Decision Gate provider_contract_get request."
    )


class _ProviderCheckSchemaGetInput(BaseModel):
    request: dict[str, JsonValue] = Field(
        ..., description="Decision Gate provider_check_schema_get request."
    )


class _SchemasRegisterInput(BaseModel):
    request: dict[str, JsonValue] = Field(
        ..., description="Decision Gate schemas_register request."
    )


class _SchemasListInput(BaseModel):
    request: dict[str, JsonValue] = Field(..., description="Decision Gate schemas_list request.")


class _SchemasGetInput(BaseModel):
    request: dict[str, JsonValue] = Field(..., description="Decision Gate schemas_get request.")


class _DocsSearchInput(BaseModel):
    request: dict[str, JsonValue] = Field(
        ..., description="Decision Gate decision_gate_docs_search request."
    )


class DecisionGatePrecheckTool(BaseTool):
    name: str = "decision_gate_precheck"
    description: str = "Run a Decision Gate precheck without mutating run state."
    args_schema: Type[BaseModel] = _PrecheckInput

    _client: DecisionGateClient = PrivateAttr()
    _validate: bool = PrivateAttr(default=False)

    def __init__(
        self,
        client: DecisionGateClient,
        validate: bool = False,
        **kwargs: Unpack[_BaseToolKwargs],
    ):
        super().__init__(**kwargs)
        self._client = client
        self._validate = validate

    def _run(self, request: PrecheckRequest) -> str:
        _maybe_validate(self._validate, validate_precheck_request, request)
        return _as_json(cast(JsonValue, self._client.precheck(request)))


class DecisionGateScenarioNextTool(BaseTool):
    name: str = "decision_gate_scenario_next"
    description: str = "Advance a Decision Gate run to the next stage if gates pass."
    args_schema: Type[BaseModel] = _ScenarioNextInput

    _client: DecisionGateClient = PrivateAttr()
    _validate: bool = PrivateAttr(default=False)

    def __init__(
        self,
        client: DecisionGateClient,
        validate: bool = False,
        **kwargs: Unpack[_BaseToolKwargs],
    ):
        super().__init__(**kwargs)
        self._client = client
        self._validate = validate

    def _run(self, request: ScenarioNextRequest) -> str:
        _maybe_validate(self._validate, validate_scenario_next_request, request)
        return _as_json(cast(JsonValue, self._client.scenario_next(request)))


class DecisionGateScenarioStatusTool(BaseTool):
    name: str = "decision_gate_scenario_status"
    description: str = "Fetch current Decision Gate run status without mutation."
    args_schema: Type[BaseModel] = _ScenarioStatusInput

    _client: DecisionGateClient = PrivateAttr()
    _validate: bool = PrivateAttr(default=False)

    def __init__(
        self,
        client: DecisionGateClient,
        validate: bool = False,
        **kwargs: Unpack[_BaseToolKwargs],
    ):
        super().__init__(**kwargs)
        self._client = client
        self._validate = validate

    def _run(self, request: ScenarioStatusRequest) -> str:
        _maybe_validate(self._validate, validate_scenario_status_request, request)
        return _as_json(cast(JsonValue, self._client.scenario_status(request)))


class DecisionGateScenarioDefineTool(BaseTool):
    name: str = "decision_gate_scenario_define"
    description: str = "Register a ScenarioSpec before starting a run."
    args_schema: Type[BaseModel] = _ScenarioDefineInput

    _client: DecisionGateClient = PrivateAttr()
    _validate: bool = PrivateAttr(default=False)

    def __init__(
        self,
        client: DecisionGateClient,
        validate: bool = False,
        **kwargs: Unpack[_BaseToolKwargs],
    ):
        super().__init__(**kwargs)
        self._client = client
        self._validate = validate

    def _run(self, request: ScenarioDefineRequest) -> str:
        _maybe_validate(self._validate, validate_scenario_define_request, request)
        return _as_json(cast(JsonValue, self._client.scenario_define(request)))


class DecisionGateScenarioStartTool(BaseTool):
    name: str = "decision_gate_scenario_start"
    description: str = "Start a new run for a registered scenario."
    args_schema: Type[BaseModel] = _ScenarioStartInput

    _client: DecisionGateClient = PrivateAttr()
    _validate: bool = PrivateAttr(default=False)

    def __init__(
        self,
        client: DecisionGateClient,
        validate: bool = False,
        **kwargs: Unpack[_BaseToolKwargs],
    ):
        super().__init__(**kwargs)
        self._client = client
        self._validate = validate

    def _run(self, request: ScenarioStartRequest) -> str:
        _maybe_validate(self._validate, validate_scenario_start_request, request)
        return _as_json(cast(JsonValue, self._client.scenario_start(request)))


class DecisionGateScenarioTriggerTool(BaseTool):
    name: str = "decision_gate_scenario_trigger"
    description: str = "Trigger a scenario evaluation with an external event."
    args_schema: Type[BaseModel] = _ScenarioTriggerInput

    _client: DecisionGateClient = PrivateAttr()
    _validate: bool = PrivateAttr(default=False)

    def __init__(
        self,
        client: DecisionGateClient,
        validate: bool = False,
        **kwargs: Unpack[_BaseToolKwargs],
    ):
        super().__init__(**kwargs)
        self._client = client
        self._validate = validate

    def _run(self, request: ScenarioTriggerRequest) -> str:
        _maybe_validate(self._validate, validate_scenario_trigger_request, request)
        return _as_json(cast(JsonValue, self._client.scenario_trigger(request)))


class DecisionGateScenarioSubmitTool(BaseTool):
    name: str = "decision_gate_scenario_submit"
    description: str = "Submit the current stage decision for a scenario run."
    args_schema: Type[BaseModel] = _ScenarioSubmitInput

    _client: DecisionGateClient = PrivateAttr()
    _validate: bool = PrivateAttr(default=False)

    def __init__(
        self,
        client: DecisionGateClient,
        validate: bool = False,
        **kwargs: Unpack[_BaseToolKwargs],
    ):
        super().__init__(**kwargs)
        self._client = client
        self._validate = validate

    def _run(self, request: ScenarioSubmitRequest) -> str:
        _maybe_validate(self._validate, validate_scenario_submit_request, request)
        return _as_json(cast(JsonValue, self._client.scenario_submit(request)))


class DecisionGateScenariosListTool(BaseTool):
    name: str = "decision_gate_scenarios_list"
    description: str = "List registered scenarios for a tenant and namespace."
    args_schema: Type[BaseModel] = _ScenariosListInput

    _client: DecisionGateClient = PrivateAttr()
    _validate: bool = PrivateAttr(default=False)

    def __init__(
        self,
        client: DecisionGateClient,
        validate: bool = False,
        **kwargs: Unpack[_BaseToolKwargs],
    ):
        super().__init__(**kwargs)
        self._client = client
        self._validate = validate

    def _run(self, request: ScenariosListRequest) -> str:
        _maybe_validate(self._validate, validate_scenarios_list_request, request)
        return _as_json(cast(JsonValue, self._client.scenarios_list(request)))


class DecisionGateEvidenceQueryTool(BaseTool):
    name: str = "decision_gate_evidence_query"
    description: str = "Query evidence providers for condition inputs."
    args_schema: Type[BaseModel] = _EvidenceQueryInput

    _client: DecisionGateClient = PrivateAttr()
    _validate: bool = PrivateAttr(default=False)

    def __init__(
        self,
        client: DecisionGateClient,
        validate: bool = False,
        **kwargs: Unpack[_BaseToolKwargs],
    ):
        super().__init__(**kwargs)
        self._client = client
        self._validate = validate

    def _run(self, request: EvidenceQueryRequest) -> str:
        _maybe_validate(self._validate, validate_evidence_query_request, request)
        return _as_json(cast(JsonValue, self._client.evidence_query(request)))


class DecisionGateRunpackExportTool(BaseTool):
    name: str = "decision_gate_runpack_export"
    description: str = "Export an audit-grade runpack for a scenario run."
    args_schema: Type[BaseModel] = _RunpackExportInput

    _client: DecisionGateClient = PrivateAttr()
    _validate: bool = PrivateAttr(default=False)

    def __init__(
        self,
        client: DecisionGateClient,
        validate: bool = False,
        **kwargs: Unpack[_BaseToolKwargs],
    ):
        super().__init__(**kwargs)
        self._client = client
        self._validate = validate

    def _run(self, request: RunpackExportRequest) -> str:
        _maybe_validate(self._validate, validate_runpack_export_request, request)
        return _as_json(cast(JsonValue, self._client.runpack_export(request)))


class DecisionGateRunpackVerifyTool(BaseTool):
    name: str = "decision_gate_runpack_verify"
    description: str = "Verify a runpack manifest against expected hashes."
    args_schema: Type[BaseModel] = _RunpackVerifyInput

    _client: DecisionGateClient = PrivateAttr()
    _validate: bool = PrivateAttr(default=False)

    def __init__(
        self,
        client: DecisionGateClient,
        validate: bool = False,
        **kwargs: Unpack[_BaseToolKwargs],
    ):
        super().__init__(**kwargs)
        self._client = client
        self._validate = validate

    def _run(self, request: RunpackVerifyRequest) -> str:
        _maybe_validate(self._validate, validate_runpack_verify_request, request)
        return _as_json(cast(JsonValue, self._client.runpack_verify(request)))


class DecisionGateProvidersListTool(BaseTool):
    name: str = "decision_gate_providers_list"
    description: str = "List registered evidence providers and their capabilities."
    args_schema: Type[BaseModel] = _ProvidersListInput

    _client: DecisionGateClient = PrivateAttr()
    _validate: bool = PrivateAttr(default=False)

    def __init__(
        self,
        client: DecisionGateClient,
        validate: bool = False,
        **kwargs: Unpack[_BaseToolKwargs],
    ):
        super().__init__(**kwargs)
        self._client = client
        self._validate = validate

    def _run(self, request: ProvidersListRequest) -> str:
        _maybe_validate(self._validate, validate_providers_list_request, request)
        return _as_json(cast(JsonValue, self._client.providers_list(request)))


class DecisionGateProviderContractGetTool(BaseTool):
    name: str = "decision_gate_provider_contract_get"
    description: str = "Fetch a provider contract payload."
    args_schema: Type[BaseModel] = _ProviderContractGetInput

    _client: DecisionGateClient = PrivateAttr()
    _validate: bool = PrivateAttr(default=False)

    def __init__(
        self,
        client: DecisionGateClient,
        validate: bool = False,
        **kwargs: Unpack[_BaseToolKwargs],
    ):
        super().__init__(**kwargs)
        self._client = client
        self._validate = validate

    def _run(self, request: ProviderContractGetRequest) -> str:
        _maybe_validate(self._validate, validate_provider_contract_get_request, request)
        return _as_json(cast(JsonValue, self._client.provider_contract_get(request)))


class DecisionGateProviderCheckSchemaGetTool(BaseTool):
    name: str = "decision_gate_provider_check_schema_get"
    description: str = "Fetch a provider check schema."
    args_schema: Type[BaseModel] = _ProviderCheckSchemaGetInput

    _client: DecisionGateClient = PrivateAttr()
    _validate: bool = PrivateAttr(default=False)

    def __init__(
        self,
        client: DecisionGateClient,
        validate: bool = False,
        **kwargs: Unpack[_BaseToolKwargs],
    ):
        super().__init__(**kwargs)
        self._client = client
        self._validate = validate

    def _run(self, request: ProviderCheckSchemaGetRequest) -> str:
        _maybe_validate(self._validate, validate_provider_check_schema_get_request, request)
        return _as_json(cast(JsonValue, self._client.provider_check_schema_get(request)))


class DecisionGateSchemasRegisterTool(BaseTool):
    name: str = "decision_gate_schemas_register"
    description: str = "Register a data shape schema."
    args_schema: Type[BaseModel] = _SchemasRegisterInput

    _client: DecisionGateClient = PrivateAttr()
    _validate: bool = PrivateAttr(default=False)

    def __init__(
        self,
        client: DecisionGateClient,
        validate: bool = False,
        **kwargs: Unpack[_BaseToolKwargs],
    ):
        super().__init__(**kwargs)
        self._client = client
        self._validate = validate

    def _run(self, request: SchemasRegisterRequest) -> str:
        _maybe_validate(self._validate, validate_schemas_register_request, request)
        return _as_json(cast(JsonValue, self._client.schemas_register(request)))


class DecisionGateSchemasListTool(BaseTool):
    name: str = "decision_gate_schemas_list"
    description: str = "List registered data shape schemas."
    args_schema: Type[BaseModel] = _SchemasListInput

    _client: DecisionGateClient = PrivateAttr()
    _validate: bool = PrivateAttr(default=False)

    def __init__(
        self,
        client: DecisionGateClient,
        validate: bool = False,
        **kwargs: Unpack[_BaseToolKwargs],
    ):
        super().__init__(**kwargs)
        self._client = client
        self._validate = validate

    def _run(self, request: SchemasListRequest) -> str:
        _maybe_validate(self._validate, validate_schemas_list_request, request)
        return _as_json(cast(JsonValue, self._client.schemas_list(request)))


class DecisionGateSchemasGetTool(BaseTool):
    name: str = "decision_gate_schemas_get"
    description: str = "Fetch a registered data shape schema."
    args_schema: Type[BaseModel] = _SchemasGetInput

    _client: DecisionGateClient = PrivateAttr()
    _validate: bool = PrivateAttr(default=False)

    def __init__(
        self,
        client: DecisionGateClient,
        validate: bool = False,
        **kwargs: Unpack[_BaseToolKwargs],
    ):
        super().__init__(**kwargs)
        self._client = client
        self._validate = validate

    def _run(self, request: SchemasGetRequest) -> str:
        _maybe_validate(self._validate, validate_schemas_get_request, request)
        return _as_json(cast(JsonValue, self._client.schemas_get(request)))


class DecisionGateDocsSearchTool(BaseTool):
    name: str = "decision_gate_docs_search"
    description: str = "Search Decision Gate documentation for runtime guidance."
    args_schema: Type[BaseModel] = _DocsSearchInput

    _client: DecisionGateClient = PrivateAttr()
    _validate: bool = PrivateAttr(default=False)

    def __init__(
        self,
        client: DecisionGateClient,
        validate: bool = False,
        **kwargs: Unpack[_BaseToolKwargs],
    ):
        super().__init__(**kwargs)
        self._client = client
        self._validate = validate

    def _run(self, request: DecisionGateDocsSearchRequest) -> str:
        _maybe_validate(self._validate, validate_decision_gate_docs_search_request, request)
        return _as_json(cast(JsonValue, self._client.decision_gate_docs_search(request)))


def build_decision_gate_tools(
    client: DecisionGateClient,
    *,
    validate: bool = False,
) -> list[BaseTool]:
    """Return CrewAI tools for core Decision Gate operations."""
    return [
        DecisionGateScenarioDefineTool(client=client, validate=validate),
        DecisionGateScenarioStartTool(client=client, validate=validate),
        DecisionGateScenarioStatusTool(client=client, validate=validate),
        DecisionGateScenarioNextTool(client=client, validate=validate),
        DecisionGateScenarioSubmitTool(client=client, validate=validate),
        DecisionGateScenarioTriggerTool(client=client, validate=validate),
        DecisionGateScenariosListTool(client=client, validate=validate),
        DecisionGateEvidenceQueryTool(client=client, validate=validate),
        DecisionGatePrecheckTool(client=client, validate=validate),
        DecisionGateRunpackExportTool(client=client, validate=validate),
        DecisionGateRunpackVerifyTool(client=client, validate=validate),
        DecisionGateProvidersListTool(client=client, validate=validate),
        DecisionGateProviderContractGetTool(client=client, validate=validate),
        DecisionGateProviderCheckSchemaGetTool(client=client, validate=validate),
        DecisionGateSchemasRegisterTool(client=client, validate=validate),
        DecisionGateSchemasListTool(client=client, validate=validate),
        DecisionGateSchemasGetTool(client=client, validate=validate),
        DecisionGateDocsSearchTool(client=client, validate=validate),
    ]


def build_decision_gate_tools_from_config(config: DecisionGateToolConfig) -> list[BaseTool]:
    """Build tools using a config object instead of a prebuilt client."""
    client = config.create_client()
    return build_decision_gate_tools(client, validate=config.validate)
