#!/usr/bin/env python3
# examples/frameworks/autogen_tool.py
# ============================================================================
# Module: Decision Gate AutoGen Adapter Example
# Description: Invoke Decision Gate precheck via AutoGen FunctionTool wrapper.
# ============================================================================

from __future__ import annotations

import asyncio
import json
import os
import sys

from autogen_core import CancellationToken
from decision_gate import DecisionGateClient
from decision_gate_autogen import build_decision_gate_tools

from common import find_tool, prepare_precheck


def main() -> int:
    endpoint = os.environ.get("DG_ENDPOINT", "http://127.0.0.1:8080/rpc")
    token = os.environ.get("DG_TOKEN")
    validate_enabled = os.environ.get("DG_VALIDATE") == "1"

    client = DecisionGateClient(endpoint=endpoint, auth_token=token)
    tools = build_decision_gate_tools(client, validate=validate_enabled)
    precheck_tool = find_tool(tools, "decision_gate_precheck")

    precheck_request, _meta = prepare_precheck(client, validate_enabled=validate_enabled)

    async def run_tool() -> dict:
        cancellation = CancellationToken()
        return await precheck_tool.run_json({"request": precheck_request}, cancellation)

    result = asyncio.run(run_tool())

    print(json.dumps({"tool": "autogen", "result": result}))
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as exc:
        print(json.dumps({"status": "fatal_error", "error": str(exc)}))
        sys.exit(1)
