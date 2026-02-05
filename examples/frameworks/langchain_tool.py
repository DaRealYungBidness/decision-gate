#!/usr/bin/env python3
# examples/frameworks/langchain_tool.py
# ============================================================================
# Module: Decision Gate LangChain Adapter Example
# Description: Invoke Decision Gate precheck via LangChain tool wrapper.
# ============================================================================

from __future__ import annotations

import json
import os
import sys

from decision_gate import DecisionGateClient
from decision_gate_langchain import build_decision_gate_tools

from common import find_tool, prepare_precheck


def main() -> int:
    endpoint = os.environ.get("DG_ENDPOINT", "http://127.0.0.1:8080/rpc")
    token = os.environ.get("DG_TOKEN")
    validate_enabled = os.environ.get("DG_VALIDATE") == "1"

    client = DecisionGateClient(endpoint=endpoint, auth_token=token)
    tools = build_decision_gate_tools(client, validate=validate_enabled)
    precheck_tool = find_tool(tools, "decision_gate_precheck")

    precheck_request, _meta = prepare_precheck(client, validate_enabled=validate_enabled)
    result = precheck_tool.invoke({"request": precheck_request})

    print(json.dumps({"tool": "langchain", "result": result}))
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as exc:
        print(json.dumps({"status": "fatal_error", "error": str(exc)}))
        sys.exit(1)
