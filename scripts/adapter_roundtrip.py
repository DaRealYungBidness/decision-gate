#!/usr/bin/env python3
# scripts/adapter_roundtrip.py
# =============================================================================
# Module: Decision Gate Adapter Roundtrip Tests
# Description: Per-tool adapter roundtrip validation against live MCP server.
# Purpose: Enforce adapter runtime correctness beyond tool-surface conformance.
# =============================================================================

from __future__ import annotations

import argparse
import asyncio
import importlib
import json
import os
import sys
import tempfile
from dataclasses import dataclass
from typing import Callable, Iterable, Mapping, Protocol, Sequence, TypeVar, cast

from decision_gate import DecisionGateClient, JsonValue

FrameworkRunner = Callable[[str], int]
JsonMapping = Mapping[str, JsonValue]
JsonDict = dict[str, JsonValue]
JsonLike = JsonValue | str


class NamedTool(Protocol):
    name: str


class CrewTool(NamedTool, Protocol):
    def run(self, *, request: JsonMapping) -> JsonLike:
        ...


class LangChainTool(NamedTool, Protocol):
    def invoke(self, input: Mapping[str, JsonMapping]) -> JsonLike:
        ...


class AutoGenTool(NamedTool, Protocol):
    async def run_json(self, input: Mapping[str, JsonMapping], cancellation_token: object) -> JsonLike:
        ...


class ToolContextLike(Protocol):
    context: Mapping[str, object]
    tool_name: str
    tool_call_id: str
    tool_arguments: str


class OpenAIAgentsTool(NamedTool, Protocol):
    async def on_invoke_tool(self, ctx: ToolContextLike, tool_arguments: str) -> JsonLike:
        ...


TTool = TypeVar("TTool", bound=NamedTool)
ToolCaller = Callable[[TTool, JsonMapping], JsonLike]


class ToolBuilder(Protocol[TTool]):
    def __call__(self, client: DecisionGateClient, *, validate: bool = False) -> Sequence[TTool]:
        ...


def _tool_name(tool: object) -> str:
    name = getattr(tool, "name", None) or getattr(tool, "tool_name", None)
    if isinstance(name, str):
        return name
    func_name = getattr(tool, "__name__", None)
    if isinstance(func_name, str):
        return func_name
    raise KeyError("tool name unavailable")


def _load_attr(module: str, attr: str) -> object:
    return getattr(importlib.import_module(module), attr)


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def _timestamp(value: int) -> JsonDict:
    return {"kind": "logical", "value": value}


def _test_suffix(framework: str) -> str:
    suffix = os.environ.get("DG_TEST_SUFFIX", "").strip()
    if suffix:
        return suffix
    return framework


