# Decision Gate OpenAI Agents SDK Adapter

This adapter exposes Decision Gate SDK calls as OpenAI Agents function tools.

## Install (local)

From the repo root:

```bash
pip install -e sdks/python
pip install -e adapters/openai_agents
```

## Usage

```python
from decision_gate import DecisionGateClient
from decision_gate_openai_agents import build_decision_gate_tools

client = DecisionGateClient(endpoint="http://127.0.0.1:8080/rpc")
tools = build_decision_gate_tools(client, validate=True)
```

## Notes

- This adapter is not published to PyPI yet.
- Use with the OpenAI Agents SDK tool APIs.
