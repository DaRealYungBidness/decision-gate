# Decision Gate AutoGen Adapter

This adapter exposes Decision Gate SDK calls as AutoGen FunctionTool entries.

## Install (local)

From the repo root:

```bash
pip install -e sdks/python
pip install -e adapters/autogen
```

## Usage

```python
from decision_gate import DecisionGateClient
from decision_gate_autogen import build_decision_gate_tools

client = DecisionGateClient(endpoint="http://127.0.0.1:8080/rpc")
tools = build_decision_gate_tools(client, validate=True)
```

## Notes

- This adapter is not published to PyPI yet.
- The tool list mirrors the full Decision Gate MCP tool surface.
