# Decision Gate LangChain Adapter

This adapter exposes Decision Gate SDK calls as LangChain tools.

## Install (local)

From the repo root:

```bash
pip install -e sdks/python
pip install -e adapters/langchain
```

## Usage

```python
from decision_gate import DecisionGateClient
from decision_gate_langchain import build_decision_gate_tools

client = DecisionGateClient(endpoint="http://127.0.0.1:8080/rpc")
tools = build_decision_gate_tools(client, validate=True)

# tools includes the full Decision Gate MCP tool surface.
```

## Notes

- This adapter is not published to PyPI yet.
- Tools are thin wrappers around the Decision Gate Python SDK.
