#!/usr/bin/env python3
# examples/frameworks/openai_agents_tool.py
# ============================================================================
# Module: Decision Gate OpenAI Agents Adapter Example
# Description: Construct Decision Gate function tools for OpenAI Agents SDK.
# ============================================================================

from __future__ import annotations

import json
import os
import sys

from decision_gate import DecisionGateClient
from decision_gate_openai_agents import build_decision_gate_tools


def main() -> int:
    endpoint = os.environ.get("DG_ENDPOINT", "http://127.0.0.1:8080/rpc")
    token = os.environ.get("DG_TOKEN")
    validate_enabled = os.environ.get("DG_VALIDATE") == "1"

    client = DecisionGateClient(endpoint=endpoint, auth_token=token)
    tools = build_decision_gate_tools(client, validate=validate_enabled)

    print(json.dumps({"tool": "openai_agents", "tool_count": len(tools)}))
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as exc:
        print(json.dumps({"status": "fatal_error", "error": str(exc)}))
        sys.exit(1)
