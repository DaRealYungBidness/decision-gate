#!/usr/bin/env python3
# examples/python/precheck.py
# ============================================================================
# Module: Decision Gate Precheck (Python)
# Description: Register a data shape and precheck a scenario with asserted data.
# Purpose: Runnable example for asserted-evidence workflows.
# Dependencies: decision_gate SDK (local), stdlib
# ============================================================================

from __future__ import annotations

import json
import os
import sys

from decision_gate import (
    DecisionGateClient,
    validate_precheck_request,
    validate_scenario_define_request,
    validate_schemas_register_request,
)


def load_env_json(name: str) -> object | None:
    value = os.environ.get(name)
    if not value:
        return None
    return json.loads(value)


def default_precheck_spec(scenario_id: str) -> dict:
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
                        "gate_id": "gate-time",
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


def default_schema_record() -> dict:
    return {
        "schema_id": "asserted_payload",
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


def maybe_validate(enabled: bool, validator, payload: dict) -> None:
    if enabled:
        validator(payload)


def main() -> int:
    endpoint = os.environ.get("DG_ENDPOINT", "http://127.0.0.1:8080/rpc")
    token = os.environ.get("DG_TOKEN")
    validate_enabled = os.environ.get("DG_VALIDATE") == "1"

    spec = load_env_json("DG_SCENARIO_SPEC") or default_precheck_spec("example-precheck")
    scenario_id = spec["scenario_id"]
    schema_record = load_env_json("DG_SCHEMA_RECORD") or default_schema_record()

    client = DecisionGateClient(endpoint=endpoint, auth_token=token)

    define_request = {"spec": spec}
    maybe_validate(validate_enabled, validate_scenario_define_request, define_request)
    define = client.scenario_define(define_request)

    register_request = {"record": schema_record}
    maybe_validate(validate_enabled, validate_schemas_register_request, register_request)
    registered = client.schemas_register(register_request)

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
    precheck = client.precheck(precheck_request)

    print(json.dumps({"define": define, "schema": registered, "precheck": precheck}))
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as exc:
        print(json.dumps({"status": "fatal_error", "error": str(exc)}))
        sys.exit(1)
