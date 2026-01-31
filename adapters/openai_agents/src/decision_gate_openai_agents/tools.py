# adapters/openai_agents/src/decision_gate_openai_agents/tools.py
# ============================================================================
# Module: Decision Gate OpenAI Agents Tools
# Description: Build OpenAI Agents function tools that call Decision Gate.
# ============================================================================

from __future__ import annotations

from dataclasses import dataclass
import importlib
from inspect import signature
from typing import Callable, List, Mapping, Optional, TypeVar, cast

from decision_gate import (
    DecisionGateClient,
    DecisionGateDocsSearchRequest,
    DecisionGateDocsSearchResponse,
    EvidenceQueryRequest,
    EvidenceQueryResponse,
    JsonValue,
    PrecheckRequest,
    PrecheckResponse,
    ProviderCheckSchemaGetRequest,
    ProviderCheckSchemaGetResponse,
    ProviderContractGetRequest,
    ProviderContractGetResponse,
    ProvidersListRequest,
    ProvidersListResponse,
    RunpackExportRequest,
    RunpackExportResponse,
    RunpackVerifyRequest,
    RunpackVerifyResponse,
    ScenarioDefineRequest,
    ScenarioDefineResponse,
    ScenarioNextRequest,
    ScenarioNextResponse,
    ScenarioStartRequest,
    ScenarioStartResponse,
    ScenarioStatusRequest,
    ScenarioStatusResponse,
    ScenarioSubmitRequest,
    ScenarioSubmitResponse,
    ScenarioTriggerRequest,
    ScenarioTriggerResponse,
    ScenariosListRequest,
    ScenariosListResponse,
    SchemasGetRequest,
    SchemasGetResponse,
    SchemasListRequest,
    SchemasListResponse,
    SchemasRegisterRequest,
    SchemasRegisterResponse,
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
ToolDecorator = Callable[[Callable[..., object]], object]


def _maybe_validate(enabled: bool, validator: Callable[[TRequest], None], payload: TRequest) -> None:
    if enabled:
        validator(payload)


def _dg_function_tool(name: str) -> ToolDecorator:
    """Return a function_tool decorator compatible with multiple SDK versions."""
    function_tool = cast(
        Callable[..., object],
        getattr(importlib.import_module("agents"), "function_tool"),
    )
    try:
        params = signature(function_tool).parameters
        kwargs = {}
        if "name" in params:
            kwargs["name"] = name
        elif "name_override" in params:
            kwargs["name_override"] = name
        if "strict_mode" in params:
            kwargs["strict_mode"] = False
        if kwargs:
            return cast(ToolDecorator, function_tool(**kwargs))
    except (TypeError, ValueError):
        # Fall back to default decorator when signature introspection fails.
        pass
    return cast(ToolDecorator, function_tool())


@dataclass(frozen=True)
class DecisionGateToolConfig:
    """Configuration helper for constructing Decision Gate OpenAI Agents tools."""

    endpoint: str = "http://127.0.0.1:8080/rpc"
    auth_token: Optional[str] = None
    timeout_sec: Optional[float] = 10.0
    headers: Optional[Mapping[str, str]] = None
    user_agent: Optional[str] = "decision-gate-openai-agents/0.1.0"
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
) -> List[object]:
    """Return OpenAI Agents function tools for core Decision Gate operations."""

    @_dg_function_tool("decision_gate_precheck")
    def decision_gate_precheck(request: dict[str, JsonValue]) -> PrecheckResponse:
        """Run a Decision Gate precheck without mutating run state."""
        typed_request = cast(PrecheckRequest, request)
        _maybe_validate(validate, validate_precheck_request, typed_request)
        return client.precheck(typed_request)

    @_dg_function_tool("decision_gate_scenario_define")
    def decision_gate_scenario_define(request: dict[str, JsonValue]) -> ScenarioDefineResponse:
        """Register a ScenarioSpec before starting a run."""
        typed_request = cast(ScenarioDefineRequest, request)
        _maybe_validate(validate, validate_scenario_define_request, typed_request)
        return client.scenario_define(typed_request)

    @_dg_function_tool("decision_gate_scenario_start")
    def decision_gate_scenario_start(request: dict[str, JsonValue]) -> ScenarioStartResponse:
        """Start a new run for a registered scenario."""
        typed_request = cast(ScenarioStartRequest, request)
        _maybe_validate(validate, validate_scenario_start_request, typed_request)
        return client.scenario_start(typed_request)

    @_dg_function_tool("decision_gate_scenario_status")
    def decision_gate_scenario_status(request: dict[str, JsonValue]) -> ScenarioStatusResponse:
        """Fetch current Decision Gate run status without mutation."""
        typed_request = cast(ScenarioStatusRequest, request)
        _maybe_validate(validate, validate_scenario_status_request, typed_request)
        return client.scenario_status(typed_request)

    @_dg_function_tool("decision_gate_scenario_next")
    def decision_gate_scenario_next(request: dict[str, JsonValue]) -> ScenarioNextResponse:
        """Advance a Decision Gate run to the next stage if gates pass."""
        typed_request = cast(ScenarioNextRequest, request)
        _maybe_validate(validate, validate_scenario_next_request, typed_request)
        return client.scenario_next(typed_request)

    @_dg_function_tool("decision_gate_scenario_submit")
    def decision_gate_scenario_submit(request: dict[str, JsonValue]) -> ScenarioSubmitResponse:
        """Submit the current stage decision for a scenario run."""
        typed_request = cast(ScenarioSubmitRequest, request)
        _maybe_validate(validate, validate_scenario_submit_request, typed_request)
        return client.scenario_submit(typed_request)

    @_dg_function_tool("decision_gate_scenario_trigger")
    def decision_gate_scenario_trigger(request: dict[str, JsonValue]) -> ScenarioTriggerResponse:
        """Trigger a scenario evaluation with an external event."""
        typed_request = cast(ScenarioTriggerRequest, request)
        _maybe_validate(validate, validate_scenario_trigger_request, typed_request)
        return client.scenario_trigger(typed_request)

    @_dg_function_tool("decision_gate_scenarios_list")
    def decision_gate_scenarios_list(request: dict[str, JsonValue]) -> ScenariosListResponse:
        """List registered scenarios for a tenant and namespace."""
        typed_request = cast(ScenariosListRequest, request)
        _maybe_validate(validate, validate_scenarios_list_request, typed_request)
        return client.scenarios_list(typed_request)

    @_dg_function_tool("decision_gate_evidence_query")
    def decision_gate_evidence_query(request: dict[str, JsonValue]) -> EvidenceQueryResponse:
        """Query evidence providers for condition inputs."""
        typed_request = cast(EvidenceQueryRequest, request)
        _maybe_validate(validate, validate_evidence_query_request, typed_request)
        return client.evidence_query(typed_request)

    @_dg_function_tool("decision_gate_runpack_export")
    def decision_gate_runpack_export(request: dict[str, JsonValue]) -> RunpackExportResponse:
        """Export an audit-grade runpack for a scenario run."""
        typed_request = cast(RunpackExportRequest, request)
        _maybe_validate(validate, validate_runpack_export_request, typed_request)
        return client.runpack_export(typed_request)

    @_dg_function_tool("decision_gate_runpack_verify")
    def decision_gate_runpack_verify(request: dict[str, JsonValue]) -> RunpackVerifyResponse:
        """Verify a runpack manifest against expected hashes."""
        typed_request = cast(RunpackVerifyRequest, request)
        _maybe_validate(validate, validate_runpack_verify_request, typed_request)
        return client.runpack_verify(typed_request)

    @_dg_function_tool("decision_gate_providers_list")
    def decision_gate_providers_list(request: dict[str, JsonValue]) -> ProvidersListResponse:
        """List registered evidence providers and their capabilities."""
        typed_request = cast(ProvidersListRequest, request)
        _maybe_validate(validate, validate_providers_list_request, typed_request)
        return client.providers_list(typed_request)

    @_dg_function_tool("decision_gate_provider_contract_get")
    def decision_gate_provider_contract_get(
        request: dict[str, JsonValue],
    ) -> ProviderContractGetResponse:
        """Fetch a provider contract payload."""
        typed_request = cast(ProviderContractGetRequest, request)
        _maybe_validate(validate, validate_provider_contract_get_request, typed_request)
        return client.provider_contract_get(typed_request)

    @_dg_function_tool("decision_gate_provider_check_schema_get")
    def decision_gate_provider_check_schema_get(
        request: dict[str, JsonValue],
    ) -> ProviderCheckSchemaGetResponse:
        """Fetch a provider check schema."""
        typed_request = cast(ProviderCheckSchemaGetRequest, request)
        _maybe_validate(validate, validate_provider_check_schema_get_request, typed_request)
        return client.provider_check_schema_get(typed_request)

    @_dg_function_tool("decision_gate_schemas_register")
    def decision_gate_schemas_register(request: dict[str, JsonValue]) -> SchemasRegisterResponse:
        """Register a data shape schema."""
        typed_request = cast(SchemasRegisterRequest, request)
        _maybe_validate(validate, validate_schemas_register_request, typed_request)
        return client.schemas_register(typed_request)

    @_dg_function_tool("decision_gate_schemas_list")
    def decision_gate_schemas_list(request: dict[str, JsonValue]) -> SchemasListResponse:
        """List registered data shape schemas."""
        typed_request = cast(SchemasListRequest, request)
        _maybe_validate(validate, validate_schemas_list_request, typed_request)
        return client.schemas_list(typed_request)

    @_dg_function_tool("decision_gate_schemas_get")
    def decision_gate_schemas_get(request: dict[str, JsonValue]) -> SchemasGetResponse:
        """Fetch a registered data shape schema."""
        typed_request = cast(SchemasGetRequest, request)
        _maybe_validate(validate, validate_schemas_get_request, typed_request)
        return client.schemas_get(typed_request)

    @_dg_function_tool("decision_gate_docs_search")
    def decision_gate_docs_search(
        request: dict[str, JsonValue],
    ) -> DecisionGateDocsSearchResponse:
        """Search Decision Gate documentation for runtime guidance."""
        typed_request = cast(DecisionGateDocsSearchRequest, request)
        _maybe_validate(validate, validate_decision_gate_docs_search_request, typed_request)
        return client.decision_gate_docs_search(typed_request)

    return [
        decision_gate_precheck,
        decision_gate_scenario_define,
        decision_gate_scenario_start,
        decision_gate_scenario_status,
        decision_gate_scenario_next,
        decision_gate_scenario_submit,
        decision_gate_scenario_trigger,
        decision_gate_scenarios_list,
        decision_gate_evidence_query,
        decision_gate_runpack_export,
        decision_gate_runpack_verify,
        decision_gate_providers_list,
        decision_gate_provider_contract_get,
        decision_gate_provider_check_schema_get,
        decision_gate_schemas_register,
        decision_gate_schemas_list,
        decision_gate_schemas_get,
        decision_gate_docs_search,
    ]


def build_decision_gate_tools_from_config(
    config: DecisionGateToolConfig,
) -> List[object]:
    """Build tools using a config object instead of a prebuilt client."""
    client = config.create_client()
    return build_decision_gate_tools(client, validate=config.validate)
