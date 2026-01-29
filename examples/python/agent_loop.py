#!/usr/bin/env python3
# examples/python/agent_loop.py
# ============================================================================
# Module: Decision Gate Agent Loop (Python)
# Description: Start a run and advance it via scenario_next.
# Purpose: Runnable example for agent-driven workflows.
# Dependencies: decision_gate SDK (local), stdlib
# ============================================================================

from __future__ import annotations

import json
import os
import sys

from decision_gate import (
    DecisionGateClient,
    validate_scenario_define_request,
    validate_scenario_next_request,
    validate_scenario_start_request,
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
        "predicates": [
            {
                "predicate": "after",
                "query": {
                    "provider_id": "time",
                    "predicate": "after",
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
                        "requirement": {"predicate": "after"},
                        "trust": None,
                    }
                ],
                "advance_to": {"kind": "terminal"},
                "timeout": None,
                "on_timeout": "fail",
            }
        ],
    }


def default_run_config(scenario_id: str, run_id: str, agent_id: str) -> dict:
    return {
        "scenario_id": scenario_id,
        "run_id": run_id,
        "tenant_id": 1,
        "namespace_id": 1,
        "policy_tags": [],
        "dispatch_targets": [{"kind": "agent", "agent_id": agent_id}],
    }


def maybe_validate(enabled: bool, validator, payload: dict) -> None:
    if enabled:
        validator(payload)


def main() -> int:
    endpoint = os.environ.get("DG_ENDPOINT", "http://127.0.0.1:8080/rpc")
    token = os.environ.get("DG_TOKEN")
    validate_enabled = os.environ.get("DG_VALIDATE") == "1"

    agent_id = os.environ.get("DG_AGENT_ID", "agent-alpha")
    spec = load_env_json("DG_SCENARIO_SPEC") or default_time_after_spec(
        "example-agent-loop", 0
    )
    scenario_id = spec["scenario_id"]
    run_config = load_env_json("DG_RUN_CONFIG") or default_run_config(
        scenario_id, "run-agent-1", agent_id
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
        "issue_entry_packets": True,
    }
    maybe_validate(validate_enabled, validate_scenario_start_request, start_request)
    start = client.scenario_start(start_request)

    next_request = {
        "scenario_id": scenario_id,
        "request": {
            "tenant_id": run_config["tenant_id"],
            "namespace_id": run_config["namespace_id"],
            "run_id": run_config["run_id"],
            "agent_id": agent_id,
            "trigger_id": "trigger-agent-1",
            "time": {"kind": "logical", "value": 2},
            "correlation_id": None,
        },
    }
    maybe_validate(validate_enabled, validate_scenario_next_request, next_request)
    next_decision = client.scenario_next(next_request)

    print(json.dumps({"define": define, "start": start, "next": next_decision}))
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as exc:
        print(json.dumps({"status": "fatal_error", "error": str(exc)}))
        sys.exit(1)
