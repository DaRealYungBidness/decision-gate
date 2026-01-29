export type JsonPrimitive = string | number | boolean | null;
export type JsonValue = JsonPrimitive | JsonValue[] | {
    [key: string]: JsonValue;
};
export declare const TOOL_NAMES: readonly ["scenario_define", "scenario_start", "scenario_status", "scenario_next", "scenario_submit", "scenario_trigger", "evidence_query", "runpack_export", "runpack_verify", "providers_list", "provider_contract_get", "provider_schema_get", "schemas_register", "schemas_list", "schemas_get", "scenarios_list", "precheck"];
export declare const TOOL_DESCRIPTIONS: Record<string, string>;
export declare const TOOL_NOTES: Record<string, string[]>;
export interface ScenarioDefineRequest {
    /** Scenario specification to register. */
    spec: JsonValue;
}
export interface ScenarioDefineResponse {
    /** Scenario identifier. */
    scenario_id: string;
    spec_hash: Record<string, JsonValue>;
}
export declare const ScenarioDefine_INPUT_SCHEMA: {
    readonly $schema: "https://json-schema.org/draft/2020-12/schema";
    readonly additionalProperties: false;
    readonly properties: {
        readonly spec: {
            readonly $ref: "decision-gate://contract/schemas/scenario.schema.json";
            readonly description: "Scenario specification to register.";
        };
    };
    readonly required: readonly ["spec"];
    readonly type: "object";
};
export declare const ScenarioDefine_OUTPUT_SCHEMA: {
    readonly $schema: "https://json-schema.org/draft/2020-12/schema";
    readonly additionalProperties: false;
    readonly properties: {
        readonly scenario_id: {
            readonly description: "Scenario identifier.";
            readonly type: "string";
        };
        readonly spec_hash: {
            readonly additionalProperties: false;
            readonly properties: {
                readonly algorithm: {
                    readonly enum: readonly ["sha256"];
                    readonly type: "string";
                };
                readonly value: {
                    readonly description: "Lowercase hex digest.";
                    readonly type: "string";
                };
            };
            readonly required: readonly ["algorithm", "value"];
            readonly type: "object";
        };
    };
    readonly required: readonly ["scenario_id", "spec_hash"];
    readonly type: "object";
};
export interface ScenarioStartRequest {
    /** Issue entry packets immediately. */
    issue_entry_packets: boolean;
    /** Run configuration and dispatch targets. */
    run_config: Record<string, JsonValue>;
    /** Scenario identifier. */
    scenario_id: string;
    /** Caller-supplied run start timestamp. */
    started_at: Record<string, JsonValue>;
}
export interface ScenarioStartResponse {
    /** Current stage identifier. */
    current_stage_id: string;
    decisions: Array<Record<string, JsonValue>>;
    dispatch_targets: Array<Record<string, JsonValue>>;
    gate_evals: Array<Record<string, JsonValue>>;
    /** Namespace identifier. Constraints: Minimum: 1. */
    namespace_id: number;
    packets: Array<Record<string, JsonValue>>;
    /** Run identifier. */
    run_id: string;
    /** Scenario identifier. */
    scenario_id: string;
    spec_hash: Record<string, JsonValue>;
    stage_entered_at: Record<string, JsonValue>;
    /** Constraints: Allowed values: "active", "completed", "failed". */
    status: "active" | "completed" | "failed";
    submissions: Array<Record<string, JsonValue>>;
    /** Tenant identifier. Constraints: Minimum: 1. */
    tenant_id: number;
    tool_calls: Array<Record<string, JsonValue>>;
    triggers: Array<Record<string, JsonValue>>;
}
export declare const ScenarioStart_INPUT_SCHEMA: {
    readonly $schema: "https://json-schema.org/draft/2020-12/schema";
    readonly additionalProperties: false;
    readonly properties: {
        readonly issue_entry_packets: {
            readonly description: "Issue entry packets immediately.";
            readonly type: "boolean";
        };
        readonly run_config: {
            readonly additionalProperties: false;
            readonly description: "Run configuration and dispatch targets.";
            readonly properties: {
                readonly dispatch_targets: {
                    readonly items: {
                        readonly oneOf: readonly [{
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly agent_id: {
                                    readonly description: "Agent identifier.";
                                    readonly type: "string";
                                };
                                readonly kind: {
                                    readonly const: "agent";
                                };
                            };
                            readonly required: readonly ["kind", "agent_id"];
                            readonly type: "object";
                        }, {
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly kind: {
                                    readonly const: "session";
                                };
                                readonly session_id: {
                                    readonly description: "Session identifier.";
                                    readonly type: "string";
                                };
                            };
                            readonly required: readonly ["kind", "session_id"];
                            readonly type: "object";
                        }, {
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly kind: {
                                    readonly const: "external";
                                };
                                readonly system: {
                                    readonly description: "External system name.";
                                    readonly type: "string";
                                };
                                readonly target: {
                                    readonly description: "External system target.";
                                    readonly type: "string";
                                };
                            };
                            readonly required: readonly ["kind", "system", "target"];
                            readonly type: "object";
                        }, {
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly channel: {
                                    readonly description: "Broadcast channel identifier.";
                                    readonly type: "string";
                                };
                                readonly kind: {
                                    readonly const: "channel";
                                };
                            };
                            readonly required: readonly ["kind", "channel"];
                            readonly type: "object";
                        }];
                    };
                    readonly type: "array";
                };
                readonly namespace_id: {
                    readonly description: "Namespace identifier.";
                    readonly minimum: 1;
                    readonly type: "integer";
                };
                readonly policy_tags: {
                    readonly description: "Policy tags applied to the run.";
                    readonly items: {
                        readonly type: "string";
                    };
                    readonly type: "array";
                };
                readonly run_id: {
                    readonly description: "Run identifier.";
                    readonly type: "string";
                };
                readonly scenario_id: {
                    readonly description: "Scenario identifier.";
                    readonly type: "string";
                };
                readonly tenant_id: {
                    readonly description: "Tenant identifier.";
                    readonly minimum: 1;
                    readonly type: "integer";
                };
            };
            readonly required: readonly ["tenant_id", "run_id", "scenario_id", "dispatch_targets", "policy_tags"];
            readonly type: "object";
        };
        readonly scenario_id: {
            readonly description: "Scenario identifier.";
            readonly type: "string";
        };
        readonly started_at: {
            readonly description: "Caller-supplied run start timestamp.";
            readonly oneOf: readonly [{
                readonly additionalProperties: false;
                readonly properties: {
                    readonly kind: {
                        readonly const: "unix_millis";
                    };
                    readonly value: {
                        readonly type: "integer";
                    };
                };
                readonly required: readonly ["kind", "value"];
                readonly type: "object";
            }, {
                readonly additionalProperties: false;
                readonly properties: {
                    readonly kind: {
                        readonly const: "logical";
                    };
                    readonly value: {
                        readonly minimum: 0;
                        readonly type: "integer";
                    };
                };
                readonly required: readonly ["kind", "value"];
                readonly type: "object";
            }];
        };
    };
    readonly required: readonly ["scenario_id", "run_config", "started_at", "issue_entry_packets"];
    readonly type: "object";
};
export declare const ScenarioStart_OUTPUT_SCHEMA: {
    readonly additionalProperties: false;
    readonly properties: {
        readonly current_stage_id: {
            readonly description: "Current stage identifier.";
            readonly type: "string";
        };
        readonly decisions: {
            readonly items: {
                readonly additionalProperties: false;
                readonly properties: {
                    readonly correlation_id: {
                        readonly oneOf: readonly [{
                            readonly type: "null";
                        }, {
                            readonly description: "Correlation identifier.";
                            readonly type: "string";
                        }];
                    };
                    readonly decided_at: {
                        readonly oneOf: readonly [{
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly kind: {
                                    readonly const: "unix_millis";
                                };
                                readonly value: {
                                    readonly type: "integer";
                                };
                            };
                            readonly required: readonly ["kind", "value"];
                            readonly type: "object";
                        }, {
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly kind: {
                                    readonly const: "logical";
                                };
                                readonly value: {
                                    readonly minimum: 0;
                                    readonly type: "integer";
                                };
                            };
                            readonly required: readonly ["kind", "value"];
                            readonly type: "object";
                        }];
                    };
                    readonly decision_id: {
                        readonly description: "Decision identifier.";
                        readonly type: "string";
                    };
                    readonly outcome: {
                        readonly oneOf: readonly [{
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly kind: {
                                    readonly const: "start";
                                };
                                readonly stage_id: {
                                    readonly description: "Initial stage identifier.";
                                    readonly type: "string";
                                };
                            };
                            readonly required: readonly ["kind", "stage_id"];
                            readonly type: "object";
                        }, {
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly kind: {
                                    readonly const: "complete";
                                };
                                readonly stage_id: {
                                    readonly description: "Terminal stage identifier.";
                                    readonly type: "string";
                                };
                            };
                            readonly required: readonly ["kind", "stage_id"];
                            readonly type: "object";
                        }, {
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly from_stage: {
                                    readonly description: "Previous stage identifier.";
                                    readonly type: "string";
                                };
                                readonly kind: {
                                    readonly const: "advance";
                                };
                                readonly timeout: {
                                    readonly type: "boolean";
                                };
                                readonly to_stage: {
                                    readonly description: "Next stage identifier.";
                                    readonly type: "string";
                                };
                            };
                            readonly required: readonly ["kind", "from_stage", "to_stage", "timeout"];
                            readonly type: "object";
                        }, {
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly kind: {
                                    readonly const: "hold";
                                };
                                readonly summary: {
                                    readonly additionalProperties: false;
                                    readonly properties: {
                                        readonly policy_tags: {
                                            readonly description: "Policy tags applied to the summary.";
                                            readonly items: {
                                                readonly type: "string";
                                            };
                                            readonly type: "array";
                                        };
                                        readonly retry_hint: {
                                            readonly oneOf: readonly [{
                                                readonly type: "null";
                                            }, {
                                                readonly description: "Optional retry hint.";
                                                readonly type: "string";
                                            }];
                                        };
                                        readonly status: {
                                            readonly description: "Summary status.";
                                            readonly type: "string";
                                        };
                                        readonly unmet_gates: {
                                            readonly items: {
                                                readonly description: "Gate identifier.";
                                                readonly type: "string";
                                            };
                                            readonly type: "array";
                                        };
                                    };
                                    readonly required: readonly ["status", "unmet_gates", "retry_hint", "policy_tags"];
                                    readonly type: "object";
                                };
                            };
                            readonly required: readonly ["kind", "summary"];
                            readonly type: "object";
                        }, {
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly kind: {
                                    readonly const: "fail";
                                };
                                readonly reason: {
                                    readonly description: "Failure reason.";
                                    readonly type: "string";
                                };
                            };
                            readonly required: readonly ["kind", "reason"];
                            readonly type: "object";
                        }];
                    };
                    readonly seq: {
                        readonly minimum: 0;
                        readonly type: "integer";
                    };
                    readonly stage_id: {
                        readonly description: "Stage identifier.";
                        readonly type: "string";
                    };
                    readonly trigger_id: {
                        readonly description: "Trigger identifier.";
                        readonly type: "string";
                    };
                };
                readonly required: readonly ["decision_id", "seq", "trigger_id", "stage_id", "decided_at", "outcome", "correlation_id"];
                readonly type: "object";
            };
            readonly type: "array";
        };
        readonly dispatch_targets: {
            readonly items: {
                readonly oneOf: readonly [{
                    readonly additionalProperties: false;
                    readonly properties: {
                        readonly agent_id: {
                            readonly description: "Agent identifier.";
                            readonly type: "string";
                        };
                        readonly kind: {
                            readonly const: "agent";
                        };
                    };
                    readonly required: readonly ["kind", "agent_id"];
                    readonly type: "object";
                }, {
                    readonly additionalProperties: false;
                    readonly properties: {
                        readonly kind: {
                            readonly const: "session";
                        };
                        readonly session_id: {
                            readonly description: "Session identifier.";
                            readonly type: "string";
                        };
                    };
                    readonly required: readonly ["kind", "session_id"];
                    readonly type: "object";
                }, {
                    readonly additionalProperties: false;
                    readonly properties: {
                        readonly kind: {
                            readonly const: "external";
                        };
                        readonly system: {
                            readonly description: "External system name.";
                            readonly type: "string";
                        };
                        readonly target: {
                            readonly description: "External system target.";
                            readonly type: "string";
                        };
                    };
                    readonly required: readonly ["kind", "system", "target"];
                    readonly type: "object";
                }, {
                    readonly additionalProperties: false;
                    readonly properties: {
                        readonly channel: {
                            readonly description: "Broadcast channel identifier.";
                            readonly type: "string";
                        };
                        readonly kind: {
                            readonly const: "channel";
                        };
                    };
                    readonly required: readonly ["kind", "channel"];
                    readonly type: "object";
                }];
            };
            readonly type: "array";
        };
        readonly gate_evals: {
            readonly items: {
                readonly additionalProperties: false;
                readonly properties: {
                    readonly evaluation: {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly gate_id: {
                                readonly description: "Gate identifier.";
                                readonly type: "string";
                            };
                            readonly status: {
                                readonly description: "Tri-state evaluation result.";
                                readonly enum: readonly ["True", "False", "Unknown"];
                                readonly type: "string";
                            };
                            readonly trace: {
                                readonly items: {
                                    readonly additionalProperties: false;
                                    readonly properties: {
                                        readonly predicate: {
                                            readonly description: "Predicate identifier.";
                                            readonly type: "string";
                                        };
                                        readonly status: {
                                            readonly description: "Tri-state evaluation result.";
                                            readonly enum: readonly ["True", "False", "Unknown"];
                                            readonly type: "string";
                                        };
                                    };
                                    readonly required: readonly ["predicate", "status"];
                                    readonly type: "object";
                                };
                                readonly type: "array";
                            };
                        };
                        readonly required: readonly ["gate_id", "status", "trace"];
                        readonly type: "object";
                    };
                    readonly evidence: {
                        readonly items: {
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly predicate: {
                                    readonly description: "Predicate identifier.";
                                    readonly type: "string";
                                };
                                readonly result: {
                                    readonly additionalProperties: false;
                                    readonly properties: {
                                        readonly content_type: {
                                            readonly oneOf: readonly [{
                                                readonly type: "null";
                                            }, {
                                                readonly description: "Evidence content type.";
                                                readonly type: "string";
                                            }];
                                        };
                                        readonly error: {
                                            readonly oneOf: readonly [{
                                                readonly type: "null";
                                            }, {
                                                readonly additionalProperties: false;
                                                readonly properties: {
                                                    readonly code: {
                                                        readonly description: "Stable error code.";
                                                        readonly type: "string";
                                                    };
                                                    readonly details: {
                                                        readonly oneOf: readonly [{
                                                            readonly type: "null";
                                                        }, {
                                                            readonly description: "Optional structured error details.";
                                                            readonly type: readonly ["null", "boolean", "number", "string", "array", "object"];
                                                        }];
                                                    };
                                                    readonly message: {
                                                        readonly description: "Error message.";
                                                        readonly type: "string";
                                                    };
                                                };
                                                readonly required: readonly ["code", "message", "details"];
                                                readonly type: "object";
                                            }];
                                        };
                                        readonly evidence_anchor: {
                                            readonly oneOf: readonly [{
                                                readonly type: "null";
                                            }, {
                                                readonly additionalProperties: false;
                                                readonly properties: {
                                                    readonly anchor_type: {
                                                        readonly description: "Anchor type identifier.";
                                                        readonly type: "string";
                                                    };
                                                    readonly anchor_value: {
                                                        readonly description: "Anchor value.";
                                                        readonly type: "string";
                                                    };
                                                };
                                                readonly required: readonly ["anchor_type", "anchor_value"];
                                                readonly type: "object";
                                            }];
                                        };
                                        readonly evidence_hash: {
                                            readonly oneOf: readonly [{
                                                readonly type: "null";
                                            }, {
                                                readonly additionalProperties: false;
                                                readonly properties: {
                                                    readonly algorithm: {
                                                        readonly enum: readonly ["sha256"];
                                                        readonly type: "string";
                                                    };
                                                    readonly value: {
                                                        readonly description: "Lowercase hex digest.";
                                                        readonly type: "string";
                                                    };
                                                };
                                                readonly required: readonly ["algorithm", "value"];
                                                readonly type: "object";
                                            }];
                                        };
                                        readonly evidence_ref: {
                                            readonly oneOf: readonly [{
                                                readonly type: "null";
                                            }, {
                                                readonly additionalProperties: false;
                                                readonly properties: {
                                                    readonly uri: {
                                                        readonly description: "Evidence reference URI.";
                                                        readonly type: "string";
                                                    };
                                                };
                                                readonly required: readonly ["uri"];
                                                readonly type: "object";
                                            }];
                                        };
                                        readonly lane: {
                                            readonly description: "Trust lane classification for evidence.";
                                            readonly enum: readonly ["verified", "asserted"];
                                            readonly type: "string";
                                        };
                                        readonly signature: {
                                            readonly oneOf: readonly [{
                                                readonly type: "null";
                                            }, {
                                                readonly additionalProperties: false;
                                                readonly properties: {
                                                    readonly key_id: {
                                                        readonly description: "Signing key identifier.";
                                                        readonly type: "string";
                                                    };
                                                    readonly scheme: {
                                                        readonly description: "Signature scheme identifier.";
                                                        readonly type: "string";
                                                    };
                                                    readonly signature: {
                                                        readonly items: {
                                                            readonly maximum: 255;
                                                            readonly minimum: 0;
                                                            readonly type: "integer";
                                                        };
                                                        readonly type: "array";
                                                    };
                                                };
                                                readonly required: readonly ["scheme", "key_id", "signature"];
                                                readonly type: "object";
                                            }];
                                        };
                                        readonly value: {
                                            readonly oneOf: readonly [{
                                                readonly type: "null";
                                            }, {
                                                readonly oneOf: readonly [{
                                                    readonly additionalProperties: false;
                                                    readonly properties: {
                                                        readonly kind: {
                                                            readonly const: "json";
                                                        };
                                                        readonly value: {
                                                            readonly description: "Evidence JSON value.";
                                                            readonly type: readonly ["null", "boolean", "number", "string", "array", "object"];
                                                        };
                                                    };
                                                    readonly required: readonly ["kind", "value"];
                                                    readonly type: "object";
                                                }, {
                                                    readonly additionalProperties: false;
                                                    readonly properties: {
                                                        readonly kind: {
                                                            readonly const: "bytes";
                                                        };
                                                        readonly value: {
                                                            readonly items: {
                                                                readonly maximum: 255;
                                                                readonly minimum: 0;
                                                                readonly type: "integer";
                                                            };
                                                            readonly type: "array";
                                                        };
                                                    };
                                                    readonly required: readonly ["kind", "value"];
                                                    readonly type: "object";
                                                }];
                                            }];
                                        };
                                    };
                                    readonly required: readonly ["value", "lane", "error", "evidence_hash", "evidence_ref", "evidence_anchor", "signature", "content_type"];
                                    readonly type: "object";
                                };
                                readonly status: {
                                    readonly description: "Tri-state evaluation result.";
                                    readonly enum: readonly ["True", "False", "Unknown"];
                                    readonly type: "string";
                                };
                            };
                            readonly required: readonly ["predicate", "status", "result"];
                            readonly type: "object";
                        };
                        readonly type: "array";
                    };
                    readonly stage_id: {
                        readonly description: "Stage identifier.";
                        readonly type: "string";
                    };
                    readonly trigger_id: {
                        readonly description: "Trigger identifier.";
                        readonly type: "string";
                    };
                };
                readonly required: readonly ["trigger_id", "stage_id", "evaluation", "evidence"];
                readonly type: "object";
            };
            readonly type: "array";
        };
        readonly namespace_id: {
            readonly description: "Namespace identifier.";
            readonly minimum: 1;
            readonly type: "integer";
        };
        readonly packets: {
            readonly items: {
                readonly additionalProperties: false;
                readonly properties: {
                    readonly decision_id: {
                        readonly description: "Decision identifier.";
                        readonly type: "string";
                    };
                    readonly envelope: {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly content_hash: {
                                readonly additionalProperties: false;
                                readonly properties: {
                                    readonly algorithm: {
                                        readonly enum: readonly ["sha256"];
                                        readonly type: "string";
                                    };
                                    readonly value: {
                                        readonly description: "Lowercase hex digest.";
                                        readonly type: "string";
                                    };
                                };
                                readonly required: readonly ["algorithm", "value"];
                                readonly type: "object";
                            };
                            readonly content_type: {
                                readonly description: "Packet content type.";
                                readonly type: "string";
                            };
                            readonly correlation_id: {
                                readonly oneOf: readonly [{
                                    readonly type: "null";
                                }, {
                                    readonly description: "Correlation identifier.";
                                    readonly type: "string";
                                }];
                            };
                            readonly expiry: {
                                readonly oneOf: readonly [{
                                    readonly type: "null";
                                }, {
                                    readonly oneOf: readonly [{
                                        readonly additionalProperties: false;
                                        readonly properties: {
                                            readonly kind: {
                                                readonly const: "unix_millis";
                                            };
                                            readonly value: {
                                                readonly type: "integer";
                                            };
                                        };
                                        readonly required: readonly ["kind", "value"];
                                        readonly type: "object";
                                    }, {
                                        readonly additionalProperties: false;
                                        readonly properties: {
                                            readonly kind: {
                                                readonly const: "logical";
                                            };
                                            readonly value: {
                                                readonly minimum: 0;
                                                readonly type: "integer";
                                            };
                                        };
                                        readonly required: readonly ["kind", "value"];
                                        readonly type: "object";
                                    }];
                                }];
                            };
                            readonly issued_at: {
                                readonly oneOf: readonly [{
                                    readonly additionalProperties: false;
                                    readonly properties: {
                                        readonly kind: {
                                            readonly const: "unix_millis";
                                        };
                                        readonly value: {
                                            readonly type: "integer";
                                        };
                                    };
                                    readonly required: readonly ["kind", "value"];
                                    readonly type: "object";
                                }, {
                                    readonly additionalProperties: false;
                                    readonly properties: {
                                        readonly kind: {
                                            readonly const: "logical";
                                        };
                                        readonly value: {
                                            readonly minimum: 0;
                                            readonly type: "integer";
                                        };
                                    };
                                    readonly required: readonly ["kind", "value"];
                                    readonly type: "object";
                                }];
                            };
                            readonly packet_id: {
                                readonly description: "Packet identifier.";
                                readonly type: "string";
                            };
                            readonly run_id: {
                                readonly description: "Run identifier.";
                                readonly type: "string";
                            };
                            readonly scenario_id: {
                                readonly description: "Scenario identifier.";
                                readonly type: "string";
                            };
                            readonly schema_id: {
                                readonly description: "Schema identifier.";
                                readonly type: "string";
                            };
                            readonly stage_id: {
                                readonly description: "Stage identifier.";
                                readonly type: "string";
                            };
                            readonly visibility: {
                                readonly additionalProperties: false;
                                readonly properties: {
                                    readonly labels: {
                                        readonly description: "Visibility labels.";
                                        readonly items: {
                                            readonly type: "string";
                                        };
                                        readonly type: "array";
                                    };
                                    readonly policy_tags: {
                                        readonly description: "Policy tags.";
                                        readonly items: {
                                            readonly type: "string";
                                        };
                                        readonly type: "array";
                                    };
                                };
                                readonly required: readonly ["labels", "policy_tags"];
                                readonly type: "object";
                            };
                        };
                        readonly required: readonly ["scenario_id", "run_id", "stage_id", "packet_id", "schema_id", "content_type", "content_hash", "visibility", "expiry", "correlation_id", "issued_at"];
                        readonly type: "object";
                    };
                    readonly payload: {
                        readonly oneOf: readonly [{
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly kind: {
                                    readonly const: "json";
                                };
                                readonly value: {
                                    readonly description: "Inline JSON payload.";
                                    readonly type: readonly ["null", "boolean", "number", "string", "array", "object"];
                                };
                            };
                            readonly required: readonly ["kind", "value"];
                            readonly type: "object";
                        }, {
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly bytes: {
                                    readonly items: {
                                        readonly maximum: 255;
                                        readonly minimum: 0;
                                        readonly type: "integer";
                                    };
                                    readonly type: "array";
                                };
                                readonly kind: {
                                    readonly const: "bytes";
                                };
                            };
                            readonly required: readonly ["kind", "bytes"];
                            readonly type: "object";
                        }, {
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly content_ref: {
                                    readonly additionalProperties: false;
                                    readonly properties: {
                                        readonly content_hash: {
                                            readonly additionalProperties: false;
                                            readonly properties: {
                                                readonly algorithm: {
                                                    readonly enum: readonly ["sha256"];
                                                    readonly type: "string";
                                                };
                                                readonly value: {
                                                    readonly description: "Lowercase hex digest.";
                                                    readonly type: "string";
                                                };
                                            };
                                            readonly required: readonly ["algorithm", "value"];
                                            readonly type: "object";
                                        };
                                        readonly encryption: {
                                            readonly oneOf: readonly [{
                                                readonly type: "null";
                                            }, {
                                                readonly description: "Encryption metadata.";
                                                readonly type: "string";
                                            }];
                                        };
                                        readonly uri: {
                                            readonly description: "Content URI.";
                                            readonly type: "string";
                                        };
                                    };
                                    readonly required: readonly ["uri", "content_hash", "encryption"];
                                    readonly type: "object";
                                };
                                readonly kind: {
                                    readonly const: "external";
                                };
                            };
                            readonly required: readonly ["kind", "content_ref"];
                            readonly type: "object";
                        }];
                    };
                    readonly receipts: {
                        readonly items: {
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly dispatch_id: {
                                    readonly description: "Dispatch identifier.";
                                    readonly type: "string";
                                };
                                readonly dispatched_at: {
                                    readonly oneOf: readonly [{
                                        readonly additionalProperties: false;
                                        readonly properties: {
                                            readonly kind: {
                                                readonly const: "unix_millis";
                                            };
                                            readonly value: {
                                                readonly type: "integer";
                                            };
                                        };
                                        readonly required: readonly ["kind", "value"];
                                        readonly type: "object";
                                    }, {
                                        readonly additionalProperties: false;
                                        readonly properties: {
                                            readonly kind: {
                                                readonly const: "logical";
                                            };
                                            readonly value: {
                                                readonly minimum: 0;
                                                readonly type: "integer";
                                            };
                                        };
                                        readonly required: readonly ["kind", "value"];
                                        readonly type: "object";
                                    }];
                                };
                                readonly dispatcher: {
                                    readonly description: "Dispatcher identifier.";
                                    readonly type: "string";
                                };
                                readonly receipt_hash: {
                                    readonly additionalProperties: false;
                                    readonly properties: {
                                        readonly algorithm: {
                                            readonly enum: readonly ["sha256"];
                                            readonly type: "string";
                                        };
                                        readonly value: {
                                            readonly description: "Lowercase hex digest.";
                                            readonly type: "string";
                                        };
                                    };
                                    readonly required: readonly ["algorithm", "value"];
                                    readonly type: "object";
                                };
                                readonly target: {
                                    readonly oneOf: readonly [{
                                        readonly additionalProperties: false;
                                        readonly properties: {
                                            readonly agent_id: {
                                                readonly description: "Agent identifier.";
                                                readonly type: "string";
                                            };
                                            readonly kind: {
                                                readonly const: "agent";
                                            };
                                        };
                                        readonly required: readonly ["kind", "agent_id"];
                                        readonly type: "object";
                                    }, {
                                        readonly additionalProperties: false;
                                        readonly properties: {
                                            readonly kind: {
                                                readonly const: "session";
                                            };
                                            readonly session_id: {
                                                readonly description: "Session identifier.";
                                                readonly type: "string";
                                            };
                                        };
                                        readonly required: readonly ["kind", "session_id"];
                                        readonly type: "object";
                                    }, {
                                        readonly additionalProperties: false;
                                        readonly properties: {
                                            readonly kind: {
                                                readonly const: "external";
                                            };
                                            readonly system: {
                                                readonly description: "External system name.";
                                                readonly type: "string";
                                            };
                                            readonly target: {
                                                readonly description: "External system target.";
                                                readonly type: "string";
                                            };
                                        };
                                        readonly required: readonly ["kind", "system", "target"];
                                        readonly type: "object";
                                    }, {
                                        readonly additionalProperties: false;
                                        readonly properties: {
                                            readonly channel: {
                                                readonly description: "Broadcast channel identifier.";
                                                readonly type: "string";
                                            };
                                            readonly kind: {
                                                readonly const: "channel";
                                            };
                                        };
                                        readonly required: readonly ["kind", "channel"];
                                        readonly type: "object";
                                    }];
                                };
                            };
                            readonly required: readonly ["dispatch_id", "target", "receipt_hash", "dispatched_at", "dispatcher"];
                            readonly type: "object";
                        };
                        readonly type: "array";
                    };
                };
                readonly required: readonly ["envelope", "payload", "receipts", "decision_id"];
                readonly type: "object";
            };
            readonly type: "array";
        };
        readonly run_id: {
            readonly description: "Run identifier.";
            readonly type: "string";
        };
        readonly scenario_id: {
            readonly description: "Scenario identifier.";
            readonly type: "string";
        };
        readonly spec_hash: {
            readonly additionalProperties: false;
            readonly properties: {
                readonly algorithm: {
                    readonly enum: readonly ["sha256"];
                    readonly type: "string";
                };
                readonly value: {
                    readonly description: "Lowercase hex digest.";
                    readonly type: "string";
                };
            };
            readonly required: readonly ["algorithm", "value"];
            readonly type: "object";
        };
        readonly stage_entered_at: {
            readonly oneOf: readonly [{
                readonly additionalProperties: false;
                readonly properties: {
                    readonly kind: {
                        readonly const: "unix_millis";
                    };
                    readonly value: {
                        readonly type: "integer";
                    };
                };
                readonly required: readonly ["kind", "value"];
                readonly type: "object";
            }, {
                readonly additionalProperties: false;
                readonly properties: {
                    readonly kind: {
                        readonly const: "logical";
                    };
                    readonly value: {
                        readonly minimum: 0;
                        readonly type: "integer";
                    };
                };
                readonly required: readonly ["kind", "value"];
                readonly type: "object";
            }];
        };
        readonly status: {
            readonly enum: readonly ["active", "completed", "failed"];
            readonly type: "string";
        };
        readonly submissions: {
            readonly items: {
                readonly additionalProperties: false;
                readonly properties: {
                    readonly content_hash: {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly algorithm: {
                                readonly enum: readonly ["sha256"];
                                readonly type: "string";
                            };
                            readonly value: {
                                readonly description: "Lowercase hex digest.";
                                readonly type: "string";
                            };
                        };
                        readonly required: readonly ["algorithm", "value"];
                        readonly type: "object";
                    };
                    readonly content_type: {
                        readonly description: "Submission content type.";
                        readonly type: "string";
                    };
                    readonly correlation_id: {
                        readonly oneOf: readonly [{
                            readonly type: "null";
                        }, {
                            readonly description: "Correlation identifier.";
                            readonly type: "string";
                        }];
                    };
                    readonly payload: {
                        readonly oneOf: readonly [{
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly kind: {
                                    readonly const: "json";
                                };
                                readonly value: {
                                    readonly description: "Inline JSON payload.";
                                    readonly type: readonly ["null", "boolean", "number", "string", "array", "object"];
                                };
                            };
                            readonly required: readonly ["kind", "value"];
                            readonly type: "object";
                        }, {
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly bytes: {
                                    readonly items: {
                                        readonly maximum: 255;
                                        readonly minimum: 0;
                                        readonly type: "integer";
                                    };
                                    readonly type: "array";
                                };
                                readonly kind: {
                                    readonly const: "bytes";
                                };
                            };
                            readonly required: readonly ["kind", "bytes"];
                            readonly type: "object";
                        }, {
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly content_ref: {
                                    readonly additionalProperties: false;
                                    readonly properties: {
                                        readonly content_hash: {
                                            readonly additionalProperties: false;
                                            readonly properties: {
                                                readonly algorithm: {
                                                    readonly enum: readonly ["sha256"];
                                                    readonly type: "string";
                                                };
                                                readonly value: {
                                                    readonly description: "Lowercase hex digest.";
                                                    readonly type: "string";
                                                };
                                            };
                                            readonly required: readonly ["algorithm", "value"];
                                            readonly type: "object";
                                        };
                                        readonly encryption: {
                                            readonly oneOf: readonly [{
                                                readonly type: "null";
                                            }, {
                                                readonly description: "Encryption metadata.";
                                                readonly type: "string";
                                            }];
                                        };
                                        readonly uri: {
                                            readonly description: "Content URI.";
                                            readonly type: "string";
                                        };
                                    };
                                    readonly required: readonly ["uri", "content_hash", "encryption"];
                                    readonly type: "object";
                                };
                                readonly kind: {
                                    readonly const: "external";
                                };
                            };
                            readonly required: readonly ["kind", "content_ref"];
                            readonly type: "object";
                        }];
                    };
                    readonly run_id: {
                        readonly description: "Run identifier.";
                        readonly type: "string";
                    };
                    readonly submission_id: {
                        readonly description: "Submission identifier.";
                        readonly type: "string";
                    };
                    readonly submitted_at: {
                        readonly oneOf: readonly [{
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly kind: {
                                    readonly const: "unix_millis";
                                };
                                readonly value: {
                                    readonly type: "integer";
                                };
                            };
                            readonly required: readonly ["kind", "value"];
                            readonly type: "object";
                        }, {
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly kind: {
                                    readonly const: "logical";
                                };
                                readonly value: {
                                    readonly minimum: 0;
                                    readonly type: "integer";
                                };
                            };
                            readonly required: readonly ["kind", "value"];
                            readonly type: "object";
                        }];
                    };
                };
                readonly required: readonly ["submission_id", "run_id", "payload", "content_type", "content_hash", "submitted_at", "correlation_id"];
                readonly type: "object";
            };
            readonly type: "array";
        };
        readonly tenant_id: {
            readonly description: "Tenant identifier.";
            readonly minimum: 1;
            readonly type: "integer";
        };
        readonly tool_calls: {
            readonly items: {
                readonly additionalProperties: false;
                readonly properties: {
                    readonly call_id: {
                        readonly description: "Tool-call identifier.";
                        readonly type: "string";
                    };
                    readonly called_at: {
                        readonly oneOf: readonly [{
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly kind: {
                                    readonly const: "unix_millis";
                                };
                                readonly value: {
                                    readonly type: "integer";
                                };
                            };
                            readonly required: readonly ["kind", "value"];
                            readonly type: "object";
                        }, {
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly kind: {
                                    readonly const: "logical";
                                };
                                readonly value: {
                                    readonly minimum: 0;
                                    readonly type: "integer";
                                };
                            };
                            readonly required: readonly ["kind", "value"];
                            readonly type: "object";
                        }];
                    };
                    readonly correlation_id: {
                        readonly oneOf: readonly [{
                            readonly type: "null";
                        }, {
                            readonly description: "Correlation identifier.";
                            readonly type: "string";
                        }];
                    };
                    readonly error: {
                        readonly oneOf: readonly [{
                            readonly type: "null";
                        }, {
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly code: {
                                    readonly description: "Stable error code.";
                                    readonly type: "string";
                                };
                                readonly details: {
                                    readonly oneOf: readonly [{
                                        readonly type: "null";
                                    }, {
                                        readonly oneOf: readonly [{
                                            readonly additionalProperties: false;
                                            readonly properties: {
                                                readonly blocked_by_policy: {
                                                    readonly type: "boolean";
                                                };
                                                readonly kind: {
                                                    readonly const: "provider_missing";
                                                };
                                                readonly missing_providers: {
                                                    readonly description: "Missing provider identifiers.";
                                                    readonly items: {
                                                        readonly type: "string";
                                                    };
                                                    readonly type: "array";
                                                };
                                                readonly required_capabilities: {
                                                    readonly description: "Required capabilities.";
                                                    readonly items: {
                                                        readonly type: "string";
                                                    };
                                                    readonly type: "array";
                                                };
                                            };
                                            readonly required: readonly ["kind", "missing_providers", "required_capabilities", "blocked_by_policy"];
                                            readonly type: "object";
                                        }, {
                                            readonly additionalProperties: false;
                                            readonly properties: {
                                                readonly info: {
                                                    readonly description: "Additional error details.";
                                                    readonly type: "string";
                                                };
                                                readonly kind: {
                                                    readonly const: "message";
                                                };
                                            };
                                            readonly required: readonly ["kind", "info"];
                                            readonly type: "object";
                                        }];
                                    }];
                                };
                                readonly message: {
                                    readonly description: "Error message.";
                                    readonly type: "string";
                                };
                            };
                            readonly required: readonly ["code", "message", "details"];
                            readonly type: "object";
                        }];
                    };
                    readonly method: {
                        readonly description: "Tool method name.";
                        readonly type: "string";
                    };
                    readonly request_hash: {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly algorithm: {
                                readonly enum: readonly ["sha256"];
                                readonly type: "string";
                            };
                            readonly value: {
                                readonly description: "Lowercase hex digest.";
                                readonly type: "string";
                            };
                        };
                        readonly required: readonly ["algorithm", "value"];
                        readonly type: "object";
                    };
                    readonly response_hash: {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly algorithm: {
                                readonly enum: readonly ["sha256"];
                                readonly type: "string";
                            };
                            readonly value: {
                                readonly description: "Lowercase hex digest.";
                                readonly type: "string";
                            };
                        };
                        readonly required: readonly ["algorithm", "value"];
                        readonly type: "object";
                    };
                };
                readonly required: readonly ["call_id", "method", "request_hash", "response_hash", "called_at", "correlation_id", "error"];
                readonly type: "object";
            };
            readonly type: "array";
        };
        readonly triggers: {
            readonly items: {
                readonly additionalProperties: false;
                readonly properties: {
                    readonly event: {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly correlation_id: {
                                readonly oneOf: readonly [{
                                    readonly type: "null";
                                }, {
                                    readonly description: "Correlation identifier.";
                                    readonly type: "string";
                                }];
                            };
                            readonly kind: {
                                readonly enum: readonly ["agent_request_next", "tick", "external_event", "backend_event"];
                                readonly type: "string";
                            };
                            readonly namespace_id: {
                                readonly description: "Namespace identifier.";
                                readonly minimum: 1;
                                readonly type: "integer";
                            };
                            readonly payload: {
                                readonly oneOf: readonly [{
                                    readonly type: "null";
                                }, {
                                    readonly oneOf: readonly [{
                                        readonly additionalProperties: false;
                                        readonly properties: {
                                            readonly kind: {
                                                readonly const: "json";
                                            };
                                            readonly value: {
                                                readonly description: "Inline JSON payload.";
                                                readonly type: readonly ["null", "boolean", "number", "string", "array", "object"];
                                            };
                                        };
                                        readonly required: readonly ["kind", "value"];
                                        readonly type: "object";
                                    }, {
                                        readonly additionalProperties: false;
                                        readonly properties: {
                                            readonly bytes: {
                                                readonly items: {
                                                    readonly maximum: 255;
                                                    readonly minimum: 0;
                                                    readonly type: "integer";
                                                };
                                                readonly type: "array";
                                            };
                                            readonly kind: {
                                                readonly const: "bytes";
                                            };
                                        };
                                        readonly required: readonly ["kind", "bytes"];
                                        readonly type: "object";
                                    }, {
                                        readonly additionalProperties: false;
                                        readonly properties: {
                                            readonly content_ref: {
                                                readonly additionalProperties: false;
                                                readonly properties: {
                                                    readonly content_hash: {
                                                        readonly additionalProperties: false;
                                                        readonly properties: {
                                                            readonly algorithm: {
                                                                readonly enum: readonly ["sha256"];
                                                                readonly type: "string";
                                                            };
                                                            readonly value: {
                                                                readonly description: "Lowercase hex digest.";
                                                                readonly type: "string";
                                                            };
                                                        };
                                                        readonly required: readonly ["algorithm", "value"];
                                                        readonly type: "object";
                                                    };
                                                    readonly encryption: {
                                                        readonly oneOf: readonly [{
                                                            readonly type: "null";
                                                        }, {
                                                            readonly description: "Encryption metadata.";
                                                            readonly type: "string";
                                                        }];
                                                    };
                                                    readonly uri: {
                                                        readonly description: "Content URI.";
                                                        readonly type: "string";
                                                    };
                                                };
                                                readonly required: readonly ["uri", "content_hash", "encryption"];
                                                readonly type: "object";
                                            };
                                            readonly kind: {
                                                readonly const: "external";
                                            };
                                        };
                                        readonly required: readonly ["kind", "content_ref"];
                                        readonly type: "object";
                                    }];
                                }];
                            };
                            readonly run_id: {
                                readonly description: "Run identifier.";
                                readonly type: "string";
                            };
                            readonly source_id: {
                                readonly description: "Trigger source identifier.";
                                readonly type: "string";
                            };
                            readonly tenant_id: {
                                readonly description: "Tenant identifier.";
                                readonly minimum: 1;
                                readonly type: "integer";
                            };
                            readonly time: {
                                readonly oneOf: readonly [{
                                    readonly additionalProperties: false;
                                    readonly properties: {
                                        readonly kind: {
                                            readonly const: "unix_millis";
                                        };
                                        readonly value: {
                                            readonly type: "integer";
                                        };
                                    };
                                    readonly required: readonly ["kind", "value"];
                                    readonly type: "object";
                                }, {
                                    readonly additionalProperties: false;
                                    readonly properties: {
                                        readonly kind: {
                                            readonly const: "logical";
                                        };
                                        readonly value: {
                                            readonly minimum: 0;
                                            readonly type: "integer";
                                        };
                                    };
                                    readonly required: readonly ["kind", "value"];
                                    readonly type: "object";
                                }];
                            };
                            readonly trigger_id: {
                                readonly description: "Trigger identifier.";
                                readonly type: "string";
                            };
                        };
                        readonly required: readonly ["trigger_id", "tenant_id", "namespace_id", "run_id", "kind", "time", "source_id"];
                        readonly type: "object";
                    };
                    readonly seq: {
                        readonly minimum: 0;
                        readonly type: "integer";
                    };
                };
                readonly required: readonly ["seq", "event"];
                readonly type: "object";
            };
            readonly type: "array";
        };
    };
    readonly required: readonly ["tenant_id", "namespace_id", "run_id", "scenario_id", "spec_hash", "current_stage_id", "stage_entered_at", "status", "dispatch_targets", "triggers", "gate_evals", "decisions", "packets", "submissions", "tool_calls"];
    readonly type: "object";
};
export interface ScenarioStatusRequest {
    /** Status request payload. */
    request: Record<string, JsonValue>;
    /** Scenario identifier. */
    scenario_id: string;
}
export interface ScenarioStatusResponse {
    /** Current stage identifier. */
    current_stage_id: string;
    issued_packet_ids: Array<string>;
    last_decision: Record<string, JsonValue> | null;
    /** Namespace identifier. Constraints: Minimum: 1. */
    namespace_id?: number;
    /** Run identifier. */
    run_id: string;
    safe_summary: Record<string, JsonValue> | null;
    /** Scenario identifier. */
    scenario_id: string;
    /** Constraints: Allowed values: "active", "completed", "failed". */
    status: "active" | "completed" | "failed";
}
export declare const ScenarioStatus_INPUT_SCHEMA: {
    readonly $schema: "https://json-schema.org/draft/2020-12/schema";
    readonly additionalProperties: false;
    readonly properties: {
        readonly request: {
            readonly additionalProperties: false;
            readonly description: "Status request payload.";
            readonly properties: {
                readonly correlation_id: {
                    readonly oneOf: readonly [{
                        readonly type: "null";
                    }, {
                        readonly description: "Correlation identifier.";
                        readonly type: "string";
                    }];
                };
                readonly namespace_id: {
                    readonly description: "Namespace identifier.";
                    readonly minimum: 1;
                    readonly type: "integer";
                };
                readonly requested_at: {
                    readonly oneOf: readonly [{
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly kind: {
                                readonly const: "unix_millis";
                            };
                            readonly value: {
                                readonly type: "integer";
                            };
                        };
                        readonly required: readonly ["kind", "value"];
                        readonly type: "object";
                    }, {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly kind: {
                                readonly const: "logical";
                            };
                            readonly value: {
                                readonly minimum: 0;
                                readonly type: "integer";
                            };
                        };
                        readonly required: readonly ["kind", "value"];
                        readonly type: "object";
                    }];
                };
                readonly run_id: {
                    readonly description: "Run identifier.";
                    readonly type: "string";
                };
                readonly tenant_id: {
                    readonly description: "Tenant identifier.";
                    readonly minimum: 1;
                    readonly type: "integer";
                };
            };
            readonly required: readonly ["tenant_id", "namespace_id", "run_id", "requested_at"];
            readonly type: "object";
        };
        readonly scenario_id: {
            readonly description: "Scenario identifier.";
            readonly type: "string";
        };
    };
    readonly required: readonly ["scenario_id", "request"];
    readonly type: "object";
};
export declare const ScenarioStatus_OUTPUT_SCHEMA: {
    readonly additionalProperties: false;
    readonly properties: {
        readonly current_stage_id: {
            readonly description: "Current stage identifier.";
            readonly type: "string";
        };
        readonly issued_packet_ids: {
            readonly items: {
                readonly description: "Packet identifier.";
                readonly type: "string";
            };
            readonly type: "array";
        };
        readonly last_decision: {
            readonly oneOf: readonly [{
                readonly type: "null";
            }, {
                readonly additionalProperties: false;
                readonly properties: {
                    readonly correlation_id: {
                        readonly oneOf: readonly [{
                            readonly type: "null";
                        }, {
                            readonly description: "Correlation identifier.";
                            readonly type: "string";
                        }];
                    };
                    readonly decided_at: {
                        readonly oneOf: readonly [{
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly kind: {
                                    readonly const: "unix_millis";
                                };
                                readonly value: {
                                    readonly type: "integer";
                                };
                            };
                            readonly required: readonly ["kind", "value"];
                            readonly type: "object";
                        }, {
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly kind: {
                                    readonly const: "logical";
                                };
                                readonly value: {
                                    readonly minimum: 0;
                                    readonly type: "integer";
                                };
                            };
                            readonly required: readonly ["kind", "value"];
                            readonly type: "object";
                        }];
                    };
                    readonly decision_id: {
                        readonly description: "Decision identifier.";
                        readonly type: "string";
                    };
                    readonly outcome: {
                        readonly oneOf: readonly [{
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly kind: {
                                    readonly const: "start";
                                };
                                readonly stage_id: {
                                    readonly description: "Initial stage identifier.";
                                    readonly type: "string";
                                };
                            };
                            readonly required: readonly ["kind", "stage_id"];
                            readonly type: "object";
                        }, {
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly kind: {
                                    readonly const: "complete";
                                };
                                readonly stage_id: {
                                    readonly description: "Terminal stage identifier.";
                                    readonly type: "string";
                                };
                            };
                            readonly required: readonly ["kind", "stage_id"];
                            readonly type: "object";
                        }, {
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly from_stage: {
                                    readonly description: "Previous stage identifier.";
                                    readonly type: "string";
                                };
                                readonly kind: {
                                    readonly const: "advance";
                                };
                                readonly timeout: {
                                    readonly type: "boolean";
                                };
                                readonly to_stage: {
                                    readonly description: "Next stage identifier.";
                                    readonly type: "string";
                                };
                            };
                            readonly required: readonly ["kind", "from_stage", "to_stage", "timeout"];
                            readonly type: "object";
                        }, {
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly kind: {
                                    readonly const: "hold";
                                };
                                readonly summary: {
                                    readonly additionalProperties: false;
                                    readonly properties: {
                                        readonly policy_tags: {
                                            readonly description: "Policy tags applied to the summary.";
                                            readonly items: {
                                                readonly type: "string";
                                            };
                                            readonly type: "array";
                                        };
                                        readonly retry_hint: {
                                            readonly oneOf: readonly [{
                                                readonly type: "null";
                                            }, {
                                                readonly description: "Optional retry hint.";
                                                readonly type: "string";
                                            }];
                                        };
                                        readonly status: {
                                            readonly description: "Summary status.";
                                            readonly type: "string";
                                        };
                                        readonly unmet_gates: {
                                            readonly items: {
                                                readonly description: "Gate identifier.";
                                                readonly type: "string";
                                            };
                                            readonly type: "array";
                                        };
                                    };
                                    readonly required: readonly ["status", "unmet_gates", "retry_hint", "policy_tags"];
                                    readonly type: "object";
                                };
                            };
                            readonly required: readonly ["kind", "summary"];
                            readonly type: "object";
                        }, {
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly kind: {
                                    readonly const: "fail";
                                };
                                readonly reason: {
                                    readonly description: "Failure reason.";
                                    readonly type: "string";
                                };
                            };
                            readonly required: readonly ["kind", "reason"];
                            readonly type: "object";
                        }];
                    };
                    readonly seq: {
                        readonly minimum: 0;
                        readonly type: "integer";
                    };
                    readonly stage_id: {
                        readonly description: "Stage identifier.";
                        readonly type: "string";
                    };
                    readonly trigger_id: {
                        readonly description: "Trigger identifier.";
                        readonly type: "string";
                    };
                };
                readonly required: readonly ["decision_id", "seq", "trigger_id", "stage_id", "decided_at", "outcome", "correlation_id"];
                readonly type: "object";
            }];
        };
        readonly namespace_id: {
            readonly description: "Namespace identifier.";
            readonly minimum: 1;
            readonly type: "integer";
        };
        readonly run_id: {
            readonly description: "Run identifier.";
            readonly type: "string";
        };
        readonly safe_summary: {
            readonly oneOf: readonly [{
                readonly type: "null";
            }, {
                readonly additionalProperties: false;
                readonly properties: {
                    readonly policy_tags: {
                        readonly description: "Policy tags applied to the summary.";
                        readonly items: {
                            readonly type: "string";
                        };
                        readonly type: "array";
                    };
                    readonly retry_hint: {
                        readonly oneOf: readonly [{
                            readonly type: "null";
                        }, {
                            readonly description: "Optional retry hint.";
                            readonly type: "string";
                        }];
                    };
                    readonly status: {
                        readonly description: "Summary status.";
                        readonly type: "string";
                    };
                    readonly unmet_gates: {
                        readonly items: {
                            readonly description: "Gate identifier.";
                            readonly type: "string";
                        };
                        readonly type: "array";
                    };
                };
                readonly required: readonly ["status", "unmet_gates", "retry_hint", "policy_tags"];
                readonly type: "object";
            }];
        };
        readonly scenario_id: {
            readonly description: "Scenario identifier.";
            readonly type: "string";
        };
        readonly status: {
            readonly enum: readonly ["active", "completed", "failed"];
            readonly type: "string";
        };
    };
    readonly required: readonly ["run_id", "scenario_id", "current_stage_id", "status", "last_decision", "issued_packet_ids", "safe_summary"];
    readonly type: "object";
};
export interface ScenarioNextRequest {
    /** Next request payload from an agent. */
    request: Record<string, JsonValue>;
    /** Scenario identifier. */
    scenario_id: string;
}
export interface ScenarioNextResponse {
    decision: Record<string, JsonValue>;
    packets: Array<Record<string, JsonValue>>;
    /** Constraints: Allowed values: "active", "completed", "failed". */
    status: "active" | "completed" | "failed";
}
export declare const ScenarioNext_INPUT_SCHEMA: {
    readonly $schema: "https://json-schema.org/draft/2020-12/schema";
    readonly additionalProperties: false;
    readonly properties: {
        readonly request: {
            readonly additionalProperties: false;
            readonly description: "Next request payload from an agent.";
            readonly properties: {
                readonly agent_id: {
                    readonly description: "Agent identifier.";
                    readonly type: "string";
                };
                readonly correlation_id: {
                    readonly oneOf: readonly [{
                        readonly type: "null";
                    }, {
                        readonly description: "Correlation identifier.";
                        readonly type: "string";
                    }];
                };
                readonly namespace_id: {
                    readonly description: "Namespace identifier.";
                    readonly minimum: 1;
                    readonly type: "integer";
                };
                readonly run_id: {
                    readonly description: "Run identifier.";
                    readonly type: "string";
                };
                readonly tenant_id: {
                    readonly description: "Tenant identifier.";
                    readonly minimum: 1;
                    readonly type: "integer";
                };
                readonly time: {
                    readonly oneOf: readonly [{
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly kind: {
                                readonly const: "unix_millis";
                            };
                            readonly value: {
                                readonly type: "integer";
                            };
                        };
                        readonly required: readonly ["kind", "value"];
                        readonly type: "object";
                    }, {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly kind: {
                                readonly const: "logical";
                            };
                            readonly value: {
                                readonly minimum: 0;
                                readonly type: "integer";
                            };
                        };
                        readonly required: readonly ["kind", "value"];
                        readonly type: "object";
                    }];
                };
                readonly trigger_id: {
                    readonly description: "Trigger identifier.";
                    readonly type: "string";
                };
            };
            readonly required: readonly ["tenant_id", "namespace_id", "run_id", "trigger_id", "agent_id", "time"];
            readonly type: "object";
        };
        readonly scenario_id: {
            readonly description: "Scenario identifier.";
            readonly type: "string";
        };
    };
    readonly required: readonly ["scenario_id", "request"];
    readonly type: "object";
};
export declare const ScenarioNext_OUTPUT_SCHEMA: {
    readonly additionalProperties: false;
    readonly properties: {
        readonly decision: {
            readonly additionalProperties: false;
            readonly properties: {
                readonly correlation_id: {
                    readonly oneOf: readonly [{
                        readonly type: "null";
                    }, {
                        readonly description: "Correlation identifier.";
                        readonly type: "string";
                    }];
                };
                readonly decided_at: {
                    readonly oneOf: readonly [{
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly kind: {
                                readonly const: "unix_millis";
                            };
                            readonly value: {
                                readonly type: "integer";
                            };
                        };
                        readonly required: readonly ["kind", "value"];
                        readonly type: "object";
                    }, {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly kind: {
                                readonly const: "logical";
                            };
                            readonly value: {
                                readonly minimum: 0;
                                readonly type: "integer";
                            };
                        };
                        readonly required: readonly ["kind", "value"];
                        readonly type: "object";
                    }];
                };
                readonly decision_id: {
                    readonly description: "Decision identifier.";
                    readonly type: "string";
                };
                readonly outcome: {
                    readonly oneOf: readonly [{
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly kind: {
                                readonly const: "start";
                            };
                            readonly stage_id: {
                                readonly description: "Initial stage identifier.";
                                readonly type: "string";
                            };
                        };
                        readonly required: readonly ["kind", "stage_id"];
                        readonly type: "object";
                    }, {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly kind: {
                                readonly const: "complete";
                            };
                            readonly stage_id: {
                                readonly description: "Terminal stage identifier.";
                                readonly type: "string";
                            };
                        };
                        readonly required: readonly ["kind", "stage_id"];
                        readonly type: "object";
                    }, {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly from_stage: {
                                readonly description: "Previous stage identifier.";
                                readonly type: "string";
                            };
                            readonly kind: {
                                readonly const: "advance";
                            };
                            readonly timeout: {
                                readonly type: "boolean";
                            };
                            readonly to_stage: {
                                readonly description: "Next stage identifier.";
                                readonly type: "string";
                            };
                        };
                        readonly required: readonly ["kind", "from_stage", "to_stage", "timeout"];
                        readonly type: "object";
                    }, {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly kind: {
                                readonly const: "hold";
                            };
                            readonly summary: {
                                readonly additionalProperties: false;
                                readonly properties: {
                                    readonly policy_tags: {
                                        readonly description: "Policy tags applied to the summary.";
                                        readonly items: {
                                            readonly type: "string";
                                        };
                                        readonly type: "array";
                                    };
                                    readonly retry_hint: {
                                        readonly oneOf: readonly [{
                                            readonly type: "null";
                                        }, {
                                            readonly description: "Optional retry hint.";
                                            readonly type: "string";
                                        }];
                                    };
                                    readonly status: {
                                        readonly description: "Summary status.";
                                        readonly type: "string";
                                    };
                                    readonly unmet_gates: {
                                        readonly items: {
                                            readonly description: "Gate identifier.";
                                            readonly type: "string";
                                        };
                                        readonly type: "array";
                                    };
                                };
                                readonly required: readonly ["status", "unmet_gates", "retry_hint", "policy_tags"];
                                readonly type: "object";
                            };
                        };
                        readonly required: readonly ["kind", "summary"];
                        readonly type: "object";
                    }, {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly kind: {
                                readonly const: "fail";
                            };
                            readonly reason: {
                                readonly description: "Failure reason.";
                                readonly type: "string";
                            };
                        };
                        readonly required: readonly ["kind", "reason"];
                        readonly type: "object";
                    }];
                };
                readonly seq: {
                    readonly minimum: 0;
                    readonly type: "integer";
                };
                readonly stage_id: {
                    readonly description: "Stage identifier.";
                    readonly type: "string";
                };
                readonly trigger_id: {
                    readonly description: "Trigger identifier.";
                    readonly type: "string";
                };
            };
            readonly required: readonly ["decision_id", "seq", "trigger_id", "stage_id", "decided_at", "outcome", "correlation_id"];
            readonly type: "object";
        };
        readonly packets: {
            readonly items: {
                readonly additionalProperties: false;
                readonly properties: {
                    readonly decision_id: {
                        readonly description: "Decision identifier.";
                        readonly type: "string";
                    };
                    readonly envelope: {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly content_hash: {
                                readonly additionalProperties: false;
                                readonly properties: {
                                    readonly algorithm: {
                                        readonly enum: readonly ["sha256"];
                                        readonly type: "string";
                                    };
                                    readonly value: {
                                        readonly description: "Lowercase hex digest.";
                                        readonly type: "string";
                                    };
                                };
                                readonly required: readonly ["algorithm", "value"];
                                readonly type: "object";
                            };
                            readonly content_type: {
                                readonly description: "Packet content type.";
                                readonly type: "string";
                            };
                            readonly correlation_id: {
                                readonly oneOf: readonly [{
                                    readonly type: "null";
                                }, {
                                    readonly description: "Correlation identifier.";
                                    readonly type: "string";
                                }];
                            };
                            readonly expiry: {
                                readonly oneOf: readonly [{
                                    readonly type: "null";
                                }, {
                                    readonly oneOf: readonly [{
                                        readonly additionalProperties: false;
                                        readonly properties: {
                                            readonly kind: {
                                                readonly const: "unix_millis";
                                            };
                                            readonly value: {
                                                readonly type: "integer";
                                            };
                                        };
                                        readonly required: readonly ["kind", "value"];
                                        readonly type: "object";
                                    }, {
                                        readonly additionalProperties: false;
                                        readonly properties: {
                                            readonly kind: {
                                                readonly const: "logical";
                                            };
                                            readonly value: {
                                                readonly minimum: 0;
                                                readonly type: "integer";
                                            };
                                        };
                                        readonly required: readonly ["kind", "value"];
                                        readonly type: "object";
                                    }];
                                }];
                            };
                            readonly issued_at: {
                                readonly oneOf: readonly [{
                                    readonly additionalProperties: false;
                                    readonly properties: {
                                        readonly kind: {
                                            readonly const: "unix_millis";
                                        };
                                        readonly value: {
                                            readonly type: "integer";
                                        };
                                    };
                                    readonly required: readonly ["kind", "value"];
                                    readonly type: "object";
                                }, {
                                    readonly additionalProperties: false;
                                    readonly properties: {
                                        readonly kind: {
                                            readonly const: "logical";
                                        };
                                        readonly value: {
                                            readonly minimum: 0;
                                            readonly type: "integer";
                                        };
                                    };
                                    readonly required: readonly ["kind", "value"];
                                    readonly type: "object";
                                }];
                            };
                            readonly packet_id: {
                                readonly description: "Packet identifier.";
                                readonly type: "string";
                            };
                            readonly run_id: {
                                readonly description: "Run identifier.";
                                readonly type: "string";
                            };
                            readonly scenario_id: {
                                readonly description: "Scenario identifier.";
                                readonly type: "string";
                            };
                            readonly schema_id: {
                                readonly description: "Schema identifier.";
                                readonly type: "string";
                            };
                            readonly stage_id: {
                                readonly description: "Stage identifier.";
                                readonly type: "string";
                            };
                            readonly visibility: {
                                readonly additionalProperties: false;
                                readonly properties: {
                                    readonly labels: {
                                        readonly description: "Visibility labels.";
                                        readonly items: {
                                            readonly type: "string";
                                        };
                                        readonly type: "array";
                                    };
                                    readonly policy_tags: {
                                        readonly description: "Policy tags.";
                                        readonly items: {
                                            readonly type: "string";
                                        };
                                        readonly type: "array";
                                    };
                                };
                                readonly required: readonly ["labels", "policy_tags"];
                                readonly type: "object";
                            };
                        };
                        readonly required: readonly ["scenario_id", "run_id", "stage_id", "packet_id", "schema_id", "content_type", "content_hash", "visibility", "expiry", "correlation_id", "issued_at"];
                        readonly type: "object";
                    };
                    readonly payload: {
                        readonly oneOf: readonly [{
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly kind: {
                                    readonly const: "json";
                                };
                                readonly value: {
                                    readonly description: "Inline JSON payload.";
                                    readonly type: readonly ["null", "boolean", "number", "string", "array", "object"];
                                };
                            };
                            readonly required: readonly ["kind", "value"];
                            readonly type: "object";
                        }, {
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly bytes: {
                                    readonly items: {
                                        readonly maximum: 255;
                                        readonly minimum: 0;
                                        readonly type: "integer";
                                    };
                                    readonly type: "array";
                                };
                                readonly kind: {
                                    readonly const: "bytes";
                                };
                            };
                            readonly required: readonly ["kind", "bytes"];
                            readonly type: "object";
                        }, {
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly content_ref: {
                                    readonly additionalProperties: false;
                                    readonly properties: {
                                        readonly content_hash: {
                                            readonly additionalProperties: false;
                                            readonly properties: {
                                                readonly algorithm: {
                                                    readonly enum: readonly ["sha256"];
                                                    readonly type: "string";
                                                };
                                                readonly value: {
                                                    readonly description: "Lowercase hex digest.";
                                                    readonly type: "string";
                                                };
                                            };
                                            readonly required: readonly ["algorithm", "value"];
                                            readonly type: "object";
                                        };
                                        readonly encryption: {
                                            readonly oneOf: readonly [{
                                                readonly type: "null";
                                            }, {
                                                readonly description: "Encryption metadata.";
                                                readonly type: "string";
                                            }];
                                        };
                                        readonly uri: {
                                            readonly description: "Content URI.";
                                            readonly type: "string";
                                        };
                                    };
                                    readonly required: readonly ["uri", "content_hash", "encryption"];
                                    readonly type: "object";
                                };
                                readonly kind: {
                                    readonly const: "external";
                                };
                            };
                            readonly required: readonly ["kind", "content_ref"];
                            readonly type: "object";
                        }];
                    };
                    readonly receipts: {
                        readonly items: {
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly dispatch_id: {
                                    readonly description: "Dispatch identifier.";
                                    readonly type: "string";
                                };
                                readonly dispatched_at: {
                                    readonly oneOf: readonly [{
                                        readonly additionalProperties: false;
                                        readonly properties: {
                                            readonly kind: {
                                                readonly const: "unix_millis";
                                            };
                                            readonly value: {
                                                readonly type: "integer";
                                            };
                                        };
                                        readonly required: readonly ["kind", "value"];
                                        readonly type: "object";
                                    }, {
                                        readonly additionalProperties: false;
                                        readonly properties: {
                                            readonly kind: {
                                                readonly const: "logical";
                                            };
                                            readonly value: {
                                                readonly minimum: 0;
                                                readonly type: "integer";
                                            };
                                        };
                                        readonly required: readonly ["kind", "value"];
                                        readonly type: "object";
                                    }];
                                };
                                readonly dispatcher: {
                                    readonly description: "Dispatcher identifier.";
                                    readonly type: "string";
                                };
                                readonly receipt_hash: {
                                    readonly additionalProperties: false;
                                    readonly properties: {
                                        readonly algorithm: {
                                            readonly enum: readonly ["sha256"];
                                            readonly type: "string";
                                        };
                                        readonly value: {
                                            readonly description: "Lowercase hex digest.";
                                            readonly type: "string";
                                        };
                                    };
                                    readonly required: readonly ["algorithm", "value"];
                                    readonly type: "object";
                                };
                                readonly target: {
                                    readonly oneOf: readonly [{
                                        readonly additionalProperties: false;
                                        readonly properties: {
                                            readonly agent_id: {
                                                readonly description: "Agent identifier.";
                                                readonly type: "string";
                                            };
                                            readonly kind: {
                                                readonly const: "agent";
                                            };
                                        };
                                        readonly required: readonly ["kind", "agent_id"];
                                        readonly type: "object";
                                    }, {
                                        readonly additionalProperties: false;
                                        readonly properties: {
                                            readonly kind: {
                                                readonly const: "session";
                                            };
                                            readonly session_id: {
                                                readonly description: "Session identifier.";
                                                readonly type: "string";
                                            };
                                        };
                                        readonly required: readonly ["kind", "session_id"];
                                        readonly type: "object";
                                    }, {
                                        readonly additionalProperties: false;
                                        readonly properties: {
                                            readonly kind: {
                                                readonly const: "external";
                                            };
                                            readonly system: {
                                                readonly description: "External system name.";
                                                readonly type: "string";
                                            };
                                            readonly target: {
                                                readonly description: "External system target.";
                                                readonly type: "string";
                                            };
                                        };
                                        readonly required: readonly ["kind", "system", "target"];
                                        readonly type: "object";
                                    }, {
                                        readonly additionalProperties: false;
                                        readonly properties: {
                                            readonly channel: {
                                                readonly description: "Broadcast channel identifier.";
                                                readonly type: "string";
                                            };
                                            readonly kind: {
                                                readonly const: "channel";
                                            };
                                        };
                                        readonly required: readonly ["kind", "channel"];
                                        readonly type: "object";
                                    }];
                                };
                            };
                            readonly required: readonly ["dispatch_id", "target", "receipt_hash", "dispatched_at", "dispatcher"];
                            readonly type: "object";
                        };
                        readonly type: "array";
                    };
                };
                readonly required: readonly ["envelope", "payload", "receipts", "decision_id"];
                readonly type: "object";
            };
            readonly type: "array";
        };
        readonly status: {
            readonly enum: readonly ["active", "completed", "failed"];
            readonly type: "string";
        };
    };
    readonly required: readonly ["decision", "packets", "status"];
    readonly type: "object";
};
export interface ScenarioSubmitRequest {
    /** Submission payload and metadata. */
    request: Record<string, JsonValue>;
    /** Scenario identifier. */
    scenario_id: string;
}
export interface ScenarioSubmitResponse {
    record: Record<string, JsonValue>;
}
export declare const ScenarioSubmit_INPUT_SCHEMA: {
    readonly $schema: "https://json-schema.org/draft/2020-12/schema";
    readonly additionalProperties: false;
    readonly properties: {
        readonly request: {
            readonly additionalProperties: false;
            readonly description: "Submission payload and metadata.";
            readonly properties: {
                readonly content_type: {
                    readonly description: "Submission content type.";
                    readonly type: "string";
                };
                readonly correlation_id: {
                    readonly oneOf: readonly [{
                        readonly type: "null";
                    }, {
                        readonly description: "Correlation identifier.";
                        readonly type: "string";
                    }];
                };
                readonly namespace_id: {
                    readonly description: "Namespace identifier.";
                    readonly minimum: 1;
                    readonly type: "integer";
                };
                readonly payload: {
                    readonly oneOf: readonly [{
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly kind: {
                                readonly const: "json";
                            };
                            readonly value: {
                                readonly description: "Inline JSON payload.";
                                readonly type: readonly ["null", "boolean", "number", "string", "array", "object"];
                            };
                        };
                        readonly required: readonly ["kind", "value"];
                        readonly type: "object";
                    }, {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly bytes: {
                                readonly items: {
                                    readonly maximum: 255;
                                    readonly minimum: 0;
                                    readonly type: "integer";
                                };
                                readonly type: "array";
                            };
                            readonly kind: {
                                readonly const: "bytes";
                            };
                        };
                        readonly required: readonly ["kind", "bytes"];
                        readonly type: "object";
                    }, {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly content_ref: {
                                readonly additionalProperties: false;
                                readonly properties: {
                                    readonly content_hash: {
                                        readonly additionalProperties: false;
                                        readonly properties: {
                                            readonly algorithm: {
                                                readonly enum: readonly ["sha256"];
                                                readonly type: "string";
                                            };
                                            readonly value: {
                                                readonly description: "Lowercase hex digest.";
                                                readonly type: "string";
                                            };
                                        };
                                        readonly required: readonly ["algorithm", "value"];
                                        readonly type: "object";
                                    };
                                    readonly encryption: {
                                        readonly oneOf: readonly [{
                                            readonly type: "null";
                                        }, {
                                            readonly description: "Encryption metadata.";
                                            readonly type: "string";
                                        }];
                                    };
                                    readonly uri: {
                                        readonly description: "Content URI.";
                                        readonly type: "string";
                                    };
                                };
                                readonly required: readonly ["uri", "content_hash", "encryption"];
                                readonly type: "object";
                            };
                            readonly kind: {
                                readonly const: "external";
                            };
                        };
                        readonly required: readonly ["kind", "content_ref"];
                        readonly type: "object";
                    }];
                };
                readonly run_id: {
                    readonly description: "Run identifier.";
                    readonly type: "string";
                };
                readonly submission_id: {
                    readonly description: "Submission identifier.";
                    readonly type: "string";
                };
                readonly submitted_at: {
                    readonly oneOf: readonly [{
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly kind: {
                                readonly const: "unix_millis";
                            };
                            readonly value: {
                                readonly type: "integer";
                            };
                        };
                        readonly required: readonly ["kind", "value"];
                        readonly type: "object";
                    }, {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly kind: {
                                readonly const: "logical";
                            };
                            readonly value: {
                                readonly minimum: 0;
                                readonly type: "integer";
                            };
                        };
                        readonly required: readonly ["kind", "value"];
                        readonly type: "object";
                    }];
                };
                readonly tenant_id: {
                    readonly description: "Tenant identifier.";
                    readonly minimum: 1;
                    readonly type: "integer";
                };
            };
            readonly required: readonly ["tenant_id", "namespace_id", "run_id", "submission_id", "payload", "content_type", "submitted_at"];
            readonly type: "object";
        };
        readonly scenario_id: {
            readonly description: "Scenario identifier.";
            readonly type: "string";
        };
    };
    readonly required: readonly ["scenario_id", "request"];
    readonly type: "object";
};
export declare const ScenarioSubmit_OUTPUT_SCHEMA: {
    readonly additionalProperties: false;
    readonly properties: {
        readonly record: {
            readonly additionalProperties: false;
            readonly properties: {
                readonly content_hash: {
                    readonly additionalProperties: false;
                    readonly properties: {
                        readonly algorithm: {
                            readonly enum: readonly ["sha256"];
                            readonly type: "string";
                        };
                        readonly value: {
                            readonly description: "Lowercase hex digest.";
                            readonly type: "string";
                        };
                    };
                    readonly required: readonly ["algorithm", "value"];
                    readonly type: "object";
                };
                readonly content_type: {
                    readonly description: "Submission content type.";
                    readonly type: "string";
                };
                readonly correlation_id: {
                    readonly oneOf: readonly [{
                        readonly type: "null";
                    }, {
                        readonly description: "Correlation identifier.";
                        readonly type: "string";
                    }];
                };
                readonly payload: {
                    readonly oneOf: readonly [{
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly kind: {
                                readonly const: "json";
                            };
                            readonly value: {
                                readonly description: "Inline JSON payload.";
                                readonly type: readonly ["null", "boolean", "number", "string", "array", "object"];
                            };
                        };
                        readonly required: readonly ["kind", "value"];
                        readonly type: "object";
                    }, {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly bytes: {
                                readonly items: {
                                    readonly maximum: 255;
                                    readonly minimum: 0;
                                    readonly type: "integer";
                                };
                                readonly type: "array";
                            };
                            readonly kind: {
                                readonly const: "bytes";
                            };
                        };
                        readonly required: readonly ["kind", "bytes"];
                        readonly type: "object";
                    }, {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly content_ref: {
                                readonly additionalProperties: false;
                                readonly properties: {
                                    readonly content_hash: {
                                        readonly additionalProperties: false;
                                        readonly properties: {
                                            readonly algorithm: {
                                                readonly enum: readonly ["sha256"];
                                                readonly type: "string";
                                            };
                                            readonly value: {
                                                readonly description: "Lowercase hex digest.";
                                                readonly type: "string";
                                            };
                                        };
                                        readonly required: readonly ["algorithm", "value"];
                                        readonly type: "object";
                                    };
                                    readonly encryption: {
                                        readonly oneOf: readonly [{
                                            readonly type: "null";
                                        }, {
                                            readonly description: "Encryption metadata.";
                                            readonly type: "string";
                                        }];
                                    };
                                    readonly uri: {
                                        readonly description: "Content URI.";
                                        readonly type: "string";
                                    };
                                };
                                readonly required: readonly ["uri", "content_hash", "encryption"];
                                readonly type: "object";
                            };
                            readonly kind: {
                                readonly const: "external";
                            };
                        };
                        readonly required: readonly ["kind", "content_ref"];
                        readonly type: "object";
                    }];
                };
                readonly run_id: {
                    readonly description: "Run identifier.";
                    readonly type: "string";
                };
                readonly submission_id: {
                    readonly description: "Submission identifier.";
                    readonly type: "string";
                };
                readonly submitted_at: {
                    readonly oneOf: readonly [{
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly kind: {
                                readonly const: "unix_millis";
                            };
                            readonly value: {
                                readonly type: "integer";
                            };
                        };
                        readonly required: readonly ["kind", "value"];
                        readonly type: "object";
                    }, {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly kind: {
                                readonly const: "logical";
                            };
                            readonly value: {
                                readonly minimum: 0;
                                readonly type: "integer";
                            };
                        };
                        readonly required: readonly ["kind", "value"];
                        readonly type: "object";
                    }];
                };
            };
            readonly required: readonly ["submission_id", "run_id", "payload", "content_type", "content_hash", "submitted_at", "correlation_id"];
            readonly type: "object";
        };
    };
    readonly required: readonly ["record"];
    readonly type: "object";
};
export interface ScenarioTriggerRequest {
    /** Scenario identifier. */
    scenario_id: string;
    /** Trigger event payload. */
    trigger: Record<string, JsonValue>;
}
export interface ScenarioTriggerResponse {
    decision: Record<string, JsonValue>;
    packets: Array<Record<string, JsonValue>>;
    /** Constraints: Allowed values: "active", "completed", "failed". */
    status: "active" | "completed" | "failed";
}
export declare const ScenarioTrigger_INPUT_SCHEMA: {
    readonly $schema: "https://json-schema.org/draft/2020-12/schema";
    readonly additionalProperties: false;
    readonly properties: {
        readonly scenario_id: {
            readonly description: "Scenario identifier.";
            readonly type: "string";
        };
        readonly trigger: {
            readonly additionalProperties: false;
            readonly description: "Trigger event payload.";
            readonly properties: {
                readonly correlation_id: {
                    readonly oneOf: readonly [{
                        readonly type: "null";
                    }, {
                        readonly description: "Correlation identifier.";
                        readonly type: "string";
                    }];
                };
                readonly kind: {
                    readonly enum: readonly ["agent_request_next", "tick", "external_event", "backend_event"];
                    readonly type: "string";
                };
                readonly namespace_id: {
                    readonly description: "Namespace identifier.";
                    readonly minimum: 1;
                    readonly type: "integer";
                };
                readonly payload: {
                    readonly oneOf: readonly [{
                        readonly type: "null";
                    }, {
                        readonly oneOf: readonly [{
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly kind: {
                                    readonly const: "json";
                                };
                                readonly value: {
                                    readonly description: "Inline JSON payload.";
                                    readonly type: readonly ["null", "boolean", "number", "string", "array", "object"];
                                };
                            };
                            readonly required: readonly ["kind", "value"];
                            readonly type: "object";
                        }, {
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly bytes: {
                                    readonly items: {
                                        readonly maximum: 255;
                                        readonly minimum: 0;
                                        readonly type: "integer";
                                    };
                                    readonly type: "array";
                                };
                                readonly kind: {
                                    readonly const: "bytes";
                                };
                            };
                            readonly required: readonly ["kind", "bytes"];
                            readonly type: "object";
                        }, {
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly content_ref: {
                                    readonly additionalProperties: false;
                                    readonly properties: {
                                        readonly content_hash: {
                                            readonly additionalProperties: false;
                                            readonly properties: {
                                                readonly algorithm: {
                                                    readonly enum: readonly ["sha256"];
                                                    readonly type: "string";
                                                };
                                                readonly value: {
                                                    readonly description: "Lowercase hex digest.";
                                                    readonly type: "string";
                                                };
                                            };
                                            readonly required: readonly ["algorithm", "value"];
                                            readonly type: "object";
                                        };
                                        readonly encryption: {
                                            readonly oneOf: readonly [{
                                                readonly type: "null";
                                            }, {
                                                readonly description: "Encryption metadata.";
                                                readonly type: "string";
                                            }];
                                        };
                                        readonly uri: {
                                            readonly description: "Content URI.";
                                            readonly type: "string";
                                        };
                                    };
                                    readonly required: readonly ["uri", "content_hash", "encryption"];
                                    readonly type: "object";
                                };
                                readonly kind: {
                                    readonly const: "external";
                                };
                            };
                            readonly required: readonly ["kind", "content_ref"];
                            readonly type: "object";
                        }];
                    }];
                };
                readonly run_id: {
                    readonly description: "Run identifier.";
                    readonly type: "string";
                };
                readonly source_id: {
                    readonly description: "Trigger source identifier.";
                    readonly type: "string";
                };
                readonly tenant_id: {
                    readonly description: "Tenant identifier.";
                    readonly minimum: 1;
                    readonly type: "integer";
                };
                readonly time: {
                    readonly oneOf: readonly [{
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly kind: {
                                readonly const: "unix_millis";
                            };
                            readonly value: {
                                readonly type: "integer";
                            };
                        };
                        readonly required: readonly ["kind", "value"];
                        readonly type: "object";
                    }, {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly kind: {
                                readonly const: "logical";
                            };
                            readonly value: {
                                readonly minimum: 0;
                                readonly type: "integer";
                            };
                        };
                        readonly required: readonly ["kind", "value"];
                        readonly type: "object";
                    }];
                };
                readonly trigger_id: {
                    readonly description: "Trigger identifier.";
                    readonly type: "string";
                };
            };
            readonly required: readonly ["trigger_id", "tenant_id", "namespace_id", "run_id", "kind", "time", "source_id"];
            readonly type: "object";
        };
    };
    readonly required: readonly ["scenario_id", "trigger"];
    readonly type: "object";
};
export declare const ScenarioTrigger_OUTPUT_SCHEMA: {
    readonly additionalProperties: false;
    readonly properties: {
        readonly decision: {
            readonly additionalProperties: false;
            readonly properties: {
                readonly correlation_id: {
                    readonly oneOf: readonly [{
                        readonly type: "null";
                    }, {
                        readonly description: "Correlation identifier.";
                        readonly type: "string";
                    }];
                };
                readonly decided_at: {
                    readonly oneOf: readonly [{
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly kind: {
                                readonly const: "unix_millis";
                            };
                            readonly value: {
                                readonly type: "integer";
                            };
                        };
                        readonly required: readonly ["kind", "value"];
                        readonly type: "object";
                    }, {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly kind: {
                                readonly const: "logical";
                            };
                            readonly value: {
                                readonly minimum: 0;
                                readonly type: "integer";
                            };
                        };
                        readonly required: readonly ["kind", "value"];
                        readonly type: "object";
                    }];
                };
                readonly decision_id: {
                    readonly description: "Decision identifier.";
                    readonly type: "string";
                };
                readonly outcome: {
                    readonly oneOf: readonly [{
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly kind: {
                                readonly const: "start";
                            };
                            readonly stage_id: {
                                readonly description: "Initial stage identifier.";
                                readonly type: "string";
                            };
                        };
                        readonly required: readonly ["kind", "stage_id"];
                        readonly type: "object";
                    }, {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly kind: {
                                readonly const: "complete";
                            };
                            readonly stage_id: {
                                readonly description: "Terminal stage identifier.";
                                readonly type: "string";
                            };
                        };
                        readonly required: readonly ["kind", "stage_id"];
                        readonly type: "object";
                    }, {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly from_stage: {
                                readonly description: "Previous stage identifier.";
                                readonly type: "string";
                            };
                            readonly kind: {
                                readonly const: "advance";
                            };
                            readonly timeout: {
                                readonly type: "boolean";
                            };
                            readonly to_stage: {
                                readonly description: "Next stage identifier.";
                                readonly type: "string";
                            };
                        };
                        readonly required: readonly ["kind", "from_stage", "to_stage", "timeout"];
                        readonly type: "object";
                    }, {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly kind: {
                                readonly const: "hold";
                            };
                            readonly summary: {
                                readonly additionalProperties: false;
                                readonly properties: {
                                    readonly policy_tags: {
                                        readonly description: "Policy tags applied to the summary.";
                                        readonly items: {
                                            readonly type: "string";
                                        };
                                        readonly type: "array";
                                    };
                                    readonly retry_hint: {
                                        readonly oneOf: readonly [{
                                            readonly type: "null";
                                        }, {
                                            readonly description: "Optional retry hint.";
                                            readonly type: "string";
                                        }];
                                    };
                                    readonly status: {
                                        readonly description: "Summary status.";
                                        readonly type: "string";
                                    };
                                    readonly unmet_gates: {
                                        readonly items: {
                                            readonly description: "Gate identifier.";
                                            readonly type: "string";
                                        };
                                        readonly type: "array";
                                    };
                                };
                                readonly required: readonly ["status", "unmet_gates", "retry_hint", "policy_tags"];
                                readonly type: "object";
                            };
                        };
                        readonly required: readonly ["kind", "summary"];
                        readonly type: "object";
                    }, {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly kind: {
                                readonly const: "fail";
                            };
                            readonly reason: {
                                readonly description: "Failure reason.";
                                readonly type: "string";
                            };
                        };
                        readonly required: readonly ["kind", "reason"];
                        readonly type: "object";
                    }];
                };
                readonly seq: {
                    readonly minimum: 0;
                    readonly type: "integer";
                };
                readonly stage_id: {
                    readonly description: "Stage identifier.";
                    readonly type: "string";
                };
                readonly trigger_id: {
                    readonly description: "Trigger identifier.";
                    readonly type: "string";
                };
            };
            readonly required: readonly ["decision_id", "seq", "trigger_id", "stage_id", "decided_at", "outcome", "correlation_id"];
            readonly type: "object";
        };
        readonly packets: {
            readonly items: {
                readonly additionalProperties: false;
                readonly properties: {
                    readonly decision_id: {
                        readonly description: "Decision identifier.";
                        readonly type: "string";
                    };
                    readonly envelope: {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly content_hash: {
                                readonly additionalProperties: false;
                                readonly properties: {
                                    readonly algorithm: {
                                        readonly enum: readonly ["sha256"];
                                        readonly type: "string";
                                    };
                                    readonly value: {
                                        readonly description: "Lowercase hex digest.";
                                        readonly type: "string";
                                    };
                                };
                                readonly required: readonly ["algorithm", "value"];
                                readonly type: "object";
                            };
                            readonly content_type: {
                                readonly description: "Packet content type.";
                                readonly type: "string";
                            };
                            readonly correlation_id: {
                                readonly oneOf: readonly [{
                                    readonly type: "null";
                                }, {
                                    readonly description: "Correlation identifier.";
                                    readonly type: "string";
                                }];
                            };
                            readonly expiry: {
                                readonly oneOf: readonly [{
                                    readonly type: "null";
                                }, {
                                    readonly oneOf: readonly [{
                                        readonly additionalProperties: false;
                                        readonly properties: {
                                            readonly kind: {
                                                readonly const: "unix_millis";
                                            };
                                            readonly value: {
                                                readonly type: "integer";
                                            };
                                        };
                                        readonly required: readonly ["kind", "value"];
                                        readonly type: "object";
                                    }, {
                                        readonly additionalProperties: false;
                                        readonly properties: {
                                            readonly kind: {
                                                readonly const: "logical";
                                            };
                                            readonly value: {
                                                readonly minimum: 0;
                                                readonly type: "integer";
                                            };
                                        };
                                        readonly required: readonly ["kind", "value"];
                                        readonly type: "object";
                                    }];
                                }];
                            };
                            readonly issued_at: {
                                readonly oneOf: readonly [{
                                    readonly additionalProperties: false;
                                    readonly properties: {
                                        readonly kind: {
                                            readonly const: "unix_millis";
                                        };
                                        readonly value: {
                                            readonly type: "integer";
                                        };
                                    };
                                    readonly required: readonly ["kind", "value"];
                                    readonly type: "object";
                                }, {
                                    readonly additionalProperties: false;
                                    readonly properties: {
                                        readonly kind: {
                                            readonly const: "logical";
                                        };
                                        readonly value: {
                                            readonly minimum: 0;
                                            readonly type: "integer";
                                        };
                                    };
                                    readonly required: readonly ["kind", "value"];
                                    readonly type: "object";
                                }];
                            };
                            readonly packet_id: {
                                readonly description: "Packet identifier.";
                                readonly type: "string";
                            };
                            readonly run_id: {
                                readonly description: "Run identifier.";
                                readonly type: "string";
                            };
                            readonly scenario_id: {
                                readonly description: "Scenario identifier.";
                                readonly type: "string";
                            };
                            readonly schema_id: {
                                readonly description: "Schema identifier.";
                                readonly type: "string";
                            };
                            readonly stage_id: {
                                readonly description: "Stage identifier.";
                                readonly type: "string";
                            };
                            readonly visibility: {
                                readonly additionalProperties: false;
                                readonly properties: {
                                    readonly labels: {
                                        readonly description: "Visibility labels.";
                                        readonly items: {
                                            readonly type: "string";
                                        };
                                        readonly type: "array";
                                    };
                                    readonly policy_tags: {
                                        readonly description: "Policy tags.";
                                        readonly items: {
                                            readonly type: "string";
                                        };
                                        readonly type: "array";
                                    };
                                };
                                readonly required: readonly ["labels", "policy_tags"];
                                readonly type: "object";
                            };
                        };
                        readonly required: readonly ["scenario_id", "run_id", "stage_id", "packet_id", "schema_id", "content_type", "content_hash", "visibility", "expiry", "correlation_id", "issued_at"];
                        readonly type: "object";
                    };
                    readonly payload: {
                        readonly oneOf: readonly [{
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly kind: {
                                    readonly const: "json";
                                };
                                readonly value: {
                                    readonly description: "Inline JSON payload.";
                                    readonly type: readonly ["null", "boolean", "number", "string", "array", "object"];
                                };
                            };
                            readonly required: readonly ["kind", "value"];
                            readonly type: "object";
                        }, {
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly bytes: {
                                    readonly items: {
                                        readonly maximum: 255;
                                        readonly minimum: 0;
                                        readonly type: "integer";
                                    };
                                    readonly type: "array";
                                };
                                readonly kind: {
                                    readonly const: "bytes";
                                };
                            };
                            readonly required: readonly ["kind", "bytes"];
                            readonly type: "object";
                        }, {
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly content_ref: {
                                    readonly additionalProperties: false;
                                    readonly properties: {
                                        readonly content_hash: {
                                            readonly additionalProperties: false;
                                            readonly properties: {
                                                readonly algorithm: {
                                                    readonly enum: readonly ["sha256"];
                                                    readonly type: "string";
                                                };
                                                readonly value: {
                                                    readonly description: "Lowercase hex digest.";
                                                    readonly type: "string";
                                                };
                                            };
                                            readonly required: readonly ["algorithm", "value"];
                                            readonly type: "object";
                                        };
                                        readonly encryption: {
                                            readonly oneOf: readonly [{
                                                readonly type: "null";
                                            }, {
                                                readonly description: "Encryption metadata.";
                                                readonly type: "string";
                                            }];
                                        };
                                        readonly uri: {
                                            readonly description: "Content URI.";
                                            readonly type: "string";
                                        };
                                    };
                                    readonly required: readonly ["uri", "content_hash", "encryption"];
                                    readonly type: "object";
                                };
                                readonly kind: {
                                    readonly const: "external";
                                };
                            };
                            readonly required: readonly ["kind", "content_ref"];
                            readonly type: "object";
                        }];
                    };
                    readonly receipts: {
                        readonly items: {
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly dispatch_id: {
                                    readonly description: "Dispatch identifier.";
                                    readonly type: "string";
                                };
                                readonly dispatched_at: {
                                    readonly oneOf: readonly [{
                                        readonly additionalProperties: false;
                                        readonly properties: {
                                            readonly kind: {
                                                readonly const: "unix_millis";
                                            };
                                            readonly value: {
                                                readonly type: "integer";
                                            };
                                        };
                                        readonly required: readonly ["kind", "value"];
                                        readonly type: "object";
                                    }, {
                                        readonly additionalProperties: false;
                                        readonly properties: {
                                            readonly kind: {
                                                readonly const: "logical";
                                            };
                                            readonly value: {
                                                readonly minimum: 0;
                                                readonly type: "integer";
                                            };
                                        };
                                        readonly required: readonly ["kind", "value"];
                                        readonly type: "object";
                                    }];
                                };
                                readonly dispatcher: {
                                    readonly description: "Dispatcher identifier.";
                                    readonly type: "string";
                                };
                                readonly receipt_hash: {
                                    readonly additionalProperties: false;
                                    readonly properties: {
                                        readonly algorithm: {
                                            readonly enum: readonly ["sha256"];
                                            readonly type: "string";
                                        };
                                        readonly value: {
                                            readonly description: "Lowercase hex digest.";
                                            readonly type: "string";
                                        };
                                    };
                                    readonly required: readonly ["algorithm", "value"];
                                    readonly type: "object";
                                };
                                readonly target: {
                                    readonly oneOf: readonly [{
                                        readonly additionalProperties: false;
                                        readonly properties: {
                                            readonly agent_id: {
                                                readonly description: "Agent identifier.";
                                                readonly type: "string";
                                            };
                                            readonly kind: {
                                                readonly const: "agent";
                                            };
                                        };
                                        readonly required: readonly ["kind", "agent_id"];
                                        readonly type: "object";
                                    }, {
                                        readonly additionalProperties: false;
                                        readonly properties: {
                                            readonly kind: {
                                                readonly const: "session";
                                            };
                                            readonly session_id: {
                                                readonly description: "Session identifier.";
                                                readonly type: "string";
                                            };
                                        };
                                        readonly required: readonly ["kind", "session_id"];
                                        readonly type: "object";
                                    }, {
                                        readonly additionalProperties: false;
                                        readonly properties: {
                                            readonly kind: {
                                                readonly const: "external";
                                            };
                                            readonly system: {
                                                readonly description: "External system name.";
                                                readonly type: "string";
                                            };
                                            readonly target: {
                                                readonly description: "External system target.";
                                                readonly type: "string";
                                            };
                                        };
                                        readonly required: readonly ["kind", "system", "target"];
                                        readonly type: "object";
                                    }, {
                                        readonly additionalProperties: false;
                                        readonly properties: {
                                            readonly channel: {
                                                readonly description: "Broadcast channel identifier.";
                                                readonly type: "string";
                                            };
                                            readonly kind: {
                                                readonly const: "channel";
                                            };
                                        };
                                        readonly required: readonly ["kind", "channel"];
                                        readonly type: "object";
                                    }];
                                };
                            };
                            readonly required: readonly ["dispatch_id", "target", "receipt_hash", "dispatched_at", "dispatcher"];
                            readonly type: "object";
                        };
                        readonly type: "array";
                    };
                };
                readonly required: readonly ["envelope", "payload", "receipts", "decision_id"];
                readonly type: "object";
            };
            readonly type: "array";
        };
        readonly status: {
            readonly enum: readonly ["active", "completed", "failed"];
            readonly type: "string";
        };
    };
    readonly required: readonly ["decision", "packets", "status"];
    readonly type: "object";
};
export interface EvidenceQueryRequest {
    /** Evidence context used for evaluation. */
    context: Record<string, JsonValue>;
    /** Evidence query payload. */
    query: Record<string, JsonValue>;
}
export interface EvidenceQueryResponse {
    result: Record<string, JsonValue>;
}
export declare const EvidenceQuery_INPUT_SCHEMA: {
    readonly $schema: "https://json-schema.org/draft/2020-12/schema";
    readonly additionalProperties: false;
    readonly properties: {
        readonly context: {
            readonly additionalProperties: false;
            readonly description: "Evidence context used for evaluation.";
            readonly properties: {
                readonly correlation_id: {
                    readonly oneOf: readonly [{
                        readonly type: "null";
                    }, {
                        readonly description: "Correlation identifier.";
                        readonly type: "string";
                    }];
                };
                readonly namespace_id: {
                    readonly description: "Namespace identifier.";
                    readonly minimum: 1;
                    readonly type: "integer";
                };
                readonly run_id: {
                    readonly description: "Run identifier.";
                    readonly type: "string";
                };
                readonly scenario_id: {
                    readonly description: "Scenario identifier.";
                    readonly type: "string";
                };
                readonly stage_id: {
                    readonly description: "Stage identifier.";
                    readonly type: "string";
                };
                readonly tenant_id: {
                    readonly description: "Tenant identifier.";
                    readonly minimum: 1;
                    readonly type: "integer";
                };
                readonly trigger_id: {
                    readonly description: "Trigger identifier.";
                    readonly type: "string";
                };
                readonly trigger_time: {
                    readonly oneOf: readonly [{
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly kind: {
                                readonly const: "unix_millis";
                            };
                            readonly value: {
                                readonly type: "integer";
                            };
                        };
                        readonly required: readonly ["kind", "value"];
                        readonly type: "object";
                    }, {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly kind: {
                                readonly const: "logical";
                            };
                            readonly value: {
                                readonly minimum: 0;
                                readonly type: "integer";
                            };
                        };
                        readonly required: readonly ["kind", "value"];
                        readonly type: "object";
                    }];
                };
            };
            readonly required: readonly ["tenant_id", "run_id", "scenario_id", "stage_id", "trigger_id", "trigger_time"];
            readonly type: "object";
        };
        readonly query: {
            readonly additionalProperties: false;
            readonly description: "Evidence query payload.";
            readonly properties: {
                readonly params: {
                    readonly description: "Provider-specific parameter payload.";
                    readonly type: readonly ["null", "boolean", "number", "string", "array", "object"];
                };
                readonly predicate: {
                    readonly description: "Provider predicate name.";
                    readonly type: "string";
                };
                readonly provider_id: {
                    readonly description: "Evidence provider identifier.";
                    readonly type: "string";
                };
            };
            readonly required: readonly ["provider_id", "predicate"];
            readonly type: "object";
        };
    };
    readonly required: readonly ["query", "context"];
    readonly type: "object";
};
export declare const EvidenceQuery_OUTPUT_SCHEMA: {
    readonly $schema: "https://json-schema.org/draft/2020-12/schema";
    readonly additionalProperties: false;
    readonly properties: {
        readonly result: {
            readonly additionalProperties: false;
            readonly properties: {
                readonly content_type: {
                    readonly oneOf: readonly [{
                        readonly type: "null";
                    }, {
                        readonly description: "Evidence content type.";
                        readonly type: "string";
                    }];
                };
                readonly error: {
                    readonly oneOf: readonly [{
                        readonly type: "null";
                    }, {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly code: {
                                readonly description: "Stable error code.";
                                readonly type: "string";
                            };
                            readonly details: {
                                readonly oneOf: readonly [{
                                    readonly type: "null";
                                }, {
                                    readonly description: "Optional structured error details.";
                                    readonly type: readonly ["null", "boolean", "number", "string", "array", "object"];
                                }];
                            };
                            readonly message: {
                                readonly description: "Error message.";
                                readonly type: "string";
                            };
                        };
                        readonly required: readonly ["code", "message", "details"];
                        readonly type: "object";
                    }];
                };
                readonly evidence_anchor: {
                    readonly oneOf: readonly [{
                        readonly type: "null";
                    }, {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly anchor_type: {
                                readonly description: "Anchor type identifier.";
                                readonly type: "string";
                            };
                            readonly anchor_value: {
                                readonly description: "Anchor value.";
                                readonly type: "string";
                            };
                        };
                        readonly required: readonly ["anchor_type", "anchor_value"];
                        readonly type: "object";
                    }];
                };
                readonly evidence_hash: {
                    readonly oneOf: readonly [{
                        readonly type: "null";
                    }, {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly algorithm: {
                                readonly enum: readonly ["sha256"];
                                readonly type: "string";
                            };
                            readonly value: {
                                readonly description: "Lowercase hex digest.";
                                readonly type: "string";
                            };
                        };
                        readonly required: readonly ["algorithm", "value"];
                        readonly type: "object";
                    }];
                };
                readonly evidence_ref: {
                    readonly oneOf: readonly [{
                        readonly type: "null";
                    }, {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly uri: {
                                readonly description: "Evidence reference URI.";
                                readonly type: "string";
                            };
                        };
                        readonly required: readonly ["uri"];
                        readonly type: "object";
                    }];
                };
                readonly lane: {
                    readonly description: "Trust lane classification for evidence.";
                    readonly enum: readonly ["verified", "asserted"];
                    readonly type: "string";
                };
                readonly signature: {
                    readonly oneOf: readonly [{
                        readonly type: "null";
                    }, {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly key_id: {
                                readonly description: "Signing key identifier.";
                                readonly type: "string";
                            };
                            readonly scheme: {
                                readonly description: "Signature scheme identifier.";
                                readonly type: "string";
                            };
                            readonly signature: {
                                readonly items: {
                                    readonly maximum: 255;
                                    readonly minimum: 0;
                                    readonly type: "integer";
                                };
                                readonly type: "array";
                            };
                        };
                        readonly required: readonly ["scheme", "key_id", "signature"];
                        readonly type: "object";
                    }];
                };
                readonly value: {
                    readonly oneOf: readonly [{
                        readonly type: "null";
                    }, {
                        readonly oneOf: readonly [{
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly kind: {
                                    readonly const: "json";
                                };
                                readonly value: {
                                    readonly description: "Evidence JSON value.";
                                    readonly type: readonly ["null", "boolean", "number", "string", "array", "object"];
                                };
                            };
                            readonly required: readonly ["kind", "value"];
                            readonly type: "object";
                        }, {
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly kind: {
                                    readonly const: "bytes";
                                };
                                readonly value: {
                                    readonly items: {
                                        readonly maximum: 255;
                                        readonly minimum: 0;
                                        readonly type: "integer";
                                    };
                                    readonly type: "array";
                                };
                            };
                            readonly required: readonly ["kind", "value"];
                            readonly type: "object";
                        }];
                    }];
                };
            };
            readonly required: readonly ["value", "lane", "error", "evidence_hash", "evidence_ref", "evidence_anchor", "signature", "content_type"];
            readonly type: "object";
        };
    };
    readonly required: readonly ["result"];
    readonly type: "object";
};
export interface RunpackExportRequest {
    /** Timestamp recorded in the manifest. */
    generated_at: Record<string, JsonValue>;
    /** Generate a verification report artifact. */
    include_verification: boolean;
    /** Optional override for the manifest file name. */
    manifest_name?: null | string;
    /** Namespace identifier. Constraints: Minimum: 1. */
    namespace_id: number;
    /** Optional output directory (required for filesystem export). */
    output_dir?: null | string;
    /** Run identifier. */
    run_id: string;
    /** Scenario identifier. */
    scenario_id: string;
    /** Tenant identifier. Constraints: Minimum: 1. */
    tenant_id: number;
}
export interface RunpackExportResponse {
    manifest: Record<string, JsonValue>;
    report: Record<string, JsonValue> | null;
    /** Optional storage URI for managed runpack storage backends. */
    storage_uri?: null | string;
}
export declare const RunpackExport_INPUT_SCHEMA: {
    readonly $schema: "https://json-schema.org/draft/2020-12/schema";
    readonly additionalProperties: false;
    readonly properties: {
        readonly generated_at: {
            readonly description: "Timestamp recorded in the manifest.";
            readonly oneOf: readonly [{
                readonly additionalProperties: false;
                readonly properties: {
                    readonly kind: {
                        readonly const: "unix_millis";
                    };
                    readonly value: {
                        readonly type: "integer";
                    };
                };
                readonly required: readonly ["kind", "value"];
                readonly type: "object";
            }, {
                readonly additionalProperties: false;
                readonly properties: {
                    readonly kind: {
                        readonly const: "logical";
                    };
                    readonly value: {
                        readonly minimum: 0;
                        readonly type: "integer";
                    };
                };
                readonly required: readonly ["kind", "value"];
                readonly type: "object";
            }];
        };
        readonly include_verification: {
            readonly description: "Generate a verification report artifact.";
            readonly type: "boolean";
        };
        readonly manifest_name: {
            readonly description: "Optional override for the manifest file name.";
            readonly oneOf: readonly [{
                readonly type: "null";
            }, {
                readonly description: "Manifest file name.";
                readonly type: "string";
            }];
        };
        readonly namespace_id: {
            readonly description: "Namespace identifier.";
            readonly minimum: 1;
            readonly type: "integer";
        };
        readonly output_dir: {
            readonly description: "Optional output directory (required for filesystem export).";
            readonly oneOf: readonly [{
                readonly type: "null";
            }, {
                readonly description: "Output directory path.";
                readonly type: "string";
            }];
        };
        readonly run_id: {
            readonly description: "Run identifier.";
            readonly type: "string";
        };
        readonly scenario_id: {
            readonly description: "Scenario identifier.";
            readonly type: "string";
        };
        readonly tenant_id: {
            readonly description: "Tenant identifier.";
            readonly minimum: 1;
            readonly type: "integer";
        };
    };
    readonly required: readonly ["scenario_id", "tenant_id", "namespace_id", "run_id", "generated_at", "include_verification"];
    readonly type: "object";
};
export declare const RunpackExport_OUTPUT_SCHEMA: {
    readonly $schema: "https://json-schema.org/draft/2020-12/schema";
    readonly additionalProperties: false;
    readonly properties: {
        readonly manifest: {
            readonly additionalProperties: false;
            readonly properties: {
                readonly artifacts: {
                    readonly items: {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly artifact_id: {
                                readonly description: "Artifact identifier.";
                                readonly type: "string";
                            };
                            readonly content_type: {
                                readonly oneOf: readonly [{
                                    readonly type: "null";
                                }, {
                                    readonly description: "Artifact content type.";
                                    readonly type: "string";
                                }];
                            };
                            readonly hash: {
                                readonly additionalProperties: false;
                                readonly properties: {
                                    readonly algorithm: {
                                        readonly enum: readonly ["sha256"];
                                        readonly type: "string";
                                    };
                                    readonly value: {
                                        readonly description: "Lowercase hex digest.";
                                        readonly type: "string";
                                    };
                                };
                                readonly required: readonly ["algorithm", "value"];
                                readonly type: "object";
                            };
                            readonly kind: {
                                readonly enum: readonly ["scenario_spec", "trigger_log", "gate_eval_log", "decision_log", "packet_log", "dispatch_log", "evidence_log", "submission_log", "tool_transcript", "verifier_report", "custom"];
                                readonly type: "string";
                            };
                            readonly path: {
                                readonly description: "Runpack-relative artifact path.";
                                readonly type: "string";
                            };
                            readonly required: {
                                readonly type: "boolean";
                            };
                        };
                        readonly required: readonly ["artifact_id", "kind", "path", "content_type", "hash", "required"];
                        readonly type: "object";
                    };
                    readonly type: "array";
                };
                readonly generated_at: {
                    readonly oneOf: readonly [{
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly kind: {
                                readonly const: "unix_millis";
                            };
                            readonly value: {
                                readonly type: "integer";
                            };
                        };
                        readonly required: readonly ["kind", "value"];
                        readonly type: "object";
                    }, {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly kind: {
                                readonly const: "logical";
                            };
                            readonly value: {
                                readonly minimum: 0;
                                readonly type: "integer";
                            };
                        };
                        readonly required: readonly ["kind", "value"];
                        readonly type: "object";
                    }];
                };
                readonly hash_algorithm: {
                    readonly enum: readonly ["sha256"];
                    readonly type: "string";
                };
                readonly integrity: {
                    readonly additionalProperties: false;
                    readonly properties: {
                        readonly file_hashes: {
                            readonly items: {
                                readonly additionalProperties: false;
                                readonly properties: {
                                    readonly hash: {
                                        readonly additionalProperties: false;
                                        readonly properties: {
                                            readonly algorithm: {
                                                readonly enum: readonly ["sha256"];
                                                readonly type: "string";
                                            };
                                            readonly value: {
                                                readonly description: "Lowercase hex digest.";
                                                readonly type: "string";
                                            };
                                        };
                                        readonly required: readonly ["algorithm", "value"];
                                        readonly type: "object";
                                    };
                                    readonly path: {
                                        readonly description: "Runpack-relative artifact path.";
                                        readonly type: "string";
                                    };
                                };
                                readonly required: readonly ["path", "hash"];
                                readonly type: "object";
                            };
                            readonly type: "array";
                        };
                        readonly root_hash: {
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly algorithm: {
                                    readonly enum: readonly ["sha256"];
                                    readonly type: "string";
                                };
                                readonly value: {
                                    readonly description: "Lowercase hex digest.";
                                    readonly type: "string";
                                };
                            };
                            readonly required: readonly ["algorithm", "value"];
                            readonly type: "object";
                        };
                    };
                    readonly required: readonly ["file_hashes", "root_hash"];
                    readonly type: "object";
                };
                readonly manifest_version: {
                    readonly description: "Runpack manifest version.";
                    readonly type: "string";
                };
                readonly namespace_id: {
                    readonly description: "Namespace identifier.";
                    readonly minimum: 1;
                    readonly type: "integer";
                };
                readonly run_id: {
                    readonly description: "Run identifier.";
                    readonly type: "string";
                };
                readonly scenario_id: {
                    readonly description: "Scenario identifier.";
                    readonly type: "string";
                };
                readonly security: {
                    readonly additionalProperties: false;
                    readonly properties: {
                        readonly dev_permissive: {
                            readonly type: "boolean";
                        };
                        readonly namespace_authority: {
                            readonly description: "Namespace authority mode label.";
                            readonly type: "string";
                        };
                    };
                    readonly required: readonly ["dev_permissive", "namespace_authority"];
                    readonly type: "object";
                };
                readonly spec_hash: {
                    readonly additionalProperties: false;
                    readonly properties: {
                        readonly algorithm: {
                            readonly enum: readonly ["sha256"];
                            readonly type: "string";
                        };
                        readonly value: {
                            readonly description: "Lowercase hex digest.";
                            readonly type: "string";
                        };
                    };
                    readonly required: readonly ["algorithm", "value"];
                    readonly type: "object";
                };
                readonly tenant_id: {
                    readonly description: "Tenant identifier.";
                    readonly minimum: 1;
                    readonly type: "integer";
                };
                readonly verifier_mode: {
                    readonly enum: readonly ["offline_strict", "offline_with_fetch"];
                    readonly type: "string";
                };
            };
            readonly required: readonly ["manifest_version", "generated_at", "tenant_id", "namespace_id", "scenario_id", "run_id", "spec_hash", "hash_algorithm", "verifier_mode", "integrity", "artifacts"];
            readonly type: "object";
        };
        readonly report: {
            readonly oneOf: readonly [{
                readonly type: "null";
            }, {
                readonly additionalProperties: false;
                readonly properties: {
                    readonly checked_files: {
                        readonly minimum: 0;
                        readonly type: "integer";
                    };
                    readonly errors: {
                        readonly description: "Verification error messages.";
                        readonly items: {
                            readonly type: "string";
                        };
                        readonly type: "array";
                    };
                    readonly status: {
                        readonly description: "Runpack verification status.";
                        readonly enum: readonly ["pass", "fail"];
                        readonly type: "string";
                    };
                };
                readonly required: readonly ["status", "checked_files", "errors"];
                readonly type: "object";
            }];
        };
        readonly storage_uri: {
            readonly description: "Optional storage URI for managed runpack storage backends.";
            readonly oneOf: readonly [{
                readonly type: "null";
            }, {
                readonly type: "string";
            }];
        };
    };
    readonly required: readonly ["manifest", "report"];
    readonly type: "object";
};
export interface RunpackVerifyRequest {
    /** Manifest path relative to runpack root. */
    manifest_path: string;
    /** Runpack root directory. */
    runpack_dir: string;
}
export interface RunpackVerifyResponse {
    report: Record<string, JsonValue>;
    /** Runpack verification status. Constraints: Allowed values: "pass", "fail". */
    status: "pass" | "fail";
}
export declare const RunpackVerify_INPUT_SCHEMA: {
    readonly $schema: "https://json-schema.org/draft/2020-12/schema";
    readonly additionalProperties: false;
    readonly properties: {
        readonly manifest_path: {
            readonly description: "Manifest path relative to runpack root.";
            readonly type: "string";
        };
        readonly runpack_dir: {
            readonly description: "Runpack root directory.";
            readonly type: "string";
        };
    };
    readonly required: readonly ["runpack_dir", "manifest_path"];
    readonly type: "object";
};
export declare const RunpackVerify_OUTPUT_SCHEMA: {
    readonly $schema: "https://json-schema.org/draft/2020-12/schema";
    readonly additionalProperties: false;
    readonly properties: {
        readonly report: {
            readonly additionalProperties: false;
            readonly properties: {
                readonly checked_files: {
                    readonly minimum: 0;
                    readonly type: "integer";
                };
                readonly errors: {
                    readonly description: "Verification error messages.";
                    readonly items: {
                        readonly type: "string";
                    };
                    readonly type: "array";
                };
                readonly status: {
                    readonly description: "Runpack verification status.";
                    readonly enum: readonly ["pass", "fail"];
                    readonly type: "string";
                };
            };
            readonly required: readonly ["status", "checked_files", "errors"];
            readonly type: "object";
        };
        readonly status: {
            readonly description: "Runpack verification status.";
            readonly enum: readonly ["pass", "fail"];
            readonly type: "string";
        };
    };
    readonly required: readonly ["report", "status"];
    readonly type: "object";
};
export interface ProvidersListRequest {
    [key: string]: never;
}
export interface ProvidersListResponse {
    providers: Array<Record<string, JsonValue>>;
}
export declare const ProvidersList_INPUT_SCHEMA: {
    readonly $schema: "https://json-schema.org/draft/2020-12/schema";
    readonly additionalProperties: false;
    readonly properties: {};
    readonly required: readonly [];
    readonly type: "object";
};
export declare const ProvidersList_OUTPUT_SCHEMA: {
    readonly $schema: "https://json-schema.org/draft/2020-12/schema";
    readonly additionalProperties: false;
    readonly properties: {
        readonly providers: {
            readonly items: {
                readonly additionalProperties: false;
                readonly properties: {
                    readonly predicates: {
                        readonly items: {
                            readonly description: "Predicate identifier.";
                            readonly type: "string";
                        };
                        readonly type: "array";
                    };
                    readonly provider_id: {
                        readonly description: "Provider identifier.";
                        readonly type: "string";
                    };
                    readonly transport: {
                        readonly description: "Provider transport type.";
                        readonly enum: readonly ["builtin", "mcp"];
                        readonly type: "string";
                    };
                };
                readonly required: readonly ["provider_id", "transport", "predicates"];
                readonly type: "object";
            };
            readonly type: "array";
        };
    };
    readonly required: readonly ["providers"];
    readonly type: "object";
};
export interface ProviderContractGetRequest {
    /** Provider identifier. */
    provider_id: string;
}
export interface ProviderContractGetResponse {
    contract: Record<string, JsonValue>;
    contract_hash: Record<string, JsonValue>;
    /** Provider identifier. */
    provider_id: string;
    /** Contract source origin. Constraints: Allowed values: "builtin", "file". */
    source: "builtin" | "file";
    /** Optional contract version label. */
    version: null | string;
}
export declare const ProviderContractGet_INPUT_SCHEMA: {
    readonly $schema: "https://json-schema.org/draft/2020-12/schema";
    readonly additionalProperties: false;
    readonly properties: {
        readonly provider_id: {
            readonly description: "Provider identifier.";
            readonly type: "string";
        };
    };
    readonly required: readonly ["provider_id"];
    readonly type: "object";
};
export declare const ProviderContractGet_OUTPUT_SCHEMA: {
    readonly $schema: "https://json-schema.org/draft/2020-12/schema";
    readonly additionalProperties: false;
    readonly properties: {
        readonly contract: {
            readonly additionalProperties: false;
            readonly properties: {
                readonly config_schema: {
                    readonly description: "Provider configuration schema.";
                    readonly type: readonly ["null", "boolean", "number", "string", "array", "object"];
                };
                readonly description: {
                    readonly description: "Provider description.";
                    readonly type: "string";
                };
                readonly name: {
                    readonly description: "Provider display name.";
                    readonly type: "string";
                };
                readonly notes: {
                    readonly description: "Provider notes and guidance.";
                    readonly items: {
                        readonly type: "string";
                    };
                    readonly type: "array";
                };
                readonly predicates: {
                    readonly items: {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly allowed_comparators: {
                                readonly description: "Comparator allow-list for this predicate.";
                                readonly items: {
                                    readonly description: "Comparator applied to evidence values.";
                                    readonly enum: readonly ["equals", "not_equals", "greater_than", "greater_than_or_equal", "less_than", "less_than_or_equal", "lex_greater_than", "lex_greater_than_or_equal", "lex_less_than", "lex_less_than_or_equal", "contains", "in_set", "deep_equals", "deep_not_equals", "exists", "not_exists"];
                                    readonly type: "string";
                                };
                                readonly type: "array";
                            };
                            readonly anchor_types: {
                                readonly description: "Anchor types emitted by this predicate.";
                                readonly items: {
                                    readonly type: "string";
                                };
                                readonly type: "array";
                            };
                            readonly content_types: {
                                readonly description: "Content types for predicate output.";
                                readonly items: {
                                    readonly type: "string";
                                };
                                readonly type: "array";
                            };
                            readonly description: {
                                readonly description: "Predicate description.";
                                readonly type: "string";
                            };
                            readonly determinism: {
                                readonly description: "Determinism classification for provider predicates.";
                                readonly enum: readonly ["deterministic", "time_dependent", "external"];
                                readonly type: "string";
                            };
                            readonly examples: {
                                readonly items: {
                                    readonly additionalProperties: false;
                                    readonly properties: {
                                        readonly description: {
                                            readonly description: "Short example description.";
                                            readonly type: "string";
                                        };
                                        readonly params: {
                                            readonly description: "Example params payload.";
                                            readonly type: readonly ["null", "boolean", "number", "string", "array", "object"];
                                        };
                                        readonly result: {
                                            readonly description: "Example result value.";
                                            readonly type: readonly ["null", "boolean", "number", "string", "array", "object"];
                                        };
                                    };
                                    readonly required: readonly ["description", "params", "result"];
                                    readonly type: "object";
                                };
                                readonly type: "array";
                            };
                            readonly name: {
                                readonly description: "Predicate name.";
                                readonly type: "string";
                            };
                            readonly params_required: {
                                readonly description: "Whether params are required for this predicate.";
                                readonly type: "boolean";
                            };
                            readonly params_schema: {
                                readonly description: "JSON schema for predicate params.";
                                readonly type: readonly ["null", "boolean", "number", "string", "array", "object"];
                            };
                            readonly result_schema: {
                                readonly description: "JSON schema for predicate result values.";
                                readonly type: readonly ["null", "boolean", "number", "string", "array", "object"];
                            };
                        };
                        readonly required: readonly ["name", "description", "determinism", "params_required", "params_schema", "result_schema", "allowed_comparators", "anchor_types", "content_types", "examples"];
                        readonly type: "object";
                    };
                    readonly type: "array";
                };
                readonly provider_id: {
                    readonly description: "Provider identifier.";
                    readonly type: "string";
                };
                readonly transport: {
                    readonly description: "Provider transport kind.";
                    readonly enum: readonly ["builtin", "mcp"];
                    readonly type: "string";
                };
            };
            readonly required: readonly ["provider_id", "name", "description", "transport", "config_schema", "predicates", "notes"];
            readonly type: "object";
        };
        readonly contract_hash: {
            readonly additionalProperties: false;
            readonly properties: {
                readonly algorithm: {
                    readonly enum: readonly ["sha256"];
                    readonly type: "string";
                };
                readonly value: {
                    readonly description: "Lowercase hex digest.";
                    readonly type: "string";
                };
            };
            readonly required: readonly ["algorithm", "value"];
            readonly type: "object";
        };
        readonly provider_id: {
            readonly description: "Provider identifier.";
            readonly type: "string";
        };
        readonly source: {
            readonly description: "Contract source origin.";
            readonly enum: readonly ["builtin", "file"];
            readonly type: "string";
        };
        readonly version: {
            readonly description: "Optional contract version label.";
            readonly oneOf: readonly [{
                readonly type: "null";
            }, {
                readonly type: "string";
            }];
        };
    };
    readonly required: readonly ["provider_id", "contract", "contract_hash", "source", "version"];
    readonly type: "object";
};
export interface ProviderSchemaGetRequest {
    /** Provider predicate name. */
    predicate: string;
    /** Provider identifier. */
    provider_id: string;
}
export interface ProviderSchemaGetResponse {
    /** Comparator allow-list for this predicate. */
    allowed_comparators: Array<"equals" | "not_equals" | "greater_than" | "greater_than_or_equal" | "less_than" | "less_than_or_equal" | "lex_greater_than" | "lex_greater_than_or_equal" | "lex_less_than" | "lex_less_than_or_equal" | "contains" | "in_set" | "deep_equals" | "deep_not_equals" | "exists" | "not_exists">;
    /** Anchor types emitted by this predicate. */
    anchor_types: Array<string>;
    /** Content types for predicate output. */
    content_types: Array<string>;
    contract_hash: Record<string, JsonValue>;
    /** Determinism classification for provider predicates. Constraints: Allowed values: */
    /** "deterministic", "time_dependent", "external". */
    determinism: "deterministic" | "time_dependent" | "external";
    examples: Array<Record<string, JsonValue>>;
    /** Whether params are required for this predicate. */
    params_required: boolean;
    /** JSON schema for predicate params. */
    params_schema: Array<JsonValue> | Record<string, JsonValue> | boolean | null | number | string;
    /** Predicate name. */
    predicate: string;
    /** Provider identifier. */
    provider_id: string;
    /** JSON schema for predicate result value. */
    result_schema: Array<JsonValue> | Record<string, JsonValue> | boolean | null | number | string;
}
export declare const ProviderSchemaGet_INPUT_SCHEMA: {
    readonly $schema: "https://json-schema.org/draft/2020-12/schema";
    readonly additionalProperties: false;
    readonly properties: {
        readonly predicate: {
            readonly description: "Provider predicate name.";
            readonly type: "string";
        };
        readonly provider_id: {
            readonly description: "Provider identifier.";
            readonly type: "string";
        };
    };
    readonly required: readonly ["provider_id", "predicate"];
    readonly type: "object";
};
export declare const ProviderSchemaGet_OUTPUT_SCHEMA: {
    readonly $schema: "https://json-schema.org/draft/2020-12/schema";
    readonly additionalProperties: false;
    readonly properties: {
        readonly allowed_comparators: {
            readonly description: "Comparator allow-list for this predicate.";
            readonly items: {
                readonly description: "Comparator applied to evidence values.";
                readonly enum: readonly ["equals", "not_equals", "greater_than", "greater_than_or_equal", "less_than", "less_than_or_equal", "lex_greater_than", "lex_greater_than_or_equal", "lex_less_than", "lex_less_than_or_equal", "contains", "in_set", "deep_equals", "deep_not_equals", "exists", "not_exists"];
                readonly type: "string";
            };
            readonly type: "array";
        };
        readonly anchor_types: {
            readonly description: "Anchor types emitted by this predicate.";
            readonly items: {
                readonly type: "string";
            };
            readonly type: "array";
        };
        readonly content_types: {
            readonly description: "Content types for predicate output.";
            readonly items: {
                readonly type: "string";
            };
            readonly type: "array";
        };
        readonly contract_hash: {
            readonly additionalProperties: false;
            readonly properties: {
                readonly algorithm: {
                    readonly enum: readonly ["sha256"];
                    readonly type: "string";
                };
                readonly value: {
                    readonly description: "Lowercase hex digest.";
                    readonly type: "string";
                };
            };
            readonly required: readonly ["algorithm", "value"];
            readonly type: "object";
        };
        readonly determinism: {
            readonly description: "Determinism classification for provider predicates.";
            readonly enum: readonly ["deterministic", "time_dependent", "external"];
            readonly type: "string";
        };
        readonly examples: {
            readonly items: {
                readonly additionalProperties: false;
                readonly properties: {
                    readonly description: {
                        readonly description: "Short example description.";
                        readonly type: "string";
                    };
                    readonly params: {
                        readonly description: "Example params payload.";
                        readonly type: readonly ["null", "boolean", "number", "string", "array", "object"];
                    };
                    readonly result: {
                        readonly description: "Example result value.";
                        readonly type: readonly ["null", "boolean", "number", "string", "array", "object"];
                    };
                };
                readonly required: readonly ["description", "params", "result"];
                readonly type: "object";
            };
            readonly type: "array";
        };
        readonly params_required: {
            readonly description: "Whether params are required for this predicate.";
            readonly type: "boolean";
        };
        readonly params_schema: {
            readonly description: "JSON schema for predicate params.";
            readonly type: readonly ["null", "boolean", "number", "string", "array", "object"];
        };
        readonly predicate: {
            readonly description: "Predicate name.";
            readonly type: "string";
        };
        readonly provider_id: {
            readonly description: "Provider identifier.";
            readonly type: "string";
        };
        readonly result_schema: {
            readonly description: "JSON schema for predicate result value.";
            readonly type: readonly ["null", "boolean", "number", "string", "array", "object"];
        };
    };
    readonly required: readonly ["provider_id", "predicate", "params_required", "params_schema", "result_schema", "allowed_comparators", "determinism", "anchor_types", "content_types", "examples", "contract_hash"];
    readonly type: "object";
};
export interface SchemasRegisterRequest {
    record: Record<string, JsonValue>;
}
export interface SchemasRegisterResponse {
    record: Record<string, JsonValue>;
}
export declare const SchemasRegister_INPUT_SCHEMA: {
    readonly $schema: "https://json-schema.org/draft/2020-12/schema";
    readonly additionalProperties: false;
    readonly properties: {
        readonly record: {
            readonly additionalProperties: false;
            readonly properties: {
                readonly created_at: {
                    readonly oneOf: readonly [{
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly kind: {
                                readonly const: "unix_millis";
                            };
                            readonly value: {
                                readonly type: "integer";
                            };
                        };
                        readonly required: readonly ["kind", "value"];
                        readonly type: "object";
                    }, {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly kind: {
                                readonly const: "logical";
                            };
                            readonly value: {
                                readonly minimum: 0;
                                readonly type: "integer";
                            };
                        };
                        readonly required: readonly ["kind", "value"];
                        readonly type: "object";
                    }];
                };
                readonly description: {
                    readonly oneOf: readonly [{
                        readonly type: "null";
                    }, {
                        readonly description: "Optional schema description.";
                        readonly type: "string";
                    }];
                };
                readonly namespace_id: {
                    readonly description: "Namespace identifier.";
                    readonly minimum: 1;
                    readonly type: "integer";
                };
                readonly schema: {
                    readonly description: "JSON Schema payload.";
                    readonly type: readonly ["null", "boolean", "number", "string", "array", "object"];
                };
                readonly schema_id: {
                    readonly description: "Data shape identifier.";
                    readonly type: "string";
                };
                readonly signing: {
                    readonly additionalProperties: false;
                    readonly properties: {
                        readonly algorithm: {
                            readonly default: null;
                            readonly oneOf: readonly [{
                                readonly type: "null";
                            }, {
                                readonly description: "Signature algorithm label.";
                                readonly type: "string";
                            }];
                        };
                        readonly key_id: {
                            readonly description: "Signing key identifier.";
                            readonly type: "string";
                        };
                        readonly signature: {
                            readonly description: "Schema signature string.";
                            readonly type: "string";
                        };
                    };
                    readonly required: readonly ["key_id", "signature"];
                    readonly type: "object";
                };
                readonly tenant_id: {
                    readonly description: "Tenant identifier.";
                    readonly minimum: 1;
                    readonly type: "integer";
                };
                readonly version: {
                    readonly description: "Data shape version identifier.";
                    readonly type: "string";
                };
            };
            readonly required: readonly ["tenant_id", "namespace_id", "schema_id", "version", "schema", "description", "created_at"];
            readonly type: "object";
        };
    };
    readonly required: readonly ["record"];
    readonly type: "object";
};
export declare const SchemasRegister_OUTPUT_SCHEMA: {
    readonly $schema: "https://json-schema.org/draft/2020-12/schema";
    readonly additionalProperties: false;
    readonly properties: {
        readonly record: {
            readonly additionalProperties: false;
            readonly properties: {
                readonly created_at: {
                    readonly oneOf: readonly [{
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly kind: {
                                readonly const: "unix_millis";
                            };
                            readonly value: {
                                readonly type: "integer";
                            };
                        };
                        readonly required: readonly ["kind", "value"];
                        readonly type: "object";
                    }, {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly kind: {
                                readonly const: "logical";
                            };
                            readonly value: {
                                readonly minimum: 0;
                                readonly type: "integer";
                            };
                        };
                        readonly required: readonly ["kind", "value"];
                        readonly type: "object";
                    }];
                };
                readonly description: {
                    readonly oneOf: readonly [{
                        readonly type: "null";
                    }, {
                        readonly description: "Optional schema description.";
                        readonly type: "string";
                    }];
                };
                readonly namespace_id: {
                    readonly description: "Namespace identifier.";
                    readonly minimum: 1;
                    readonly type: "integer";
                };
                readonly schema: {
                    readonly description: "JSON Schema payload.";
                    readonly type: readonly ["null", "boolean", "number", "string", "array", "object"];
                };
                readonly schema_id: {
                    readonly description: "Data shape identifier.";
                    readonly type: "string";
                };
                readonly signing: {
                    readonly additionalProperties: false;
                    readonly properties: {
                        readonly algorithm: {
                            readonly default: null;
                            readonly oneOf: readonly [{
                                readonly type: "null";
                            }, {
                                readonly description: "Signature algorithm label.";
                                readonly type: "string";
                            }];
                        };
                        readonly key_id: {
                            readonly description: "Signing key identifier.";
                            readonly type: "string";
                        };
                        readonly signature: {
                            readonly description: "Schema signature string.";
                            readonly type: "string";
                        };
                    };
                    readonly required: readonly ["key_id", "signature"];
                    readonly type: "object";
                };
                readonly tenant_id: {
                    readonly description: "Tenant identifier.";
                    readonly minimum: 1;
                    readonly type: "integer";
                };
                readonly version: {
                    readonly description: "Data shape version identifier.";
                    readonly type: "string";
                };
            };
            readonly required: readonly ["tenant_id", "namespace_id", "schema_id", "version", "schema", "description", "created_at"];
            readonly type: "object";
        };
    };
    readonly required: readonly ["record"];
    readonly type: "object";
};
export interface SchemasListRequest {
    cursor?: null | string;
    /** Maximum number of records to return. Constraints: Minimum: 1; Maximum: 1000. */
    limit?: number;
    /** Namespace identifier. Constraints: Minimum: 1. */
    namespace_id: number;
    /** Tenant identifier. Constraints: Minimum: 1. */
    tenant_id: number;
}
export interface SchemasListResponse {
    items: Array<Record<string, JsonValue>>;
    next_token: null | string;
}
export declare const SchemasList_INPUT_SCHEMA: {
    readonly $schema: "https://json-schema.org/draft/2020-12/schema";
    readonly additionalProperties: false;
    readonly properties: {
        readonly cursor: {
            readonly oneOf: readonly [{
                readonly type: "null";
            }, {
                readonly description: "Pagination cursor.";
                readonly type: "string";
            }];
        };
        readonly limit: {
            readonly description: "Maximum number of records to return.";
            readonly maximum: 1000;
            readonly minimum: 1;
            readonly type: "integer";
        };
        readonly namespace_id: {
            readonly description: "Namespace identifier.";
            readonly minimum: 1;
            readonly type: "integer";
        };
        readonly tenant_id: {
            readonly description: "Tenant identifier.";
            readonly minimum: 1;
            readonly type: "integer";
        };
    };
    readonly required: readonly ["tenant_id", "namespace_id"];
    readonly type: "object";
};
export declare const SchemasList_OUTPUT_SCHEMA: {
    readonly $schema: "https://json-schema.org/draft/2020-12/schema";
    readonly additionalProperties: false;
    readonly properties: {
        readonly items: {
            readonly items: {
                readonly additionalProperties: false;
                readonly properties: {
                    readonly created_at: {
                        readonly oneOf: readonly [{
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly kind: {
                                    readonly const: "unix_millis";
                                };
                                readonly value: {
                                    readonly type: "integer";
                                };
                            };
                            readonly required: readonly ["kind", "value"];
                            readonly type: "object";
                        }, {
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly kind: {
                                    readonly const: "logical";
                                };
                                readonly value: {
                                    readonly minimum: 0;
                                    readonly type: "integer";
                                };
                            };
                            readonly required: readonly ["kind", "value"];
                            readonly type: "object";
                        }];
                    };
                    readonly description: {
                        readonly oneOf: readonly [{
                            readonly type: "null";
                        }, {
                            readonly description: "Optional schema description.";
                            readonly type: "string";
                        }];
                    };
                    readonly namespace_id: {
                        readonly description: "Namespace identifier.";
                        readonly minimum: 1;
                        readonly type: "integer";
                    };
                    readonly schema: {
                        readonly description: "JSON Schema payload.";
                        readonly type: readonly ["null", "boolean", "number", "string", "array", "object"];
                    };
                    readonly schema_id: {
                        readonly description: "Data shape identifier.";
                        readonly type: "string";
                    };
                    readonly signing: {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly algorithm: {
                                readonly default: null;
                                readonly oneOf: readonly [{
                                    readonly type: "null";
                                }, {
                                    readonly description: "Signature algorithm label.";
                                    readonly type: "string";
                                }];
                            };
                            readonly key_id: {
                                readonly description: "Signing key identifier.";
                                readonly type: "string";
                            };
                            readonly signature: {
                                readonly description: "Schema signature string.";
                                readonly type: "string";
                            };
                        };
                        readonly required: readonly ["key_id", "signature"];
                        readonly type: "object";
                    };
                    readonly tenant_id: {
                        readonly description: "Tenant identifier.";
                        readonly minimum: 1;
                        readonly type: "integer";
                    };
                    readonly version: {
                        readonly description: "Data shape version identifier.";
                        readonly type: "string";
                    };
                };
                readonly required: readonly ["tenant_id", "namespace_id", "schema_id", "version", "schema", "description", "created_at"];
                readonly type: "object";
            };
            readonly type: "array";
        };
        readonly next_token: {
            readonly oneOf: readonly [{
                readonly type: "null";
            }, {
                readonly description: "Pagination token for the next page.";
                readonly type: "string";
            }];
        };
    };
    readonly required: readonly ["items", "next_token"];
    readonly type: "object";
};
export interface SchemasGetRequest {
    /** Namespace identifier. Constraints: Minimum: 1. */
    namespace_id: number;
    /** Data shape identifier. */
    schema_id: string;
    /** Tenant identifier. Constraints: Minimum: 1. */
    tenant_id: number;
    /** Data shape version identifier. */
    version: string;
}
export interface SchemasGetResponse {
    record: Record<string, JsonValue>;
}
export declare const SchemasGet_INPUT_SCHEMA: {
    readonly $schema: "https://json-schema.org/draft/2020-12/schema";
    readonly additionalProperties: false;
    readonly properties: {
        readonly namespace_id: {
            readonly description: "Namespace identifier.";
            readonly minimum: 1;
            readonly type: "integer";
        };
        readonly schema_id: {
            readonly description: "Data shape identifier.";
            readonly type: "string";
        };
        readonly tenant_id: {
            readonly description: "Tenant identifier.";
            readonly minimum: 1;
            readonly type: "integer";
        };
        readonly version: {
            readonly description: "Data shape version identifier.";
            readonly type: "string";
        };
    };
    readonly required: readonly ["tenant_id", "namespace_id", "schema_id", "version"];
    readonly type: "object";
};
export declare const SchemasGet_OUTPUT_SCHEMA: {
    readonly $schema: "https://json-schema.org/draft/2020-12/schema";
    readonly additionalProperties: false;
    readonly properties: {
        readonly record: {
            readonly additionalProperties: false;
            readonly properties: {
                readonly created_at: {
                    readonly oneOf: readonly [{
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly kind: {
                                readonly const: "unix_millis";
                            };
                            readonly value: {
                                readonly type: "integer";
                            };
                        };
                        readonly required: readonly ["kind", "value"];
                        readonly type: "object";
                    }, {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly kind: {
                                readonly const: "logical";
                            };
                            readonly value: {
                                readonly minimum: 0;
                                readonly type: "integer";
                            };
                        };
                        readonly required: readonly ["kind", "value"];
                        readonly type: "object";
                    }];
                };
                readonly description: {
                    readonly oneOf: readonly [{
                        readonly type: "null";
                    }, {
                        readonly description: "Optional schema description.";
                        readonly type: "string";
                    }];
                };
                readonly namespace_id: {
                    readonly description: "Namespace identifier.";
                    readonly minimum: 1;
                    readonly type: "integer";
                };
                readonly schema: {
                    readonly description: "JSON Schema payload.";
                    readonly type: readonly ["null", "boolean", "number", "string", "array", "object"];
                };
                readonly schema_id: {
                    readonly description: "Data shape identifier.";
                    readonly type: "string";
                };
                readonly signing: {
                    readonly additionalProperties: false;
                    readonly properties: {
                        readonly algorithm: {
                            readonly default: null;
                            readonly oneOf: readonly [{
                                readonly type: "null";
                            }, {
                                readonly description: "Signature algorithm label.";
                                readonly type: "string";
                            }];
                        };
                        readonly key_id: {
                            readonly description: "Signing key identifier.";
                            readonly type: "string";
                        };
                        readonly signature: {
                            readonly description: "Schema signature string.";
                            readonly type: "string";
                        };
                    };
                    readonly required: readonly ["key_id", "signature"];
                    readonly type: "object";
                };
                readonly tenant_id: {
                    readonly description: "Tenant identifier.";
                    readonly minimum: 1;
                    readonly type: "integer";
                };
                readonly version: {
                    readonly description: "Data shape version identifier.";
                    readonly type: "string";
                };
            };
            readonly required: readonly ["tenant_id", "namespace_id", "schema_id", "version", "schema", "description", "created_at"];
            readonly type: "object";
        };
    };
    readonly required: readonly ["record"];
    readonly type: "object";
};
export interface ScenariosListRequest {
    cursor?: null | string;
    /** Maximum number of records to return. Constraints: Minimum: 1; Maximum: 1000. */
    limit?: number;
    /** Namespace identifier. Constraints: Minimum: 1. */
    namespace_id: number;
    /** Tenant identifier. Constraints: Minimum: 1. */
    tenant_id: number;
}
export interface ScenariosListResponse {
    items: Array<Record<string, JsonValue>>;
    next_token: null | string;
}
export declare const ScenariosList_INPUT_SCHEMA: {
    readonly $schema: "https://json-schema.org/draft/2020-12/schema";
    readonly additionalProperties: false;
    readonly properties: {
        readonly cursor: {
            readonly oneOf: readonly [{
                readonly type: "null";
            }, {
                readonly description: "Pagination cursor.";
                readonly type: "string";
            }];
        };
        readonly limit: {
            readonly description: "Maximum number of records to return.";
            readonly maximum: 1000;
            readonly minimum: 1;
            readonly type: "integer";
        };
        readonly namespace_id: {
            readonly description: "Namespace identifier.";
            readonly minimum: 1;
            readonly type: "integer";
        };
        readonly tenant_id: {
            readonly description: "Tenant identifier.";
            readonly minimum: 1;
            readonly type: "integer";
        };
    };
    readonly required: readonly ["tenant_id", "namespace_id"];
    readonly type: "object";
};
export declare const ScenariosList_OUTPUT_SCHEMA: {
    readonly $schema: "https://json-schema.org/draft/2020-12/schema";
    readonly additionalProperties: false;
    readonly properties: {
        readonly items: {
            readonly items: {
                readonly additionalProperties: false;
                readonly properties: {
                    readonly namespace_id: {
                        readonly description: "Namespace identifier.";
                        readonly minimum: 1;
                        readonly type: "integer";
                    };
                    readonly scenario_id: {
                        readonly description: "Scenario identifier.";
                        readonly type: "string";
                    };
                    readonly spec_hash: {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly algorithm: {
                                readonly enum: readonly ["sha256"];
                                readonly type: "string";
                            };
                            readonly value: {
                                readonly description: "Lowercase hex digest.";
                                readonly type: "string";
                            };
                        };
                        readonly required: readonly ["algorithm", "value"];
                        readonly type: "object";
                    };
                };
                readonly required: readonly ["scenario_id", "namespace_id", "spec_hash"];
                readonly type: "object";
            };
            readonly type: "array";
        };
        readonly next_token: {
            readonly oneOf: readonly [{
                readonly type: "null";
            }, {
                readonly description: "Pagination token for the next page.";
                readonly type: "string";
            }];
        };
    };
    readonly required: readonly ["items", "next_token"];
    readonly type: "object";
};
export interface PrecheckRequest {
    data_shape: Record<string, JsonValue>;
    /** Namespace identifier. Constraints: Minimum: 1. */
    namespace_id: number;
    /** Asserted data payload. */
    payload: Array<JsonValue> | Record<string, JsonValue> | boolean | null | number | string;
    scenario_id?: null | string;
    spec?: JsonValue;
    stage_id?: null | string;
    /** Tenant identifier. Constraints: Minimum: 1. */
    tenant_id: number;
}
export interface PrecheckResponse {
    decision: Record<string, JsonValue>;
    gate_evaluations: Array<Record<string, JsonValue>>;
}
export declare const Precheck_INPUT_SCHEMA: {
    readonly $schema: "https://json-schema.org/draft/2020-12/schema";
    readonly additionalProperties: false;
    readonly properties: {
        readonly data_shape: {
            readonly additionalProperties: false;
            readonly properties: {
                readonly schema_id: {
                    readonly description: "Data shape identifier.";
                    readonly type: "string";
                };
                readonly version: {
                    readonly description: "Data shape version identifier.";
                    readonly type: "string";
                };
            };
            readonly required: readonly ["schema_id", "version"];
            readonly type: "object";
        };
        readonly namespace_id: {
            readonly description: "Namespace identifier.";
            readonly minimum: 1;
            readonly type: "integer";
        };
        readonly payload: {
            readonly description: "Asserted data payload.";
            readonly type: readonly ["null", "boolean", "number", "string", "array", "object"];
        };
        readonly scenario_id: {
            readonly oneOf: readonly [{
                readonly type: "null";
            }, {
                readonly description: "Scenario identifier.";
                readonly type: "string";
            }];
        };
        readonly spec: {
            readonly oneOf: readonly [{
                readonly type: "null";
            }, {
                readonly $ref: "decision-gate://contract/schemas/scenario.schema.json";
            }];
        };
        readonly stage_id: {
            readonly oneOf: readonly [{
                readonly type: "null";
            }, {
                readonly description: "Stage identifier override.";
                readonly type: "string";
            }];
        };
        readonly tenant_id: {
            readonly description: "Tenant identifier.";
            readonly minimum: 1;
            readonly type: "integer";
        };
    };
    readonly required: readonly ["tenant_id", "namespace_id", "data_shape", "payload"];
    readonly type: "object";
};
export declare const Precheck_OUTPUT_SCHEMA: {
    readonly $schema: "https://json-schema.org/draft/2020-12/schema";
    readonly additionalProperties: false;
    readonly properties: {
        readonly decision: {
            readonly oneOf: readonly [{
                readonly additionalProperties: false;
                readonly properties: {
                    readonly kind: {
                        readonly const: "start";
                    };
                    readonly stage_id: {
                        readonly description: "Initial stage identifier.";
                        readonly type: "string";
                    };
                };
                readonly required: readonly ["kind", "stage_id"];
                readonly type: "object";
            }, {
                readonly additionalProperties: false;
                readonly properties: {
                    readonly kind: {
                        readonly const: "complete";
                    };
                    readonly stage_id: {
                        readonly description: "Terminal stage identifier.";
                        readonly type: "string";
                    };
                };
                readonly required: readonly ["kind", "stage_id"];
                readonly type: "object";
            }, {
                readonly additionalProperties: false;
                readonly properties: {
                    readonly from_stage: {
                        readonly description: "Previous stage identifier.";
                        readonly type: "string";
                    };
                    readonly kind: {
                        readonly const: "advance";
                    };
                    readonly timeout: {
                        readonly type: "boolean";
                    };
                    readonly to_stage: {
                        readonly description: "Next stage identifier.";
                        readonly type: "string";
                    };
                };
                readonly required: readonly ["kind", "from_stage", "to_stage", "timeout"];
                readonly type: "object";
            }, {
                readonly additionalProperties: false;
                readonly properties: {
                    readonly kind: {
                        readonly const: "hold";
                    };
                    readonly summary: {
                        readonly additionalProperties: false;
                        readonly properties: {
                            readonly policy_tags: {
                                readonly description: "Policy tags applied to the summary.";
                                readonly items: {
                                    readonly type: "string";
                                };
                                readonly type: "array";
                            };
                            readonly retry_hint: {
                                readonly oneOf: readonly [{
                                    readonly type: "null";
                                }, {
                                    readonly description: "Optional retry hint.";
                                    readonly type: "string";
                                }];
                            };
                            readonly status: {
                                readonly description: "Summary status.";
                                readonly type: "string";
                            };
                            readonly unmet_gates: {
                                readonly items: {
                                    readonly description: "Gate identifier.";
                                    readonly type: "string";
                                };
                                readonly type: "array";
                            };
                        };
                        readonly required: readonly ["status", "unmet_gates", "retry_hint", "policy_tags"];
                        readonly type: "object";
                    };
                };
                readonly required: readonly ["kind", "summary"];
                readonly type: "object";
            }, {
                readonly additionalProperties: false;
                readonly properties: {
                    readonly kind: {
                        readonly const: "fail";
                    };
                    readonly reason: {
                        readonly description: "Failure reason.";
                        readonly type: "string";
                    };
                };
                readonly required: readonly ["kind", "reason"];
                readonly type: "object";
            }];
        };
        readonly gate_evaluations: {
            readonly items: {
                readonly additionalProperties: false;
                readonly properties: {
                    readonly gate_id: {
                        readonly description: "Gate identifier.";
                        readonly type: "string";
                    };
                    readonly status: {
                        readonly description: "Tri-state evaluation result.";
                        readonly enum: readonly ["True", "False", "Unknown"];
                        readonly type: "string";
                    };
                    readonly trace: {
                        readonly items: {
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly predicate: {
                                    readonly description: "Predicate identifier.";
                                    readonly type: "string";
                                };
                                readonly status: {
                                    readonly description: "Tri-state evaluation result.";
                                    readonly enum: readonly ["True", "False", "Unknown"];
                                    readonly type: "string";
                                };
                            };
                            readonly required: readonly ["predicate", "status"];
                            readonly type: "object";
                        };
                        readonly type: "array";
                    };
                };
                readonly required: readonly ["gate_id", "status", "trace"];
                readonly type: "object";
            };
            readonly type: "array";
        };
    };
    readonly required: readonly ["decision", "gate_evaluations"];
    readonly type: "object";
};
export declare abstract class GeneratedDecisionGateClient {
    protected abstract callTool<T>(name: string, arguments_: JsonValue): Promise<T>;
    /**
     * Register a ScenarioSpec, validate it, and return the canonical hash used for integrity checks.
     *
     * Notes:
     * - Use before starting runs; scenario_id becomes the stable handle for later calls.
     * - Validates stage/gate/predicate IDs, RET trees, and predicate references.
     * - Spec hash is deterministic; store it for audit and runpack integrity.
     * - Fails closed on invalid specs or duplicate scenario IDs.
     *
     * Examples:
     * - Register the example scenario spec.
     *   Input:
     *   ```json
     *   {
     *     "spec": {
     *       "default_tenant_id": null,
     *       "namespace_id": 1,
     *       "policies": [],
     *       "predicates": [
     *         {
     *           "comparator": "equals",
     *           "expected": "production",
     *           "policy_tags": [],
     *           "predicate": "env_is_prod",
     *           "query": {
     *             "params": {
     *               "key": "DEPLOY_ENV"
     *             },
     *             "predicate": "get",
     *             "provider_id": "env"
     *           }
     *         },
     *         {
     *           "comparator": "equals",
     *           "expected": true,
     *           "policy_tags": [],
     *           "predicate": "after_freeze",
     *           "query": {
     *             "params": {
     *               "timestamp": 1710000000000
     *             },
     *             "predicate": "after",
     *             "provider_id": "time"
     *           }
     *         }
     *       ],
     *       "scenario_id": "example-scenario",
     *       "schemas": [],
     *       "spec_version": "v1",
     *       "stages": [
     *         {
     *           "advance_to": {
     *             "kind": "terminal"
     *           },
     *           "entry_packets": [
     *             {
     *               "content_type": "application/json",
     *               "expiry": null,
     *               "packet_id": "packet-hello",
     *               "payload": {
     *                 "kind": "json",
     *                 "value": {
     *                   "message": "hello",
     *                   "purpose": "scenario entry packet"
     *                 }
     *               },
     *               "policy_tags": [],
     *               "schema_id": "schema-hello",
     *               "visibility_labels": [
     *                 "public"
     *               ]
     *             }
     *           ],
     *           "gates": [
     *             {
     *               "gate_id": "env_gate",
     *               "requirement": {
     *                 "Predicate": "env_is_prod"
     *               }
     *             },
     *             {
     *               "gate_id": "time_gate",
     *               "requirement": {
     *                 "Predicate": "after_freeze"
     *               }
     *             }
     *           ],
     *           "on_timeout": "fail",
     *           "stage_id": "main",
     *           "timeout": null
     *         }
     *       ]
     *     }
     *   }
     *   ```
     *   Output:
     *   ```json
     *   {
     *     "scenario_id": "example-scenario",
     *     "spec_hash": {
     *       "algorithm": "sha256",
     *       "value": "5c3a5b6bce0f4a2c9e22c4fa6a1e6d8d90b0f2dfed1b7f1e9b3d3b3d1f0c9b21"
     *     }
     *   }
     *   ```
     */
    scenario_define(request: ScenarioDefineRequest): Promise<ScenarioDefineResponse>;
    /**
     * Create a new run state for a scenario and optionally emit entry packets.
     *
     * Notes:
     * - Requires RunConfig (tenant_id, run_id, scenario_id, dispatch_targets).
     * - Use started_at to record the caller-supplied start timestamp.
     * - If issue_entry_packets is true, entry packets are disclosed immediately.
     * - Fails closed if run_id already exists or scenario_id is unknown.
     *
     * Examples:
     * - Start a run for the example scenario and issue entry packets.
     *   Input:
     *   ```json
     *   {
     *     "issue_entry_packets": true,
     *     "run_config": {
     *       "dispatch_targets": [
     *         {
     *           "agent_id": "agent-alpha",
     *           "kind": "agent"
     *         }
     *       ],
     *       "namespace_id": 1,
     *       "policy_tags": [],
     *       "run_id": "run-0001",
     *       "scenario_id": "example-scenario",
     *       "tenant_id": 1
     *     },
     *     "scenario_id": "example-scenario",
     *     "started_at": {
     *       "kind": "unix_millis",
     *       "value": 1710000000000
     *     }
     *   }
     *   ```
     *   Output:
     *   ```json
     *   {
     *     "current_stage_id": "main",
     *     "decisions": [],
     *     "dispatch_targets": [
     *       {
     *         "agent_id": "agent-alpha",
     *         "kind": "agent"
     *       }
     *     ],
     *     "gate_evals": [],
     *     "namespace_id": 1,
     *     "packets": [],
     *     "run_id": "run-0001",
     *     "scenario_id": "example-scenario",
     *     "spec_hash": {
     *       "algorithm": "sha256",
     *       "value": "5c3a5b6bce0f4a2c9e22c4fa6a1e6d8d90b0f2dfed1b7f1e9b3d3b3d1f0c9b21"
     *     },
     *     "stage_entered_at": {
     *       "kind": "unix_millis",
     *       "value": 1710000000000
     *     },
     *     "status": "active",
     *     "submissions": [],
     *     "tenant_id": 1,
     *     "tool_calls": [],
     *     "triggers": []
     *   }
     *   ```
     */
    scenario_start(request: ScenarioStartRequest): Promise<ScenarioStartResponse>;
    /**
     * Fetch a read-only run snapshot and safe summary without changing state.
     *
     * Notes:
     * - Use for polling or UI state; does not evaluate gates.
     * - Safe summaries omit evidence values and may include retry hints.
     * - Returns issued packet IDs to help track disclosures.
     *
     * Examples:
     * - Poll run status without advancing the run.
     *   Input:
     *   ```json
     *   {
     *     "request": {
     *       "correlation_id": null,
     *       "namespace_id": 1,
     *       "requested_at": {
     *         "kind": "unix_millis",
     *         "value": 1710000000000
     *       },
     *       "run_id": "run-0001",
     *       "tenant_id": 1
     *     },
     *     "scenario_id": "example-scenario"
     *   }
     *   ```
     *   Output:
     *   ```json
     *   {
     *     "current_stage_id": "main",
     *     "issued_packet_ids": [],
     *     "last_decision": null,
     *     "run_id": "run-0001",
     *     "safe_summary": null,
     *     "scenario_id": "example-scenario",
     *     "status": "active"
     *   }
     *   ```
     */
    scenario_status(request: ScenarioStatusRequest): Promise<ScenarioStatusResponse>;
    /**
     * Evaluate gates in response to an agent-driven next request.
     *
     * Notes:
     * - Idempotent by trigger_id; repeated calls return the same decision.
     * - Records decision, evidence, and packet disclosures in run state.
     * - Requires an active run; completed or failed runs do not advance.
     *
     * Examples:
     * - Evaluate the next agent-driven step for a run.
     *   Input:
     *   ```json
     *   {
     *     "request": {
     *       "agent_id": "agent-alpha",
     *       "correlation_id": null,
     *       "namespace_id": 1,
     *       "run_id": "run-0001",
     *       "tenant_id": 1,
     *       "time": {
     *         "kind": "unix_millis",
     *         "value": 1710000000000
     *       },
     *       "trigger_id": "trigger-0001"
     *     },
     *     "scenario_id": "example-scenario"
     *   }
     *   ```
     *   Output:
     *   ```json
     *   {
     *     "decision": {
     *       "correlation_id": null,
     *       "decided_at": {
     *         "kind": "unix_millis",
     *         "value": 1710000000000
     *       },
     *       "decision_id": "decision-0001",
     *       "outcome": {
     *         "kind": "complete",
     *         "stage_id": "main"
     *       },
     *       "seq": 0,
     *       "stage_id": "main",
     *       "trigger_id": "trigger-0001"
     *     },
     *     "packets": [],
     *     "status": "completed"
     *   }
     *   ```
     */
    scenario_next(request: ScenarioNextRequest): Promise<ScenarioNextResponse>;
    /**
     * Submit external artifacts into run state for audit and later evaluation.
     *
     * Notes:
     * - Payload is hashed and stored as a submission record.
     * - Does not advance the run by itself.
     * - Use for artifacts the model or operator supplies.
     *
     * Examples:
     * - Submit an external artifact for audit and later evaluation.
     *   Input:
     *   ```json
     *   {
     *     "request": {
     *       "content_type": "application/json",
     *       "correlation_id": null,
     *       "namespace_id": 1,
     *       "payload": {
     *         "kind": "json",
     *         "value": {
     *           "artifact": "attestation",
     *           "status": "approved"
     *         }
     *       },
     *       "run_id": "run-0001",
     *       "submission_id": "submission-0001",
     *       "submitted_at": {
     *         "kind": "unix_millis",
     *         "value": 1710000000000
     *       },
     *       "tenant_id": 1
     *     },
     *     "scenario_id": "example-scenario"
     *   }
     *   ```
     *   Output:
     *   ```json
     *   {
     *     "record": {
     *       "content_hash": {
     *         "algorithm": "sha256",
     *         "value": "5c3a5b6bce0f4a2c9e22c4fa6a1e6d8d90b0f2dfed1b7f1e9b3d3b3d1f0c9b21"
     *       },
     *       "content_type": "application/json",
     *       "correlation_id": null,
     *       "payload": {
     *         "kind": "json",
     *         "value": {
     *           "artifact": "attestation",
     *           "status": "approved"
     *         }
     *       },
     *       "run_id": "run-0001",
     *       "submission_id": "submission-0001",
     *       "submitted_at": {
     *         "kind": "unix_millis",
     *         "value": 1710000000000
     *       }
     *     }
     *   }
     *   ```
     */
    scenario_submit(request: ScenarioSubmitRequest): Promise<ScenarioSubmitResponse>;
    /**
     * Submit a trigger event (scheduler/external) and evaluate the run.
     *
     * Notes:
     * - Trigger time is supplied by the caller; no wall-clock reads.
     * - Records the trigger event and resulting decision.
     * - Use for time-based or external system triggers.
     *
     * Examples:
     * - Advance a run from a scheduler or external trigger.
     *   Input:
     *   ```json
     *   {
     *     "scenario_id": "example-scenario",
     *     "trigger": {
     *       "correlation_id": null,
     *       "kind": "tick",
     *       "namespace_id": 1,
     *       "payload": null,
     *       "run_id": "run-0001",
     *       "source_id": "scheduler-01",
     *       "tenant_id": 1,
     *       "time": {
     *         "kind": "unix_millis",
     *         "value": 1710000000000
     *       },
     *       "trigger_id": "trigger-0001"
     *     }
     *   }
     *   ```
     *   Output:
     *   ```json
     *   {
     *     "decision": {
     *       "correlation_id": null,
     *       "decided_at": {
     *         "kind": "unix_millis",
     *         "value": 1710000000000
     *       },
     *       "decision_id": "decision-0001",
     *       "outcome": {
     *         "kind": "complete",
     *         "stage_id": "main"
     *       },
     *       "seq": 0,
     *       "stage_id": "main",
     *       "trigger_id": "trigger-0001"
     *     },
     *     "packets": [],
     *     "status": "completed"
     *   }
     *   ```
     */
    scenario_trigger(request: ScenarioTriggerRequest): Promise<ScenarioTriggerResponse>;
    /**
     * Query an evidence provider with full run context and disclosure policy.
     *
     * Notes:
     * - Disclosure policy may redact raw values; hashes/anchors still returned.
     * - Use for diagnostics or preflight checks; runtime uses the same provider logic.
     * - Requires provider_id, predicate, and full EvidenceContext.
     *
     * Examples:
     * - Query an evidence provider using the run context.
     *   Input:
     *   ```json
     *   {
     *     "context": {
     *       "correlation_id": null,
     *       "namespace_id": 1,
     *       "run_id": "run-0001",
     *       "scenario_id": "example-scenario",
     *       "stage_id": "main",
     *       "tenant_id": 1,
     *       "trigger_id": "trigger-0001",
     *       "trigger_time": {
     *         "kind": "unix_millis",
     *         "value": 1710000000000
     *       }
     *     },
     *     "query": {
     *       "params": {
     *         "key": "DEPLOY_ENV"
     *       },
     *       "predicate": "get",
     *       "provider_id": "env"
     *     }
     *   }
     *   ```
     *   Output:
     *   ```json
     *   {
     *     "result": {
     *       "content_type": "text/plain",
     *       "error": null,
     *       "evidence_anchor": {
     *         "anchor_type": "env",
     *         "anchor_value": "DEPLOY_ENV"
     *       },
     *       "evidence_hash": {
     *         "algorithm": "sha256",
     *         "value": "5c3a5b6bce0f4a2c9e22c4fa6a1e6d8d90b0f2dfed1b7f1e9b3d3b3d1f0c9b21"
     *       },
     *       "evidence_ref": null,
     *       "lane": "verified",
     *       "signature": null,
     *       "value": {
     *         "kind": "json",
     *         "value": "production"
     *       }
     *     }
     *   }
     *   ```
     */
    evidence_query(request: EvidenceQueryRequest): Promise<EvidenceQueryResponse>;
    /**
     * Export deterministic runpack artifacts for offline verification.
     *
     * Notes:
     * - Writes manifest and logs to output_dir; generated_at is recorded in the manifest.
     * - include_verification adds a verification report artifact.
     * - Use after runs complete or for audit snapshots.
     *
     * Examples:
     * - Export a runpack with manifest metadata.
     *   Input:
     *   ```json
     *   {
     *     "generated_at": {
     *       "kind": "unix_millis",
     *       "value": 1710000000000
     *     },
     *     "include_verification": false,
     *     "manifest_name": "manifest.json",
     *     "namespace_id": 1,
     *     "output_dir": "/var/lib/decision-gate/runpacks/run-0001",
     *     "run_id": "run-0001",
     *     "scenario_id": "example-scenario",
     *     "tenant_id": 1
     *   }
     *   ```
     *   Output:
     *   ```json
     *   {
     *     "manifest": {
     *       "artifacts": [
     *         {
     *           "artifact_id": "decision_log",
     *           "content_type": "application/json",
     *           "hash": {
     *             "algorithm": "sha256",
     *             "value": "5c3a5b6bce0f4a2c9e22c4fa6a1e6d8d90b0f2dfed1b7f1e9b3d3b3d1f0c9b21"
     *           },
     *           "kind": "decision_log",
     *           "path": "decision_log.json",
     *           "required": true
     *         }
     *       ],
     *       "generated_at": {
     *         "kind": "unix_millis",
     *         "value": 1710000000000
     *       },
     *       "hash_algorithm": "sha256",
     *       "integrity": {
     *         "file_hashes": [
     *           {
     *             "hash": {
     *               "algorithm": "sha256",
     *               "value": "5c3a5b6bce0f4a2c9e22c4fa6a1e6d8d90b0f2dfed1b7f1e9b3d3b3d1f0c9b21"
     *             },
     *             "path": "decision_log.json"
     *           }
     *         ],
     *         "root_hash": {
     *           "algorithm": "sha256",
     *           "value": "5c3a5b6bce0f4a2c9e22c4fa6a1e6d8d90b0f2dfed1b7f1e9b3d3b3d1f0c9b21"
     *         }
     *       },
     *       "manifest_version": "v1",
     *       "namespace_id": 1,
     *       "run_id": "run-0001",
     *       "scenario_id": "example-scenario",
     *       "spec_hash": {
     *         "algorithm": "sha256",
     *         "value": "5c3a5b6bce0f4a2c9e22c4fa6a1e6d8d90b0f2dfed1b7f1e9b3d3b3d1f0c9b21"
     *       },
     *       "tenant_id": 1,
     *       "verifier_mode": "offline_strict"
     *     },
     *     "report": null,
     *     "storage_uri": null
     *   }
     *   ```
     */
    runpack_export(request: RunpackExportRequest): Promise<RunpackExportResponse>;
    /**
     * Verify a runpack manifest and artifacts offline.
     *
     * Notes:
     * - Validates hashes, integrity root, and decision log structure.
     * - Fails closed on missing or tampered files.
     * - Use in CI or offline audit pipelines.
     *
     * Examples:
     * - Verify a runpack manifest and artifacts offline.
     *   Input:
     *   ```json
     *   {
     *     "manifest_path": "manifest.json",
     *     "runpack_dir": "/var/lib/decision-gate/runpacks/run-0001"
     *   }
     *   ```
     *   Output:
     *   ```json
     *   {
     *     "report": {
     *       "checked_files": 12,
     *       "errors": [],
     *       "status": "pass"
     *     },
     *     "status": "pass"
     *   }
     *   ```
     */
    runpack_verify(request: RunpackVerifyRequest): Promise<RunpackVerifyResponse>;
    /**
     * List registered evidence providers and capabilities summary.
     *
     * Notes:
     * - Returns provider identifiers and transport metadata.
     * - Results are scoped by auth policy.
     *
     * Examples:
     * - List registered evidence providers.
     *   Input:
     *   ```json
     *   {}
     *   ```
     *   Output:
     *   ```json
     *   {
     *     "providers": [
     *       {
     *         "predicates": [
     *           "get"
     *         ],
     *         "provider_id": "env",
     *         "transport": "builtin"
     *       }
     *     ]
     *   }
     *   ```
     */
    providers_list(request: ProvidersListRequest): Promise<ProvidersListResponse>;
    /**
     * Fetch the canonical provider contract JSON and hash for a provider.
     *
     * Notes:
     * - Returns the provider contract as loaded by the MCP server.
     * - Includes a canonical hash for audit and reproducibility.
     * - Subject to provider disclosure policy and authz.
     *
     * Examples:
     * - Fetch the contract JSON for a provider.
     *   Input:
     *   ```json
     *   {
     *     "provider_id": "json"
     *   }
     *   ```
     *   Output:
     *   ```json
     *   {
     *     "contract": {
     *       "config_schema": {
     *         "additionalProperties": false,
     *         "type": "object"
     *       },
     *       "description": "Reads JSON or YAML files and evaluates JSONPath.",
     *       "name": "JSON Provider",
     *       "notes": [],
     *       "predicates": [],
     *       "provider_id": "json",
     *       "transport": "builtin"
     *     },
     *     "contract_hash": {
     *       "algorithm": "sha256",
     *       "value": "5c3a5b6bce0f4a2c9e22c4fa6a1e6d8d90b0f2dfed1b7f1e9b3d3b3d1f0c9b21"
     *     },
     *     "provider_id": "json",
     *     "source": "builtin",
     *     "version": null
     *   }
     *   ```
     */
    provider_contract_get(request: ProviderContractGetRequest): Promise<ProviderContractGetResponse>;
    /**
     * Fetch predicate schema details (params/result/comparators) for a provider.
     *
     * Notes:
     * - Returns compiled schema metadata for a single predicate.
     * - Includes comparator allow-lists and predicate examples.
     * - Subject to provider disclosure policy and authz.
     *
     * Examples:
     * - Fetch predicate schema details for a provider.
     *   Input:
     *   ```json
     *   {
     *     "predicate": "path",
     *     "provider_id": "json"
     *   }
     *   ```
     *   Output:
     *   ```json
     *   {
     *     "allowed_comparators": [
     *       "equals",
     *       "in_set",
     *       "exists",
     *       "not_exists"
     *     ],
     *     "anchor_types": [],
     *     "content_types": [
     *       "application/json"
     *     ],
     *     "contract_hash": {
     *       "algorithm": "sha256",
     *       "value": "5c3a5b6bce0f4a2c9e22c4fa6a1e6d8d90b0f2dfed1b7f1e9b3d3b3d1f0c9b21"
     *     },
     *     "determinism": "external",
     *     "examples": [],
     *     "params_required": true,
     *     "params_schema": {
     *       "properties": {
     *         "file": {
     *           "type": "string"
     *         },
     *         "jsonpath": {
     *           "type": "string"
     *         }
     *       },
     *       "required": [
     *         "file"
     *       ],
     *       "type": "object"
     *     },
     *     "predicate": "path",
     *     "provider_id": "json",
     *     "result_schema": {
     *       "type": [
     *         "null",
     *         "string",
     *         "number",
     *         "boolean",
     *         "array",
     *         "object"
     *       ]
     *     }
     *   }
     *   ```
     */
    provider_schema_get(request: ProviderSchemaGetRequest): Promise<ProviderSchemaGetResponse>;
    /**
     * Register a data shape schema for a tenant and namespace.
     *
     * Notes:
     * - Schemas are immutable; registering the same version twice fails.
     * - Provide created_at to record when the schema was authored.
     *
     * Examples:
     * - Register a data shape schema.
     *   Input:
     *   ```json
     *   {
     *     "record": {
     *       "created_at": {
     *         "kind": "unix_millis",
     *         "value": 1710000000000
     *       },
     *       "description": "Asserted payload schema.",
     *       "namespace_id": 1,
     *       "schema": {
     *         "additionalProperties": false,
     *         "properties": {
     *           "deploy_env": {
     *             "type": "string"
     *           }
     *         },
     *         "required": [
     *           "deploy_env"
     *         ],
     *         "type": "object"
     *       },
     *       "schema_id": "asserted_payload",
     *       "tenant_id": 1,
     *       "version": "v1"
     *     }
     *   }
     *   ```
     *   Output:
     *   ```json
     *   {
     *     "record": {
     *       "created_at": {
     *         "kind": "unix_millis",
     *         "value": 1710000000000
     *       },
     *       "description": "Asserted payload schema.",
     *       "namespace_id": 1,
     *       "schema": {
     *         "additionalProperties": false,
     *         "properties": {
     *           "deploy_env": {
     *             "type": "string"
     *           }
     *         },
     *         "required": [
     *           "deploy_env"
     *         ],
     *         "type": "object"
     *       },
     *       "schema_id": "asserted_payload",
     *       "tenant_id": 1,
     *       "version": "v1"
     *     }
     *   }
     *   ```
     */
    schemas_register(request: SchemasRegisterRequest): Promise<SchemasRegisterResponse>;
    /**
     * List registered data shapes for a tenant and namespace.
     *
     * Notes:
     * - Requires tenant_id and namespace_id.
     * - Supports pagination via cursor + limit.
     *
     * Examples:
     * - List data shapes for a namespace.
     *   Input:
     *   ```json
     *   {
     *     "cursor": null,
     *     "limit": 50,
     *     "namespace_id": 1,
     *     "tenant_id": 1
     *   }
     *   ```
     *   Output:
     *   ```json
     *   {
     *     "items": [
     *       {
     *         "created_at": {
     *           "kind": "unix_millis",
     *           "value": 1710000000000
     *         },
     *         "description": "Asserted payload schema.",
     *         "namespace_id": 1,
     *         "schema": {
     *           "additionalProperties": false,
     *           "properties": {
     *             "deploy_env": {
     *               "type": "string"
     *             }
     *           },
     *           "required": [
     *             "deploy_env"
     *           ],
     *           "type": "object"
     *         },
     *         "schema_id": "asserted_payload",
     *         "tenant_id": 1,
     *         "version": "v1"
     *       }
     *     ],
     *     "next_token": null
     *   }
     *   ```
     */
    schemas_list(request: SchemasListRequest): Promise<SchemasListResponse>;
    /**
     * Fetch a specific data shape by identifier and version.
     *
     * Notes:
     * - Requires tenant_id, namespace_id, schema_id, and version.
     * - Fails closed when schema is missing.
     *
     * Examples:
     * - Fetch a data shape by identifier and version.
     *   Input:
     *   ```json
     *   {
     *     "namespace_id": 1,
     *     "schema_id": "asserted_payload",
     *     "tenant_id": 1,
     *     "version": "v1"
     *   }
     *   ```
     *   Output:
     *   ```json
     *   {
     *     "record": {
     *       "created_at": {
     *         "kind": "unix_millis",
     *         "value": 1710000000000
     *       },
     *       "description": "Asserted payload schema.",
     *       "namespace_id": 1,
     *       "schema": {
     *         "additionalProperties": false,
     *         "properties": {
     *           "deploy_env": {
     *             "type": "string"
     *           }
     *         },
     *         "required": [
     *           "deploy_env"
     *         ],
     *         "type": "object"
     *       },
     *       "schema_id": "asserted_payload",
     *       "tenant_id": 1,
     *       "version": "v1"
     *     }
     *   }
     *   ```
     */
    schemas_get(request: SchemasGetRequest): Promise<SchemasGetResponse>;
    /**
     * List registered scenarios for a tenant and namespace.
     *
     * Notes:
     * - Requires tenant_id and namespace_id.
     * - Returns scenario identifiers and hashes.
     *
     * Examples:
     * - List scenarios for a namespace.
     *   Input:
     *   ```json
     *   {
     *     "cursor": null,
     *     "limit": 50,
     *     "namespace_id": 1,
     *     "tenant_id": 1
     *   }
     *   ```
     *   Output:
     *   ```json
     *   {
     *     "items": [
     *       {
     *         "namespace_id": 1,
     *         "scenario_id": "example-scenario",
     *         "spec_hash": {
     *           "algorithm": "sha256",
     *           "value": "5c3a5b6bce0f4a2c9e22c4fa6a1e6d8d90b0f2dfed1b7f1e9b3d3b3d1f0c9b21"
     *         }
     *       }
     *     ],
     *     "next_token": null
     *   }
     *   ```
     */
    scenarios_list(request: ScenariosListRequest): Promise<ScenariosListResponse>;
    /**
     * Evaluate a scenario against asserted data without mutating state.
     *
     * Notes:
     * - Validates asserted data against a registered shape.
     * - Does not mutate run state; intended for simulation.
     *
     * Examples:
     * - Precheck a scenario with asserted data.
     *   Input:
     *   ```json
     *   {
     *     "data_shape": {
     *       "schema_id": "asserted_payload",
     *       "version": "v1"
     *     },
     *     "namespace_id": 1,
     *     "payload": {
     *       "deploy_env": "production"
     *     },
     *     "scenario_id": "example-scenario",
     *     "spec": null,
     *     "stage_id": null,
     *     "tenant_id": 1
     *   }
     *   ```
     *   Output:
     *   ```json
     *   {
     *     "decision": {
     *       "kind": "hold",
     *       "summary": {
     *         "policy_tags": [],
     *         "retry_hint": "await_evidence",
     *         "status": "hold",
     *         "unmet_gates": [
     *           "ready"
     *         ]
     *       }
     *     },
     *     "gate_evaluations": []
     *   }
     *   ```
     */
    precheck(request: PrecheckRequest): Promise<PrecheckResponse>;
}
export type SchemaValidator = (schema: unknown, payload: unknown) => void;
export declare class SchemaValidationError extends Error {
    readonly errors?: unknown;
    constructor(message: string, errors?: unknown);
}
export declare function validateSchemaWith(validator: SchemaValidator, schema: unknown, payload: unknown): void;
export declare function validateSchemaWithAjv(schema: unknown, payload: unknown): Promise<void>;
export declare function validateScenarioDefineRequest(payload: ScenarioDefineRequest, validator: SchemaValidator): void;
export declare function validateScenarioDefineResponse(payload: ScenarioDefineResponse, validator: SchemaValidator): void;
export declare function validateScenarioDefineRequestWithAjv(payload: ScenarioDefineRequest): Promise<void>;
export declare function validateScenarioDefineResponseWithAjv(payload: ScenarioDefineResponse): Promise<void>;
export declare function validateScenarioStartRequest(payload: ScenarioStartRequest, validator: SchemaValidator): void;
export declare function validateScenarioStartResponse(payload: ScenarioStartResponse, validator: SchemaValidator): void;
export declare function validateScenarioStartRequestWithAjv(payload: ScenarioStartRequest): Promise<void>;
export declare function validateScenarioStartResponseWithAjv(payload: ScenarioStartResponse): Promise<void>;
export declare function validateScenarioStatusRequest(payload: ScenarioStatusRequest, validator: SchemaValidator): void;
export declare function validateScenarioStatusResponse(payload: ScenarioStatusResponse, validator: SchemaValidator): void;
export declare function validateScenarioStatusRequestWithAjv(payload: ScenarioStatusRequest): Promise<void>;
export declare function validateScenarioStatusResponseWithAjv(payload: ScenarioStatusResponse): Promise<void>;
export declare function validateScenarioNextRequest(payload: ScenarioNextRequest, validator: SchemaValidator): void;
export declare function validateScenarioNextResponse(payload: ScenarioNextResponse, validator: SchemaValidator): void;
export declare function validateScenarioNextRequestWithAjv(payload: ScenarioNextRequest): Promise<void>;
export declare function validateScenarioNextResponseWithAjv(payload: ScenarioNextResponse): Promise<void>;
export declare function validateScenarioSubmitRequest(payload: ScenarioSubmitRequest, validator: SchemaValidator): void;
export declare function validateScenarioSubmitResponse(payload: ScenarioSubmitResponse, validator: SchemaValidator): void;
export declare function validateScenarioSubmitRequestWithAjv(payload: ScenarioSubmitRequest): Promise<void>;
export declare function validateScenarioSubmitResponseWithAjv(payload: ScenarioSubmitResponse): Promise<void>;
export declare function validateScenarioTriggerRequest(payload: ScenarioTriggerRequest, validator: SchemaValidator): void;
export declare function validateScenarioTriggerResponse(payload: ScenarioTriggerResponse, validator: SchemaValidator): void;
export declare function validateScenarioTriggerRequestWithAjv(payload: ScenarioTriggerRequest): Promise<void>;
export declare function validateScenarioTriggerResponseWithAjv(payload: ScenarioTriggerResponse): Promise<void>;
export declare function validateEvidenceQueryRequest(payload: EvidenceQueryRequest, validator: SchemaValidator): void;
export declare function validateEvidenceQueryResponse(payload: EvidenceQueryResponse, validator: SchemaValidator): void;
export declare function validateEvidenceQueryRequestWithAjv(payload: EvidenceQueryRequest): Promise<void>;
export declare function validateEvidenceQueryResponseWithAjv(payload: EvidenceQueryResponse): Promise<void>;
export declare function validateRunpackExportRequest(payload: RunpackExportRequest, validator: SchemaValidator): void;
export declare function validateRunpackExportResponse(payload: RunpackExportResponse, validator: SchemaValidator): void;
export declare function validateRunpackExportRequestWithAjv(payload: RunpackExportRequest): Promise<void>;
export declare function validateRunpackExportResponseWithAjv(payload: RunpackExportResponse): Promise<void>;
export declare function validateRunpackVerifyRequest(payload: RunpackVerifyRequest, validator: SchemaValidator): void;
export declare function validateRunpackVerifyResponse(payload: RunpackVerifyResponse, validator: SchemaValidator): void;
export declare function validateRunpackVerifyRequestWithAjv(payload: RunpackVerifyRequest): Promise<void>;
export declare function validateRunpackVerifyResponseWithAjv(payload: RunpackVerifyResponse): Promise<void>;
export declare function validateProvidersListRequest(payload: ProvidersListRequest, validator: SchemaValidator): void;
export declare function validateProvidersListResponse(payload: ProvidersListResponse, validator: SchemaValidator): void;
export declare function validateProvidersListRequestWithAjv(payload: ProvidersListRequest): Promise<void>;
export declare function validateProvidersListResponseWithAjv(payload: ProvidersListResponse): Promise<void>;
export declare function validateProviderContractGetRequest(payload: ProviderContractGetRequest, validator: SchemaValidator): void;
export declare function validateProviderContractGetResponse(payload: ProviderContractGetResponse, validator: SchemaValidator): void;
export declare function validateProviderContractGetRequestWithAjv(payload: ProviderContractGetRequest): Promise<void>;
export declare function validateProviderContractGetResponseWithAjv(payload: ProviderContractGetResponse): Promise<void>;
export declare function validateProviderSchemaGetRequest(payload: ProviderSchemaGetRequest, validator: SchemaValidator): void;
export declare function validateProviderSchemaGetResponse(payload: ProviderSchemaGetResponse, validator: SchemaValidator): void;
export declare function validateProviderSchemaGetRequestWithAjv(payload: ProviderSchemaGetRequest): Promise<void>;
export declare function validateProviderSchemaGetResponseWithAjv(payload: ProviderSchemaGetResponse): Promise<void>;
export declare function validateSchemasRegisterRequest(payload: SchemasRegisterRequest, validator: SchemaValidator): void;
export declare function validateSchemasRegisterResponse(payload: SchemasRegisterResponse, validator: SchemaValidator): void;
export declare function validateSchemasRegisterRequestWithAjv(payload: SchemasRegisterRequest): Promise<void>;
export declare function validateSchemasRegisterResponseWithAjv(payload: SchemasRegisterResponse): Promise<void>;
export declare function validateSchemasListRequest(payload: SchemasListRequest, validator: SchemaValidator): void;
export declare function validateSchemasListResponse(payload: SchemasListResponse, validator: SchemaValidator): void;
export declare function validateSchemasListRequestWithAjv(payload: SchemasListRequest): Promise<void>;
export declare function validateSchemasListResponseWithAjv(payload: SchemasListResponse): Promise<void>;
export declare function validateSchemasGetRequest(payload: SchemasGetRequest, validator: SchemaValidator): void;
export declare function validateSchemasGetResponse(payload: SchemasGetResponse, validator: SchemaValidator): void;
export declare function validateSchemasGetRequestWithAjv(payload: SchemasGetRequest): Promise<void>;
export declare function validateSchemasGetResponseWithAjv(payload: SchemasGetResponse): Promise<void>;
export declare function validateScenariosListRequest(payload: ScenariosListRequest, validator: SchemaValidator): void;
export declare function validateScenariosListResponse(payload: ScenariosListResponse, validator: SchemaValidator): void;
export declare function validateScenariosListRequestWithAjv(payload: ScenariosListRequest): Promise<void>;
export declare function validateScenariosListResponseWithAjv(payload: ScenariosListResponse): Promise<void>;
export declare function validatePrecheckRequest(payload: PrecheckRequest, validator: SchemaValidator): void;
export declare function validatePrecheckResponse(payload: PrecheckResponse, validator: SchemaValidator): void;
export declare function validatePrecheckRequestWithAjv(payload: PrecheckRequest): Promise<void>;
export declare function validatePrecheckResponseWithAjv(payload: PrecheckResponse): Promise<void>;
