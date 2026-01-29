// examples/typescript/ci_gate.ts
// ============================================================================
// Module: Decision Gate CI Gate (TypeScript)
// Description: Trigger a run and export a runpack for audit.
// Purpose: Runnable example for CI/CD gating workflows.
// Dependencies: Decision Gate SDK (local), fetch API
// ============================================================================

import {
  DecisionGateClient,
  validateRunpackExportRequestWithAjv,
  validateScenarioDefineRequestWithAjv,
  validateScenarioStartRequestWithAjv,
  validateScenarioTriggerRequestWithAjv,
} from "../../sdks/typescript/src/index.ts";

import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

function loadEnvJson(name: string): unknown | null {
  const value = process.env[name];
  if (!value) {
    return null;
  }
  return JSON.parse(value);
}

function defaultTimeAfterSpec(scenarioId: string, threshold: number): Record<string, unknown> {
  return {
    scenario_id: scenarioId,
    namespace_id: 1,
    spec_version: "1",
    default_tenant_id: null,
    policies: [],
    schemas: [],
    predicates: [
      {
        predicate: "after",
        query: {
          provider_id: "time",
          predicate: "after",
          params: { timestamp: threshold },
        },
        comparator: "equals",
        expected: true,
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
            requirement: { predicate: "after" },
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

function defaultRunConfig(scenarioId: string, runId: string): Record<string, unknown> {
  return {
    scenario_id: scenarioId,
    run_id: runId,
    tenant_id: 1,
    namespace_id: 1,
    policy_tags: [],
    dispatch_targets: [],
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
    defaultTimeAfterSpec("example-ci-gate", 0);
  const scenarioId = spec.scenario_id as string;
  const runConfig = (loadEnvJson("DG_RUN_CONFIG") as Record<string, unknown>) ??
    defaultRunConfig(scenarioId, "run-ci-1");
  const startedAt = loadEnvJson("DG_STARTED_AT") ?? { kind: "logical", value: 1 };

  const client = new DecisionGateClient({
    endpoint,
    authToken: token,
  });

  const defineRequest = { spec };
  await maybeValidate(validateEnabled, validateScenarioDefineRequestWithAjv, defineRequest);
  const define = await client.scenario_define(defineRequest);

  const startRequest = {
    scenario_id: scenarioId,
    run_config: runConfig,
    started_at: startedAt,
    issue_entry_packets: false,
  };
  await maybeValidate(validateEnabled, validateScenarioStartRequestWithAjv, startRequest);
  const start = await client.scenario_start(startRequest);

  const triggerRequest = {
    scenario_id: scenarioId,
    trigger: {
      trigger_id: "trigger-ci-1",
      kind: "tick",
      source_id: "ci",
      tenant_id: runConfig.tenant_id,
      namespace_id: runConfig.namespace_id,
      run_id: runConfig.run_id,
      time: { kind: "logical", value: 2 },
      payload: null,
      correlation_id: null,
    },
  };
  await maybeValidate(validateEnabled, validateScenarioTriggerRequestWithAjv, triggerRequest);
  const trigger = await client.scenario_trigger(triggerRequest);

  const outputDir = mkdtempSync(join(tmpdir(), "decision-gate-runpack-"));
  const exportRequest = {
    scenario_id: scenarioId,
    run_id: runConfig.run_id,
    tenant_id: runConfig.tenant_id,
    namespace_id: runConfig.namespace_id,
    output_dir: outputDir,
    manifest_name: "manifest.json",
    generated_at: { kind: "logical", value: 3 },
    include_verification: false,
  };
  await maybeValidate(validateEnabled, validateRunpackExportRequestWithAjv, exportRequest);
  const runpack = await client.runpack_export(exportRequest);

  console.log(JSON.stringify({ define, start, trigger, runpack }));
  return 0;
}

main()
  .then((code) => process.exit(code))
  .catch((error) => {
    console.log(JSON.stringify({ status: "fatal_error", error: String(error) }));
    process.exit(1);
  });
