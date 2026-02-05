#!/usr/bin/env python3
# system-tests/tests/fixtures/sdk_client_python.py
# ============================================================================
# Module: SDK Client Test Script (Python)
# Description: Exercises the Decision Gate Python SDK against MCP HTTP.
# Purpose: Validate SDK transport + tool invocations in system-tests.
# Dependencies: stdlib, decision_gate SDK
# ============================================================================

from __future__ import annotations

import json
import os
import sys

from decision_gate import DecisionGateClient, DecisionGateTransportError


def load_env_json(name: str) -> object:
    value = os.environ.get(name)
    if not value:
        raise RuntimeError(f"missing env {name}")
    return json.loads(value)


def main() -> int:
    endpoint = os.environ.get("DG_ENDPOINT")
    if not endpoint:
        raise RuntimeError("missing DG_ENDPOINT")
    token = os.environ.get("DG_TOKEN")
    expect_failure = os.environ.get("DG_EXPECT_FAILURE") == "1"

    spec = load_env_json("DG_SCENARIO_SPEC")
    run_config = load_env_json("DG_RUN_CONFIG")
    started_at = load_env_json("DG_STARTED_AT")

    client = DecisionGateClient(endpoint=endpoint, auth_token=token)

    if expect_failure:
        try:
            client.scenario_define({"spec": spec})
        except DecisionGateTransportError as exc:
            print(json.dumps({"status": "expected_failure", "error": str(exc)}))
            return 0
        except Exception as exc:  # pragma: no cover - unexpected error type
            print(json.dumps({"status": "unexpected_error", "error": str(exc)}))
            return 1
        print(json.dumps({"status": "unexpected_success"}))
        return 1

    define = client.scenario_define({"spec": spec})
    start = client.scenario_start(
        {
            "scenario_id": run_config["scenario_id"],
            "run_config": run_config,
            "started_at": started_at,
            "issue_entry_packets": False,
        }
    )
    status = client.scenario_status(
        {
            "scenario_id": run_config["scenario_id"],
            "request": {
                "tenant_id": run_config["tenant_id"],
                "namespace_id": run_config["namespace_id"],
                "run_id": run_config["run_id"],
                "requested_at": {"kind": "logical", "value": 2},
                "correlation_id": None,
            },
        }
    )
    print(json.dumps({"define": define, "start": start, "status": status}))
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as exc:  # pragma: no cover - hard failure
        print(json.dumps({"status": "fatal_error", "error": str(exc)}))
        sys.exit(1)
