// sdks/typescript/src/errors.ts
// ============================================================================
// Module: SDK Errors
// Description: Error types for Decision Gate TypeScript client SDK.
// Purpose: Provide structured error reporting for transport and JSON-RPC failures.
// Dependencies: stdlib
// ============================================================================
export class DecisionGateError extends Error {
    cause;
    constructor(message, cause) {
        super(message);
        this.name = "DecisionGateError";
        this.cause = cause;
    }
}
export class DecisionGateTransportError extends DecisionGateError {
    statusCode;
    body;
    constructor(message, options) {
        super(message, options?.cause);
        this.name = "DecisionGateTransportError";
        this.statusCode = options?.statusCode;
        this.body = options?.body;
    }
}
export class DecisionGateProtocolError extends DecisionGateError {
    constructor(message, cause) {
        super(message, cause);
        this.name = "DecisionGateProtocolError";
    }
}
export class DecisionGateRpcError extends DecisionGateError {
    code;
    data;
    requestId;
    constructor(code, message, options) {
        super(message);
        this.name = "DecisionGateRpcError";
        this.code = code;
        this.data = options?.data;
        this.requestId = options?.requestId;
    }
    get kind() {
        const value = this.data?.kind;
        return typeof value === "string" ? value : undefined;
    }
    get retryable() {
        const value = this.data?.retryable;
        return typeof value === "boolean" ? value : undefined;
    }
    get retryAfterMs() {
        const value = this.data?.retry_after_ms;
        return typeof value === "number" ? value : undefined;
    }
}
