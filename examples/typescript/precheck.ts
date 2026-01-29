// examples/typescript/precheck.ts
// ============================================================================
// Module: Decision Gate Precheck (TypeScript)
// Description: Register a data shape and precheck a scenario with asserted data.
// Purpose: Runnable example for asserted-evidence workflows.
// Dependencies: Decision Gate SDK (local), fetch API
// ============================================================================

import {
  DecisionGateClient,
  validatePrecheckRequestWithAjv,
  validateScenarioDefineRequestWithAjv,
  validateSchemasRegisterRequestWithAjv,
} from "../../sdks/typescript/src/index.ts";

function loadEnvJson(name: string): unknown | null {
  const value = process.env[name];
  if (!value) {
    return null;
  }
  return JSON.parse(value);
}

function defaultPrecheckSpec(scenarioId: string): Record<string, unknown> {
  return {
    scenario_id: scenarioId,
    namespace_id: 1,
    spec_version: "1",
    default_tenant_id: null,
    policies: [],
    schemas: [],
    predicates: [
      {
        predicate: "deploy_env",
        query: {
          provider_id: "env",
          predicate: "get",
          params: { key: "DEPLOY_ENV" },
        },
        comparator: "equals",
        expected: "production",
        policy_tags: [],
        trust: null,
      },
    ],
    stages: [
      {
        stage_id: "main",
        entry_packets: [],
        gates: [
          {
            gate_id: "gate-time",
            requirement: { predicate: "deploy_env" },
            trust: null,
          },
        ],
        advance_to: { kind: "terminal" },
        timeout: null,
        on_timeout: "fail",
      },
    ],
  };
}

function defaultSchemaRecord(): Record<string, unknown> {
  return {
    schema_id: "asserted_payload",
    version: "v1",
    description: "Asserted payload schema.",
    tenant_id: 1,
    namespace_id: 1,
    created_at: { kind: "logical", value: 1 },
    schema: {
      type: "object",
      additionalProperties: false,
      properties: { deploy_env: { type: "string" } },
      required: ["deploy_env"],
    },
  };
}

async function maybeValidate(enabled: boolean, validator: (payload: any) => Promise<void>, payload: any) {
  if (enabled) {
    await validator(payload);
  }
}

async function main(): Promise<number> {
  const endpoint = process.env.DG_ENDPOINT ?? "http://127.0.0.1:8080/rpc";
  const token = process.env.DG_TOKEN;
  const validateEnabled = process.env.DG_VALIDATE === "1";

  const spec = (loadEnvJson("DG_SCENARIO_SPEC") as Record<string, unknown>) ??
    defaultPrecheckSpec("example-precheck");
  const scenarioId = spec.scenario_id as string;
  const schemaRecord = (loadEnvJson("DG_SCHEMA_RECORD") as Record<string, unknown>) ??
    defaultSchemaRecord();

  const client = new DecisionGateClient({
    endpoint,
    authToken: token,
  });

  const defineRequest = { spec };
  await maybeValidate(validateEnabled, validateScenarioDefineRequestWithAjv, defineRequest);
  const define = await client.scenario_define(defineRequest);

  const registerRequest = { record: schemaRecord };
  await maybeValidate(validateEnabled, validateSchemasRegisterRequestWithAjv, registerRequest);
  const registered = await client.schemas_register(registerRequest);

  const precheckRequest = {
    scenario_id: scenarioId,
    spec: null,
    stage_id: null,
    tenant_id: schemaRecord.tenant_id,
    namespace_id: schemaRecord.namespace_id,
    data_shape: {
      schema_id: schemaRecord.schema_id,
      version: schemaRecord.version,
    },
    payload: { deploy_env: "production" },
  };
  await maybeValidate(validateEnabled, validatePrecheckRequestWithAjv, precheckRequest);
  const precheck = await client.precheck(precheckRequest);

  console.log(JSON.stringify({ define, schema: registered, precheck }));
  return 0;
}

main()
  .then((code) => process.exit(code))
  .catch((error) => {
    console.log(JSON.stringify({ status: "fatal_error", error: String(error) }));
    process.exit(1);
  });
