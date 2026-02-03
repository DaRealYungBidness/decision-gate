#!/usr/bin/env python3
# system-tests/tests/fixtures/agentic/drivers/adapter_langchain.py
# ============================================================================
# Module: Agentic Flow Harness LangChain Driver
# Description: Executes a scenario pack via the Decision Gate LangChain tools.
# ============================================================================

from __future__ import annotations

import json
import os
import sys
import tempfile
from typing import Any

try:
    from decision_gate import DecisionGateClient
    from decision_gate_langchain import build_decision_gate_tools
except Exception as exc:  # pragma: no cover - optional dependency
    print(json.dumps({"status": "skipped", "driver": "langchain", "reason": str(exc)}))
    sys.exit(0)

HTTP_PLACEHOLDER = "{{HTTP_BASE_URL}}"


def load_json(path: str) -> Any:
    with open(path, "r", encoding="utf-8") as handle:
        return json.load(handle)


def replace_placeholder(value: Any, placeholder: str, replacement: str) -> Any:
    if isinstance(value, dict):
        return {
            key: replace_placeholder(val, placeholder, replacement) for key, val in value.items()
        }
    if isinstance(value, list):
        return [replace_placeholder(item, placeholder, replacement) for item in value]
    if isinstance(value, str) and placeholder in value:
        return value.replace(placeholder, replacement)
    return value


def extract_outcome_kind(status: dict[str, Any]) -> str | None:
    decision = status.get("last_decision")
    if not isinstance(decision, dict):
        return None
    outcome = decision.get("outcome")
    if not isinstance(outcome, dict):
        return None
    kind = outcome.get("kind")
    if isinstance(kind, str):
        return kind.lower()
    if len(outcome) != 1:
        return None
    return next(iter(outcome.keys())).lower()


def main() -> int:
    scenario_dir = os.environ.get("DG_SCENARIO_PACK")
    if not scenario_dir:
        raise RuntimeError("missing DG_SCENARIO_PACK")

    endpoint = os.environ.get("DG_ENDPOINT")
    if not endpoint:
        raise RuntimeError("missing DG_ENDPOINT")

    http_base_url = os.environ.get("DG_HTTP_BASE_URL")
    runpack_dir = os.environ.get("DG_RUNPACK_DIR")
    token = os.environ.get("DG_TOKEN")

    spec = load_json(os.path.join(scenario_dir, "spec.json"))
    if http_base_url:
        spec = replace_placeholder(spec, HTTP_PLACEHOLDER, http_base_url)

    run_config = load_json(os.path.join(scenario_dir, "run_config.json"))
    trigger = load_json(os.path.join(scenario_dir, "trigger.json"))

    client = DecisionGateClient(endpoint=endpoint, auth_token=token)
    tools = build_decision_gate_tools(client, validate=False)
    tool_map = {tool.name: tool for tool in tools}

    def invoke(name: str, request: dict[str, Any]) -> dict[str, Any]:
        tool = tool_map[name]
        return tool.invoke({"request": request})

    invoke("decision_gate_scenario_define", {"spec": spec})
    invoke(
        "decision_gate_scenario_start",
        {
            "scenario_id": run_config["scenario_id"],
            "run_config": run_config,
            "started_at": {"kind": "logical", "value": 1},
            "issue_entry_packets": False,
        },
    )
    invoke(
        "decision_gate_scenario_trigger",
        {"scenario_id": run_config["scenario_id"], "trigger": trigger},
    )

    status = invoke(
        "decision_gate_scenario_status",
        {
            "scenario_id": run_config["scenario_id"],
            "request": {
                "tenant_id": run_config["tenant_id"],
                "namespace_id": run_config["namespace_id"],
                "run_id": run_config["run_id"],
                "requested_at": {"kind": "logical", "value": 3},
                "correlation_id": None,
            },
        },
    )

    if not runpack_dir:
        runpack_dir = tempfile.mkdtemp(prefix="dg-agentic-runpack-")

    export = invoke(
        "decision_gate_runpack_export",
        {
            "scenario_id": run_config["scenario_id"],
            "run_id": run_config["run_id"],
            "tenant_id": run_config["tenant_id"],
            "namespace_id": run_config["namespace_id"],
            "output_dir": runpack_dir,
            "manifest_name": "manifest.json",
            "generated_at": {"kind": "logical", "value": 10},
            "include_verification": False,
        },
    )

    root_hash = export.get("manifest", {}).get("integrity", {}).get("root_hash", {})
    summary = {
        "driver": "langchain",
        "scenario_id": run_config["scenario_id"],
        "status": status.get("status"),
        "outcome": extract_outcome_kind(status),
        "runpack_root_hash": root_hash.get("value"),
        "runpack_hash_algorithm": root_hash.get("algorithm"),
        "runpack_dir": runpack_dir,
    }
    print(json.dumps(summary, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as exc:
        print(json.dumps({"status": "fatal_error", "error": str(exc)}))
        sys.exit(1)