@dataclass(frozen=True)
class RoundtripFixture:
    scenario_id: str
    schema_id: str
    run_id: str
    trigger_id: str
    submission_id: str
    output_dir: str
    manifest_name: str

    def scenario_spec(self) -> JsonDict:
        return {
            "scenario_id": self.scenario_id,
            "namespace_id": 1,
            "spec_version": "1",
            "default_tenant_id": 1,
            "policies": [],
            "schemas": [],
            "conditions": [
                {
                    "condition_id": "deploy_env",
                    "query": {
                        "provider_id": "env",
                        "check_id": "get",
                        "params": {"key": "DEPLOY_ENV"},
                    },
                    "comparator": "equals",
                    "expected": "production",
                    "policy_tags": [],
                    "trust": None,
                }
            ],
            "stages": [
                {
                    "stage_id": "main",
                    "entry_packets": [],
                    "gates": [
                        {
                            "gate_id": "gate-env",
                            "requirement": {"Condition": "deploy_env"},
                            "trust": None,
                        }
                    ],
                    "advance_to": {"kind": "terminal"},
                    "timeout": None,
                    "on_timeout": "fail",
                }
            ],
        }

    def schema_record(self) -> JsonDict:
        return {
            "schema_id": self.schema_id,
            "version": "v1",
            "description": "Asserted payload schema.",
            "tenant_id": 1,
            "namespace_id": 1,
            "created_at": _timestamp(1),
            "schema": {
                "type": "object",
                "additionalProperties": False,
                "properties": {"deploy_env": {"type": "string"}},
                "required": ["deploy_env"],
            },
        }

    def precheck_request(self) -> JsonDict:
        return {
            "scenario_id": self.scenario_id,
            "spec": None,
            "stage_id": None,
            "tenant_id": 1,
            "namespace_id": 1,
            "data_shape": {"schema_id": self.schema_id, "version": "v1"},
            "payload": {"deploy_env": "production"},
        }

    def run_config(self) -> JsonDict:
        return {
            "tenant_id": 1,
            "namespace_id": 1,
            "run_id": self.run_id,
            "scenario_id": self.scenario_id,
            "dispatch_targets": [{"kind": "agent", "agent_id": "agent-1"}],
            "policy_tags": [],
        }

    def scenario_start_request(self) -> JsonDict:
        return {
            "scenario_id": self.scenario_id,
            "run_config": self.run_config(),
            "issue_entry_packets": False,
            "started_at": _timestamp(2),
        }

    def scenario_status_request(self) -> JsonDict:
        return {
            "scenario_id": self.scenario_id,
            "request": {
                "tenant_id": 1,
                "namespace_id": 1,
                "run_id": self.run_id,
                "requested_at": _timestamp(3),
            },
        }

    def scenario_next_request(self) -> JsonDict:
        return {
            "scenario_id": self.scenario_id,
            "request": {
                "tenant_id": 1,
                "namespace_id": 1,
                "run_id": self.run_id,
                "trigger_id": self.trigger_id,
                "agent_id": "agent-1",
                "time": _timestamp(4),
            },
        }

    def scenario_submit_request(self) -> JsonDict:
        return {
            "scenario_id": self.scenario_id,
            "request": {
                "tenant_id": 1,
                "namespace_id": 1,
                "run_id": self.run_id,
                "submission_id": self.submission_id,
                "payload": {"kind": "json", "value": {"deploy_env": "production"}},
                "content_type": "application/json",
                "submitted_at": _timestamp(5),
            },
        }

    def scenario_trigger_request(self) -> JsonDict:
        return {
            "scenario_id": self.scenario_id,
            "trigger": {
                "trigger_id": self.trigger_id,
                "tenant_id": 1,
                "namespace_id": 1,
                "run_id": self.run_id,
                "kind": "external_event",
                "time": _timestamp(6),
                "source_id": "adapter-roundtrip",
                "payload": {"kind": "json", "value": {"signal": "ping"}},
            },
        }

    def evidence_query_request(self) -> JsonDict:
        return {
            "context": {
                "tenant_id": 1,
                "namespace_id": 1,
                "run_id": self.run_id,
                "scenario_id": self.scenario_id,
                "stage_id": "main",
                "trigger_id": self.trigger_id,
                "trigger_time": _timestamp(7),
            },
            "query": {
                "provider_id": "env",
                "check_id": "get",
                "params": {"key": "DEPLOY_ENV"},
            },
        }

    def runpack_export_request(self) -> JsonDict:
        return {
            "tenant_id": 1,
            "namespace_id": 1,
            "run_id": self.run_id,
            "scenario_id": self.scenario_id,
            "generated_at": _timestamp(8),
            "include_verification": True,
            "output_dir": self.output_dir,
            "manifest_name": self.manifest_name,
        }

    def runpack_verify_request(self) -> JsonDict:
        return {"runpack_dir": self.output_dir, "manifest_path": self.manifest_name}


def _fixture_for_framework(framework: str, output_dir: str) -> RoundtripFixture:
    suffix = _test_suffix(framework)
    return RoundtripFixture(
        scenario_id=f"adapter-roundtrip-{framework}-{suffix}",
        schema_id=f"adapter-schema-{framework}-{suffix}",
        run_id=f"run-{framework}-{suffix}",
        trigger_id=f"trigger-{framework}-{suffix}",
        submission_id=f"submission-{framework}-{suffix}",
        output_dir=output_dir,
        manifest_name="manifest.json",
    )


def _parse_result(value: JsonLike) -> JsonLike:
    if isinstance(value, str):
        try:
            parsed = json.loads(value)
        except json.JSONDecodeError:
            return value
        return cast(JsonValue, parsed)
    return value


def _assert_dict(label: str, value: JsonLike) -> JsonDict:
    _require(isinstance(value, dict), f"{label} result should be dict, got {type(value).__name__}")
    return cast(JsonDict, value)


def _assert_keys(label: str, payload: Mapping[str, JsonValue], keys: Iterable[str]) -> None:
    missing = [key for key in keys if key not in payload]
    _require(not missing, f"{label} missing keys: {', '.join(missing)}")


def _as_list_of_dicts(label: str, value: JsonValue) -> list[JsonDict]:
    _require(isinstance(value, list), f"{label} should be list, got {type(value).__name__}")
    items: list[JsonDict] = []
    for item in value:
        _require(isinstance(item, dict), f"{label} item should be dict, got {type(item).__name__}")
        items.append(cast(JsonDict, item))
    return items


def _build_client() -> DecisionGateClient:
    endpoint = os.environ.get("DG_ENDPOINT", "http://127.0.0.1:8080/rpc")
    token = os.environ.get("DG_TOKEN")
    return DecisionGateClient(endpoint=endpoint, auth_token=token)


def _validate_enabled() -> bool:
    return os.environ.get("DG_VALIDATE") == "1"


