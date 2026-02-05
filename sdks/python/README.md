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

```bash dg-skip dg-reason="local install step" dg-expires=2026-12-31
pip install -e .
```

## Usage

```python dg-run dg-level=fast dg-server=mcp dg-session=sdk-python dg-requires=python,cargo
import os

from decision_gate import DecisionGateClient

endpoint = os.environ.get("DG_ENDPOINT", "http://127.0.0.1:8080/rpc")
token = os.environ.get("DG_TOKEN", "token-1")

client = DecisionGateClient(endpoint=endpoint, auth_token=token)

response = client.scenario_define(
    {
        "spec": {
            "scenario_id": "example-scenario",
            "spec_version": "1",
            "namespace_id": 1,
            "default_tenant_id": None,
            "policies": [],
            "conditions": [],
            "schemas": [],
            "stages": [],
        }
    }
)
print(response)
```

## Validation (optional)

Runtime validation helpers are generated alongside the types. Install the
optional validator dependency and call the per-tool helpers.

```bash dg-skip dg-reason="optional validation extra" dg-expires=2026-12-31
pip install -e .[validation]
```

```python dg-skip dg-reason="requires validation extra" dg-expires=2026-12-31
from decision_gate import validate_scenario_define_request

payload = {
    "spec": {
        "scenario_id": "example-scenario",
        "spec_version": "1",
        "namespace_id": 1,
        "default_tenant_id": None,
        "policies": [],
        "conditions": [],
        "schemas": [],
        "stages": [],
    }
}
validate_scenario_define_request(payload)
```
