# adapters/langchain/src/decision_gate_langchain/tools.py
# ============================================================================
# Module: Decision Gate LangChain Tools
# Description: Build LangChain tools that call Decision Gate via the SDK.
# ============================================================================

from __future__ import annotations

from dataclasses import dataclass
import importlib
from typing import Callable, Mapping, Optional, Protocol, TypeVar, cast

from pydantic import BaseModel, Field

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
class BaseToolLike(Protocol):
    name: str


def _tool(
    name: str,
    *,
    args_schema: type[BaseModel],
) -> Callable[[Callable[..., object]], BaseToolLike]:
    langchain_module = importlib.import_module("langchain_core.tools")
    langchain_tool = cast(Callable[..., object], getattr(langchain_module, "tool"))
    decorator = langchain_tool(name, args_schema=args_schema)
    return cast(Callable[[Callable[..., object]], BaseToolLike], decorator)


def _maybe_validate(enabled: bool, validator: Callable[[TRequest], None], payload: TRequest) -> None:
    if enabled:
        validator(payload)


class DecisionGateToolArgs(BaseModel):
    """LangChain args schema wrapper for JSON payloads."""

    request: dict[str, JsonValue] = Field(
        ..., description="Decision Gate tool request payload."
    )


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
) -> list[BaseToolLike]:
    """Return LangChain tools for core Decision Gate operations."""

    @_tool("decision_gate_precheck", args_schema=DecisionGateToolArgs)
    def decision_gate_precheck(request: PrecheckRequest) -> PrecheckResponse:
        """Run a Decision Gate precheck without mutating run state."""
        _maybe_validate(validate, validate_precheck_request, request)
        return client.precheck(request)

    @_tool("decision_gate_scenario_define", args_schema=DecisionGateToolArgs)
    def decision_gate_scenario_define(request: ScenarioDefineRequest) -> ScenarioDefineResponse:
        """Register a ScenarioSpec before starting a run."""
        _maybe_validate(validate, validate_scenario_define_request, request)
        return client.scenario_define(request)

    @_tool("decision_gate_scenario_start", args_schema=DecisionGateToolArgs)
    def decision_gate_scenario_start(request: ScenarioStartRequest) -> ScenarioStartResponse:
        """Start a new run for a registered scenario."""
        _maybe_validate(validate, validate_scenario_start_request, request)
        return client.scenario_start(request)

    @_tool("decision_gate_scenario_status", args_schema=DecisionGateToolArgs)
    def decision_gate_scenario_status(request: ScenarioStatusRequest) -> ScenarioStatusResponse:
        """Fetch current Decision Gate run status without mutation."""
        _maybe_validate(validate, validate_scenario_status_request, request)
        return client.scenario_status(request)

    @_tool("decision_gate_scenario_next", args_schema=DecisionGateToolArgs)
    def decision_gate_scenario_next(request: ScenarioNextRequest) -> ScenarioNextResponse:
        """Advance a Decision Gate run to the next stage if gates pass."""
        _maybe_validate(validate, validate_scenario_next_request, request)
        return client.scenario_next(request)

    @_tool("decision_gate_scenario_submit", args_schema=DecisionGateToolArgs)
    def decision_gate_scenario_submit(request: ScenarioSubmitRequest) -> ScenarioSubmitResponse:
        """Submit the current stage decision for a scenario run."""
        _maybe_validate(validate, validate_scenario_submit_request, request)
        return client.scenario_submit(request)

    @_tool("decision_gate_scenario_trigger", args_schema=DecisionGateToolArgs)
    def decision_gate_scenario_trigger(request: ScenarioTriggerRequest) -> ScenarioTriggerResponse:
        """Trigger a scenario evaluation with an external event."""
        _maybe_validate(validate, validate_scenario_trigger_request, request)
        return client.scenario_trigger(request)

    @_tool("decision_gate_scenarios_list", args_schema=DecisionGateToolArgs)
    def decision_gate_scenarios_list(request: ScenariosListRequest) -> ScenariosListResponse:
        """List registered scenarios for a tenant and namespace."""
        _maybe_validate(validate, validate_scenarios_list_request, request)
        return client.scenarios_list(request)

    @_tool("decision_gate_evidence_query", args_schema=DecisionGateToolArgs)
    def decision_gate_evidence_query(request: EvidenceQueryRequest) -> EvidenceQueryResponse:
        """Query evidence providers for condition inputs."""
        _maybe_validate(validate, validate_evidence_query_request, request)
        return client.evidence_query(request)

    @_tool("decision_gate_runpack_export", args_schema=DecisionGateToolArgs)
    def decision_gate_runpack_export(request: RunpackExportRequest) -> RunpackExportResponse:
        """Export an audit-grade runpack for a scenario run."""
        _maybe_validate(validate, validate_runpack_export_request, request)
        return client.runpack_export(request)

    @_tool("decision_gate_runpack_verify", args_schema=DecisionGateToolArgs)
    def decision_gate_runpack_verify(request: RunpackVerifyRequest) -> RunpackVerifyResponse:
        """Verify a runpack manifest against expected hashes."""
        _maybe_validate(validate, validate_runpack_verify_request, request)
        return client.runpack_verify(request)

    @_tool("decision_gate_providers_list", args_schema=DecisionGateToolArgs)
    def decision_gate_providers_list(request: ProvidersListRequest) -> ProvidersListResponse:
        """List registered evidence providers and their capabilities."""
        _maybe_validate(validate, validate_providers_list_request, request)
        return client.providers_list(request)

    @_tool("decision_gate_provider_contract_get", args_schema=DecisionGateToolArgs)
    def decision_gate_provider_contract_get(
        request: ProviderContractGetRequest,
    ) -> ProviderContractGetResponse:
        """Fetch a provider contract payload."""
        _maybe_validate(validate, validate_provider_contract_get_request, request)
        return client.provider_contract_get(request)

    @_tool("decision_gate_provider_check_schema_get", args_schema=DecisionGateToolArgs)
    def decision_gate_provider_check_schema_get(
        request: ProviderCheckSchemaGetRequest,
    ) -> ProviderCheckSchemaGetResponse:
        """Fetch a provider check schema."""
        _maybe_validate(validate, validate_provider_check_schema_get_request, request)
        return client.provider_check_schema_get(request)

    @_tool("decision_gate_schemas_register", args_schema=DecisionGateToolArgs)
    def decision_gate_schemas_register(request: SchemasRegisterRequest) -> SchemasRegisterResponse:
        """Register a data shape schema."""
        _maybe_validate(validate, validate_schemas_register_request, request)
        return client.schemas_register(request)

    @_tool("decision_gate_schemas_list", args_schema=DecisionGateToolArgs)
    def decision_gate_schemas_list(request: SchemasListRequest) -> SchemasListResponse:
        """List registered data shape schemas."""
        _maybe_validate(validate, validate_schemas_list_request, request)
        return client.schemas_list(request)

    @_tool("decision_gate_schemas_get", args_schema=DecisionGateToolArgs)
    def decision_gate_schemas_get(request: SchemasGetRequest) -> SchemasGetResponse:
        """Fetch a registered data shape schema."""
        _maybe_validate(validate, validate_schemas_get_request, request)
        return client.schemas_get(request)

    @_tool("decision_gate_docs_search", args_schema=DecisionGateToolArgs)
    def decision_gate_docs_search(
        request: DecisionGateDocsSearchRequest,
    ) -> DecisionGateDocsSearchResponse:
        """Search Decision Gate documentation for runtime guidance."""
        _maybe_validate(validate, validate_decision_gate_docs_search_request, request)
        return client.decision_gate_docs_search(request)

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
) -> list[BaseToolLike]:
    """Build tools using a config object instead of a prebuilt client."""
    client = config.create_client()
    return build_decision_gate_tools(client, validate=config.validate)