def _crewAI_runner(framework: str) -> int:
    from decision_gate_crewai import build_decision_gate_tools

    client = _build_client()
    builder = cast(ToolBuilder[CrewTool], build_decision_gate_tools)
    tools_list = builder(client, validate=_validate_enabled())
    tools: dict[str, CrewTool] = {_tool_name(tool): tool for tool in tools_list}

    with tempfile.TemporaryDirectory() as output_dir:
        fixture = _fixture_for_framework(framework, output_dir)
        return _exercise_roundtrip(framework, tools, fixture, _crew_call)


def _crew_call(tool: CrewTool, request: JsonMapping) -> JsonLike:
    return _parse_result(tool.run(request=request))


def _langchain_runner(framework: str) -> int:
    from decision_gate_langchain import build_decision_gate_tools

    client = _build_client()
    builder = cast(ToolBuilder[LangChainTool], build_decision_gate_tools)
    tools_list = builder(client, validate=_validate_enabled())
    tools: dict[str, LangChainTool] = {_tool_name(tool): tool for tool in tools_list}

    with tempfile.TemporaryDirectory() as output_dir:
        fixture = _fixture_for_framework(framework, output_dir)
        return _exercise_roundtrip(framework, tools, fixture, _langchain_call)


def _langchain_call(tool: LangChainTool, request: JsonMapping) -> JsonLike:
    return _parse_result(tool.invoke({"request": request}))


def _autogen_runner(framework: str) -> int:
    from decision_gate_autogen import build_decision_gate_tools

    client = _build_client()
    builder = cast(ToolBuilder[AutoGenTool], build_decision_gate_tools)
    tools_list = builder(client, validate=_validate_enabled())
    tools: dict[str, AutoGenTool] = {_tool_name(tool): tool for tool in tools_list}
    cancellation_token_type = cast(type[object], _load_attr("autogen_core", "CancellationToken"))

    async def call_tool(tool: AutoGenTool, request: JsonMapping) -> JsonLike:
        return await tool.run_json({"request": request}, cancellation_token_type())

    with tempfile.TemporaryDirectory() as output_dir:
        fixture = _fixture_for_framework(framework, output_dir)
        return _exercise_roundtrip(framework, tools, fixture, lambda tool, req: asyncio.run(call_tool(tool, req)))


def _openai_agents_runner(framework: str) -> int:
    from decision_gate_openai_agents import build_decision_gate_tools

    client = _build_client()
    builder = cast(ToolBuilder[OpenAIAgentsTool], build_decision_gate_tools)
    tools_list = builder(client, validate=_validate_enabled())
    tools: dict[str, OpenAIAgentsTool] = {_tool_name(tool): tool for tool in tools_list}
    tool_context_type = cast(type[ToolContextLike], _load_attr("agents.tool_context", "ToolContext"))

    async def call_tool(tool: OpenAIAgentsTool, request: JsonMapping) -> JsonLike:
        tool_name = _tool_name(tool)
        args = json.dumps({"request": request})
        ctx = tool_context_type(
            context={},
            tool_name=tool_name,
            tool_call_id=f"call-{tool_name}",
            tool_arguments=args,
        )
        return await tool.on_invoke_tool(ctx, args)

    with tempfile.TemporaryDirectory() as output_dir:
        fixture = _fixture_for_framework(framework, output_dir)
        return _exercise_roundtrip(
            framework,
            tools,
            fixture,
            lambda tool, req: _parse_result(asyncio.run(call_tool(tool, req))),
        )


