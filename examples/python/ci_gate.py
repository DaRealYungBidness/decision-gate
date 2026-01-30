#!/usr/bin/env python3
# examples/python/ci_gate.py
# ============================================================================
# Module: Decision Gate CI Gate (Python)
# Description: Trigger a run and export a runpack for audit.
# Purpose: Runnable example for CI/CD gating workflows.
# Dependencies: decision_gate SDK (local), stdlib
# ============================================================================

from __future__ import annotations

import json
import os
import sys
import tempfile

from decision_gate import (
    DecisionGateClient,
    validate_runpack_export_request,
    validate_scenario_define_request,
    validate_scenario_start_request,
    validate_scenario_trigger_request,
)


def load_env_json(name: str) -> object | None:
    value = os.environ.get(name)
    if not value:
        return None
    return json.loads(value)


def default_time_after_spec(scenario_id: str, threshold: int) -> dict:
    return {
        "scenario_id": scenario_id,
        "namespace_id": 1,
        "spec_version": "1",
        "default_tenant_id": None,
        "policies": [],
        "schemas": [],
        "conditions": [
            {
                "condition_id": "after",
                "query": {
                    "provider_id": "time",
                    "check_id": "after",
                    "params": {"timestamp": threshold},
                },
                "comparator": "equals",
                "expected": True,
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
                        "requirement": {"Condition": "after"},
                        "trust": None,
                    }
                ],
                "advance_to": {"kind": "terminal"},
                "timeout": None,
                "on_timeout": "fail",
            }
        ],
    }


def default_run_config(scenario_id: str, run_id: str) -> dict:
    return {
        "scenario_id": scenario_id,
        "run_id": run_id,
        "tenant_id": 1,
        "namespace_id": 1,
        "policy_tags": [],
        "dispatch_targets": [],
    }


def maybe_validate(enabled: bool, validator, payload: dict) -> None:
    if enabled:
        validator(payload)


def main() -> int:
    endpoint = os.environ.get("DG_ENDPOINT", "http://127.0.0.1:8080/rpc")
    token = os.environ.get("DG_TOKEN")
    validate_enabled = os.environ.get("DG_VALIDATE") == "1"

    spec = load_env_json("DG_SCENARIO_SPEC") or default_time_after_spec(
        "example-ci-gate", 0
    )
    scenario_id = spec["scenario_id"]
    run_config = load_env_json("DG_RUN_CONFIG") or default_run_config(
        scenario_id, "run-ci-1"
    )
    started_at = load_env_json("DG_STARTED_AT") or {"kind": "logical", "value": 1}

    client = DecisionGateClient(endpoint=endpoint, auth_token=token)

    define_request = {"spec": spec}
    maybe_validate(validate_enabled, validate_scenario_define_request, define_request)
    define = client.scenario_define(define_request)

    start_request = {
        "scenario_id": scenario_id,
        "run_config": run_config,
        "started_at": started_at,
        "issue_entry_packets": False,
    }
    maybe_validate(validate_enabled, validate_scenario_start_request, start_request)
    start = client.scenario_start(start_request)

    trigger_request = {
        "scenario_id": scenario_id,
        "trigger": {
            "trigger_id": "trigger-ci-1",
            "kind": "tick",
            "source_id": "ci",
            "tenant_id": run_config["tenant_id"],
            "namespace_id": run_config["namespace_id"],
            "run_id": run_config["run_id"],
            "time": {"kind": "logical", "value": 2},
            "payload": None,
            "correlation_id": None,
        },
    }
    maybe_validate(validate_enabled, validate_scenario_trigger_request, trigger_request)
    trigger = client.scenario_trigger(trigger_request)

    output_dir = tempfile.mkdtemp(prefix="decision-gate-runpack-")
    export_request = {
        "scenario_id": scenario_id,
        "run_id": run_config["run_id"],
        "tenant_id": run_config["tenant_id"],
        "namespace_id": run_config["namespace_id"],
        "output_dir": output_dir,
        "manifest_name": "manifest.json",
        "generated_at": {"kind": "logical", "value": 3},
        "include_verification": False,
    }
    maybe_validate(validate_enabled, validate_runpack_export_request, export_request)
    runpack = client.runpack_export(export_request)

    print(
        json.dumps(
            {
                "define": define,
                "start": start,
                "trigger": trigger,
                "runpack": runpack,
            }
        )
    )
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as exc:
        print(json.dumps({"status": "fatal_error", "error": str(exc)}))
        sys.exit(1)
