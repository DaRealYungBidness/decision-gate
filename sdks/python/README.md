<!--
sdks/python/README.md
============================================================================
Document: Decision Gate Python SDK
Description: Usage guide for the Decision Gate Python client.
Purpose: Provide quickstart instructions for SDK consumers.
============================================================================
-->

# Decision Gate Python SDK

Python client for Decision Gate MCP JSON-RPC tools.

## Typed + documented models

The SDK ships generated `TypedDict` models with field-level documentation
derived from the Decision Gate contract (schemas + tool notes).

## Install (local)

```bash
pip install -e .
```

## Usage

```python
from decision_gate import DecisionGateClient

client = DecisionGateClient(
    endpoint="http://127.0.0.1:8080/rpc",
    auth_token="token-1",
)

response = client.scenario_define({
    "spec": {
        "scenario_id": "example-scenario",
        "spec_version": "v1",
        "namespace_id": 1,
        "default_tenant_id": None,
        "policies": [],
        "predicates": [],
        "schemas": [],
        "stages": [],
    }
})
print(response)
```

## Validation (optional)

Runtime validation helpers are generated alongside the types. Install the
optional validator dependency and call the per-tool helpers.

```bash
pip install -e .[validation]
```

```python
from decision_gate import validate_scenario_define_request

payload = {"spec": { "scenario_id": "example-scenario", "spec_version": "v1", "namespace_id": 1,
    "default_tenant_id": None, "policies": [], "predicates": [], "schemas": [], "stages": [] }}
validate_scenario_define_request(payload)
```