def _exercise_roundtrip(
    framework: str,
    tools: Mapping[str, TTool],
    fixture: RoundtripFixture,
    caller: ToolCaller[TTool],
) -> int:
    def tool(name: str) -> TTool:
        if name not in tools:
            raise KeyError(f"{framework} missing tool {name}")
        return tools[name]

    result = caller(tool("decision_gate_scenario_define"), {"spec": fixture.scenario_spec()})
    result = _assert_dict("scenario_define", _parse_result(result))
    _assert_keys("scenario_define", result, ["scenario_id", "spec_hash"])

    result = caller(tool("decision_gate_schemas_register"), {"record": fixture.schema_record()})
    result = _assert_dict("schemas_register", _parse_result(result))
    _assert_keys("schemas_register", result, ["record"])

    result = caller(tool("decision_gate_precheck"), fixture.precheck_request())
    result = _assert_dict("precheck", _parse_result(result))
    _assert_keys("precheck", result, ["decision", "gate_evaluations"])

    result = caller(tool("decision_gate_scenarios_list"), {"tenant_id": 1, "namespace_id": 1})
    result = _assert_dict("scenarios_list", _parse_result(result))
    _assert_keys("scenarios_list", result, ["items", "next_token"])

    result = caller(tool("decision_gate_schemas_list"), {"tenant_id": 1, "namespace_id": 1})
    result = _assert_dict("schemas_list", _parse_result(result))
    _assert_keys("schemas_list", result, ["items", "next_token"])

    result = caller(
        tool("decision_gate_schemas_get"),
        {"tenant_id": 1, "namespace_id": 1, "schema_id": fixture.schema_id, "version": "v1"},
    )
    result = _assert_dict("schemas_get", _parse_result(result))
    _assert_keys("schemas_get", result, ["record"])

    result = caller(tool("decision_gate_scenario_start"), fixture.scenario_start_request())
    result = _assert_dict("scenario_start", _parse_result(result))
    _assert_keys("scenario_start", result, ["run_id", "scenario_id", "status"])

    result = caller(tool("decision_gate_scenario_status"), fixture.scenario_status_request())
    result = _assert_dict("scenario_status", _parse_result(result))
    _assert_keys("scenario_status", result, ["run_id", "scenario_id", "status"])

    result = caller(tool("decision_gate_scenario_next"), fixture.scenario_next_request())
    result = _assert_dict("scenario_next", _parse_result(result))
    _assert_keys("scenario_next", result, ["decision", "status", "packets"])

    result = caller(tool("decision_gate_scenario_submit"), fixture.scenario_submit_request())
    result = _assert_dict("scenario_submit", _parse_result(result))
    _assert_keys("scenario_submit", result, ["record"])

    result = caller(tool("decision_gate_scenario_trigger"), fixture.scenario_trigger_request())
    result = _assert_dict("scenario_trigger", _parse_result(result))
    _assert_keys("scenario_trigger", result, ["decision", "status", "packets"])

    result = caller(tool("decision_gate_evidence_query"), fixture.evidence_query_request())
    result = _assert_dict("evidence_query", _parse_result(result))
    _assert_keys("evidence_query", result, ["result"])

    result = caller(tool("decision_gate_providers_list"), {})
    result = _assert_dict("providers_list", _parse_result(result))
    _assert_keys("providers_list", result, ["providers"])
    providers = _as_list_of_dicts("providers_list.providers", result["providers"])
    _require(
        any(provider.get("provider_id") == "env" for provider in providers),
        "providers_list missing env provider",
    )

    result = caller(tool("decision_gate_provider_contract_get"), {"provider_id": "env"})
    result = _assert_dict("provider_contract_get", _parse_result(result))
    _assert_keys("provider_contract_get", result, ["provider_id", "contract_hash", "contract"])

    result = caller(
        tool("decision_gate_provider_check_schema_get"),
        {"provider_id": "env", "check_id": "get"},
    )
    result = _assert_dict("provider_check_schema_get", _parse_result(result))
    _assert_keys("provider_check_schema_get", result, ["provider_id", "check_id", "params_schema"])

    result = caller(tool("decision_gate_runpack_export"), fixture.runpack_export_request())
    result = _assert_dict("runpack_export", _parse_result(result))
    _assert_keys("runpack_export", result, ["manifest", "report"])

    result = caller(tool("decision_gate_runpack_verify"), fixture.runpack_verify_request())
    result = _assert_dict("runpack_verify", _parse_result(result))
    _assert_keys("runpack_verify", result, ["status", "report"])
    _require(result.get("status") == "pass", "runpack_verify status != pass")

    result = caller(tool("decision_gate_docs_search"), {"query": "evidence flow", "max_sections": 3})
    result = _assert_dict("docs_search", _parse_result(result))
    _assert_keys("docs_search", result, ["docs_covered", "sections", "suggested_followups"])

    print(json.dumps({"framework": framework, "status": "ok"}))
    return 0


def _parse_frameworks(value: str) -> list[str]:
    if not value:
        return []
    return [item.strip() for item in value.split(",") if item.strip()]


def main() -> int:
    parser = argparse.ArgumentParser(description="Adapter roundtrip test suite")
    parser.add_argument(
        "--frameworks",
        default="langchain,crewai,autogen,openai_agents",
        help="Comma-separated adapters to test.",
    )
    args = parser.parse_args()

    frameworks = _parse_frameworks(args.frameworks)
    if not frameworks:
        print("No frameworks specified.", file=sys.stderr)
        return 1

    runners: dict[str, FrameworkRunner] = {
        "langchain": _langchain_runner,
        "crewai": _crewAI_runner,
        "autogen": _autogen_runner,
        "openai_agents": _openai_agents_runner,
        "openai-agents": _openai_agents_runner,
    }

    for framework in frameworks:
        runner = runners.get(framework)
        if runner is None:
            print(f"Unknown framework: {framework}", file=sys.stderr)
            return 1
        try:
            runner(framework)
        except Exception as exc:
            print(
                json.dumps(
                    {
                        "framework": framework,
                        "status": "fatal_error",
                        "error": str(exc),
                    }
                )
            )
            return 1

    return 0


if __name__ == "__main__":
    sys.exit(main())
