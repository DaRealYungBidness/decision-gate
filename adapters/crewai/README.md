# Decision Gate CrewAI Adapter

This adapter exposes Decision Gate SDK calls as CrewAI tools.

## Install (local)

From the repo root:

```bash
pip install -e sdks/python
pip install -e adapters/crewai
```

## Usage

```python
from decision_gate import DecisionGateClient
from decision_gate_crewai import build_decision_gate_tools

client = DecisionGateClient(endpoint="http://127.0.0.1:8080/rpc")
tools = build_decision_gate_tools(client, validate=True)
```

## Notes

- This adapter is not published to PyPI yet.
- Tool outputs are JSON strings (safe for LLM tool responses).
