# adapters/autogen/src/decision_gate_autogen/tools.py
# ============================================================================
# Module: Decision Gate AutoGen Tools
# Description: Build AutoGen FunctionTool entries that call Decision Gate.
# ============================================================================

from __future__ import annotations

from dataclasses import dataclass
from typing import Callable, Mapping, Optional, Protocol, TypeVar, cast

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


class FunctionToolLike(Protocol):
    name: str


def _maybe_validate(enabled: bool, validator: Callable[[TRequest], None], payload: TRequest) -> None:
    if enabled:
        validator(payload)


def _coerce_request(
    payload: dict[str, JsonValue],
    validator: Callable[[TRequest], None],
    validate: bool,
) -> TRequest:
    typed = cast(TRequest, payload)
    _maybe_validate(validate, validator, typed)
    return typed


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
) -> list[FunctionToolLike]:
    """Return AutoGen FunctionTool entries for core Decision Gate operations."""
    from autogen_core.tools import FunctionTool

    def decision_gate_precheck(request: dict[str, JsonValue]) -> PrecheckResponse:
        """Run a Decision Gate precheck without mutating run state."""
        typed: PrecheckRequest = _coerce_request(request, validate_precheck_request, validate)
        return client.precheck(typed)

    def decision_gate_scenario_define(request: dict[str, JsonValue]) -> ScenarioDefineResponse:
        """Register a ScenarioSpec before starting a run."""
        typed: ScenarioDefineRequest = _coerce_request(
            request, validate_scenario_define_request, validate
        )
        return client.scenario_define(typed)

    def decision_gate_scenario_start(request: dict[str, JsonValue]) -> ScenarioStartResponse:
        """Start a new run for a registered scenario."""
        typed: ScenarioStartRequest = _coerce_request(
            request, validate_scenario_start_request, validate
        )
        return client.scenario_start(typed)

    def decision_gate_scenario_status(request: dict[str, JsonValue]) -> ScenarioStatusResponse:
        """Fetch current Decision Gate run status without mutation."""
        typed: ScenarioStatusRequest = _coerce_request(
            request, validate_scenario_status_request, validate
        )
        return client.scenario_status(typed)

    def decision_gate_scenario_next(request: dict[str, JsonValue]) -> ScenarioNextResponse:
        """Advance a Decision Gate run to the next stage if gates pass."""
        typed: ScenarioNextRequest = _coerce_request(
            request, validate_scenario_next_request, validate
        )
        return client.scenario_next(typed)

    def decision_gate_scenario_submit(request: dict[str, JsonValue]) -> ScenarioSubmitResponse:
        """Submit the current stage decision for a scenario run."""
        typed: ScenarioSubmitRequest = _coerce_request(
            request, validate_scenario_submit_request, validate
        )
        return client.scenario_submit(typed)

    def decision_gate_scenario_trigger(request: dict[str, JsonValue]) -> ScenarioTriggerResponse:
        """Trigger a scenario evaluation with an external event."""
        typed: ScenarioTriggerRequest = _coerce_request(
            request, validate_scenario_trigger_request, validate
        )
        return client.scenario_trigger(typed)

    def decision_gate_scenarios_list(request: dict[str, JsonValue]) -> ScenariosListResponse:
        """List registered scenarios for a tenant and namespace."""
        typed: ScenariosListRequest = _coerce_request(
            request, validate_scenarios_list_request, validate
        )
        return client.scenarios_list(typed)

    def decision_gate_evidence_query(request: dict[str, JsonValue]) -> EvidenceQueryResponse:
        """Query evidence providers for condition inputs."""
        typed: EvidenceQueryRequest = _coerce_request(
            request, validate_evidence_query_request, validate
        )
        return client.evidence_query(typed)

    def decision_gate_runpack_export(request: dict[str, JsonValue]) -> RunpackExportResponse:
        """Export an audit-grade runpack for a scenario run."""
        typed: RunpackExportRequest = _coerce_request(
            request, validate_runpack_export_request, validate
        )
        return client.runpack_export(typed)

    def decision_gate_runpack_verify(request: dict[str, JsonValue]) -> RunpackVerifyResponse:
        """Verify a runpack manifest against expected hashes."""
        typed: RunpackVerifyRequest = _coerce_request(
            request, validate_runpack_verify_request, validate
        )
        return client.runpack_verify(typed)

    def decision_gate_providers_list(request: dict[str, JsonValue]) -> ProvidersListResponse:
        """List registered evidence providers and their capabilities."""
        typed: ProvidersListRequest = _coerce_request(
            request, validate_providers_list_request, validate
        )
        return client.providers_list(typed)

    def decision_gate_provider_contract_get(
        request: dict[str, JsonValue],
    ) -> ProviderContractGetResponse:
        """Fetch a provider contract payload."""
        typed: ProviderContractGetRequest = _coerce_request(
            request, validate_provider_contract_get_request, validate
        )
        return client.provider_contract_get(typed)

    def decision_gate_provider_check_schema_get(
        request: dict[str, JsonValue],
    ) -> ProviderCheckSchemaGetResponse:
        """Fetch a provider check schema."""
        typed: ProviderCheckSchemaGetRequest = _coerce_request(
            request, validate_provider_check_schema_get_request, validate
        )
        return client.provider_check_schema_get(typed)

    def decision_gate_schemas_register(request: dict[str, JsonValue]) -> SchemasRegisterResponse:
        """Register a data shape schema."""
        typed: SchemasRegisterRequest = _coerce_request(
            request, validate_schemas_register_request, validate
        )
        return client.schemas_register(typed)

    def decision_gate_schemas_list(request: dict[str, JsonValue]) -> SchemasListResponse:
        """List registered data shape schemas."""
        typed: SchemasListRequest = _coerce_request(
            request, validate_schemas_list_request, validate
        )
        return client.schemas_list(typed)

    def decision_gate_schemas_get(request: dict[str, JsonValue]) -> SchemasGetResponse:
        """Fetch a registered data shape schema."""
        typed: SchemasGetRequest = _coerce_request(
            request, validate_schemas_get_request, validate
        )
        return client.schemas_get(typed)

    def decision_gate_docs_search(
        request: dict[str, JsonValue],
    ) -> DecisionGateDocsSearchResponse:
        """Search Decision Gate documentation for runtime guidance."""
        typed: DecisionGateDocsSearchRequest = _coerce_request(
            request, validate_decision_gate_docs_search_request, validate
        )
        return client.decision_gate_docs_search(typed)

    return [
        FunctionTool(
            decision_gate_precheck,
            name="decision_gate_precheck",
            description="Run a Decision Gate precheck without mutating run state.",
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
            decision_gate_scenario_status,
            name="decision_gate_scenario_status",
            description="Fetch current Decision Gate run status without mutation.",
        ),
        FunctionTool(
            decision_gate_scenario_next,
            name="decision_gate_scenario_next",
            description="Advance a Decision Gate run to the next stage if gates pass.",
        ),
        FunctionTool(
            decision_gate_scenario_submit,
            name="decision_gate_scenario_submit",
            description="Submit the current stage decision for a scenario run.",
        ),
        FunctionTool(
            decision_gate_scenario_trigger,
            name="decision_gate_scenario_trigger",
            description="Trigger a scenario evaluation with an external event.",
        ),
        FunctionTool(
            decision_gate_scenarios_list,
            name="decision_gate_scenarios_list",
            description="List registered scenarios for a tenant and namespace.",
        ),
        FunctionTool(
            decision_gate_evidence_query,
            name="decision_gate_evidence_query",
            description="Query evidence providers for condition inputs.",
        ),
        FunctionTool(
            decision_gate_runpack_export,
            name="decision_gate_runpack_export",
            description="Export an audit-grade runpack for a scenario run.",
        ),
        FunctionTool(
            decision_gate_runpack_verify,
            name="decision_gate_runpack_verify",
            description="Verify a runpack manifest against expected hashes.",
        ),
        FunctionTool(
            decision_gate_providers_list,
            name="decision_gate_providers_list",
            description="List registered evidence providers and capabilities.",
        ),
        FunctionTool(
            decision_gate_provider_contract_get,
            name="decision_gate_provider_contract_get",
            description="Fetch a provider contract payload.",
        ),
        FunctionTool(
            decision_gate_provider_check_schema_get,
            name="decision_gate_provider_check_schema_get",
            description="Fetch a provider check schema.",
        ),
        FunctionTool(
            decision_gate_schemas_register,
            name="decision_gate_schemas_register",
            description="Register a data shape schema.",
        ),
        FunctionTool(
            decision_gate_schemas_list,
            name="decision_gate_schemas_list",
            description="List registered data shape schemas.",
        ),
        FunctionTool(
            decision_gate_schemas_get,
            name="decision_gate_schemas_get",
            description="Fetch a registered data shape schema.",
        ),
        FunctionTool(
            decision_gate_docs_search,
            name="decision_gate_docs_search",
            description="Search Decision Gate documentation for runtime guidance.",
        ),
    ]


def build_decision_gate_tools_from_config(
    config: DecisionGateToolConfig,
) -> list[FunctionToolLike]:
    """Build tools using a config object instead of a prebuilt client."""
    client = config.create_client()
    return build_decision_gate_tools(client, validate=config.validate)
