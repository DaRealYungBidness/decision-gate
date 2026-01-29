# Decision Gate Framework Adapters

This directory contains thin framework adapters that expose the Decision Gate
Python SDK as native tools in popular agent frameworks.

## Adapters

- `adapters/langchain` - LangChain tools (`decision-gate-langchain`)
- `adapters/crewai` - CrewAI tools (`decision-gate-crewai`)
- `adapters/autogen` - AutoGen FunctionTool entries (`decision-gate-autogen`)
- `adapters/openai_agents` - OpenAI Agents SDK tools (`decision-gate-openai-agents`)

## Install (local)

Install the Decision Gate Python SDK first, then install the adapter you need:

```bash
pip install -e sdks/python
pip install -e adapters/langchain
```

## Notes

- Adapters are OSS and intentionally thin wrappers.
- Adapters are not published to PyPI yet.
