<!--
sdks/typescript/README.md
============================================================================
Document: Decision Gate TypeScript SDK
Description: Usage guide for the Decision Gate TypeScript client.
Purpose: Provide quickstart instructions for SDK consumers.
============================================================================
-->

# Decision Gate TypeScript SDK

TypeScript client for Decision Gate MCP JSON-RPC tools.

## Typed + documented models

The SDK ships generated interfaces with field-level documentation derived from
the Decision Gate contract (schemas + tool notes).

## Install (local)

```bash
npm install
npm run build
```

## Usage

```typescript
import { DecisionGateClient } from "./src/index";

const client = new DecisionGateClient({
  endpoint: "http://127.0.0.1:8080/rpc",
  authToken: "token-1",
});

const response = await client.scenario_define({
  spec: {
    scenario_id: "example-scenario",
    spec_version: "v1",
    namespace_id: 1,
    default_tenant_id: null,
    policies: [],
    predicates: [],
    schemas: [],
    stages: [],
  },
});
console.log(response);
```

## Validation (optional)

Runtime validation helpers are generated alongside the types. Install a JSON
schema validator (Ajv) and call the per-tool helpers.

```bash
npm install ajv
```

```typescript
import { validateScenarioDefineRequestWithAjv } from "./src/index";

await validateScenarioDefineRequestWithAjv({
  spec: {
    scenario_id: "example-scenario",
    spec_version: "v1",
    namespace_id: 1,
    default_tenant_id: null,
    policies: [],
    predicates: [],
    schemas: [],
    stages: [],
  },
});
```
