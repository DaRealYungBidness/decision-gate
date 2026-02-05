#!/usr/bin/env python3
# examples/frameworks/crewai_tool.py
# ============================================================================
# Module: Decision Gate CrewAI Adapter Example
# Description: Invoke Decision Gate precheck via CrewAI tool wrapper.
# ============================================================================

from __future__ import annotations

import json
import os
import sys

from decision_gate import DecisionGateClient
from decision_gate_crewai import DecisionGatePrecheckTool

from common import prepare_precheck


def main() -> int:
    endpoint = os.environ.get("DG_ENDPOINT", "http://127.0.0.1:8080/rpc")
    token = os.environ.get("DG_TOKEN")
    validate_enabled = os.environ.get("DG_VALIDATE") == "1"

    client = DecisionGateClient(endpoint=endpoint, auth_token=token)
    tool = DecisionGatePrecheckTool(client=client, validate=validate_enabled)

    precheck_request, _meta = prepare_precheck(client, validate_enabled=validate_enabled)
    result_json = tool.run(request=precheck_request)

    print(json.dumps({"tool": "crewai", "result": json.loads(result_json)}))
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as exc:
        print(json.dumps({"status": "fatal_error", "error": str(exc)}))
        sys.exit(1)
