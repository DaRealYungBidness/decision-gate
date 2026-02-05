// sdks/typescript/src/index.ts
// ============================================================================
// Module: Decision Gate SDK Entry
// Description: Public exports for Decision Gate TypeScript client SDK.
// Purpose: Provide a stable import surface for SDK consumers.
// ============================================================================
export { DecisionGateClient } from "./client.js";
export { DecisionGateError, DecisionGateProtocolError, DecisionGateRpcError, DecisionGateTransportError, } from "./errors.js";
export * from "./_generated.js";
