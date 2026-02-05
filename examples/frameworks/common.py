#!/usr/bin/env python3
# examples/frameworks/common.py
# ============================================================================
# Module: Shared helpers for framework adapter examples.
# ============================================================================

from __future__ import annotations

import json
import os
from typing import Any, Dict, Iterable, Tuple

from decision_gate import (
    DecisionGateClient,
    validate_precheck_request,
    validate_scenario_define_request,
    validate_schemas_register_request,
)


def _suffix(value: str, suffix: str) -> str:
    return f"{value}-{suffix}" if suffix else value


def _test_suffix() -> str:
    return os.environ.get("DG_TEST_SUFFIX", "").strip()


def load_env_json(name: str) -> object | None:
    value = os.environ.get(name)
    if not value:
        return None
    return json.loads(value)


def default_precheck_spec(scenario_id: str) -> Dict[str, Any]:
    return {
        "scenario_id": scenario_id,
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


def default_schema_record(schema_id: str) -> Dict[str, Any]:
    return {
        "schema_id": schema_id,
        "version": "v1",
        "description": "Asserted payload schema.",
        "tenant_id": 1,
        "namespace_id": 1,
        "created_at": {"kind": "logical", "value": 1},
        "schema": {
            "type": "object",
            "additionalProperties": False,
            "properties": {"deploy_env": {"type": "string"}},
            "required": ["deploy_env"],
        },
    }


def maybe_validate(enabled: bool, validator, payload: Dict[str, Any]) -> None:
    if enabled:
        validator(payload)


def find_tool(tools: Iterable[object], name: str) -> object:
    for tool in tools:
        tool_name = getattr(tool, "name", None) or getattr(tool, "tool_name", None)
        if isinstance(tool_name, str) and tool_name == name:
            return tool
        func_name = getattr(tool, "__name__", None)
        if isinstance(func_name, str) and func_name == name:
            return tool
    raise KeyError(f"Tool not found: {name}")


def prepare_precheck(
    client: DecisionGateClient,
    *,
    validate_enabled: bool,
) -> Tuple[Dict[str, Any], Dict[str, Any]]:
    suffix = _test_suffix()
    default_scenario_id = _suffix("example-framework-precheck", suffix)
    default_schema_id = _suffix("asserted_payload", suffix)

    spec = load_env_json("DG_SCENARIO_SPEC") or default_precheck_spec(default_scenario_id)
    scenario_id = spec["scenario_id"]
    schema_record = load_env_json("DG_SCHEMA_RECORD") or default_schema_record(default_schema_id)

    define_request = {"spec": spec}
    maybe_validate(validate_enabled, validate_scenario_define_request, define_request)
    client.scenario_define(define_request)

    register_request = {"record": schema_record}
    maybe_validate(validate_enabled, validate_schemas_register_request, register_request)
    client.schemas_register(register_request)

    precheck_request = {
        "scenario_id": scenario_id,
        "spec": None,
        "stage_id": None,
        "tenant_id": schema_record["tenant_id"],
        "namespace_id": schema_record["namespace_id"],
        "data_shape": {
            "schema_id": schema_record["schema_id"],
            "version": schema_record["version"],
        },
        "payload": {"deploy_env": "production"},
    }
    maybe_validate(validate_enabled, validate_precheck_request, precheck_request)

    return precheck_request, {
        "scenario_id": scenario_id,
        "tenant_id": schema_record["tenant_id"],
        "namespace_id": schema_record["namespace_id"],
    }
