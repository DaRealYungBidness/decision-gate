# Framework Adapter Examples

These examples show how to use the Decision Gate framework adapters with local
Decision Gate services. Each script assumes a DG server is running at
`http://127.0.0.1:8080/rpc` unless you override `DG_ENDPOINT`.

## Setup (local)

```bash
pip install -e sdks/python
pip install -e adapters/langchain
pip install -e adapters/crewai
pip install -e adapters/autogen
pip install -e adapters/openai_agents
```

## Run

```bash
python examples/frameworks/langchain_tool.py
python examples/frameworks/crewai_tool.py
python examples/frameworks/autogen_tool.py
python examples/frameworks/openai_agents_tool.py
```

## Notes

- These examples do not invoke LLM APIs; they exercise tool construction and
  Decision Gate calls directly.
- Set `DG_VALIDATE=1` to enable runtime JSON Schema validation.
