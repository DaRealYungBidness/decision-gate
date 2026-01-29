export declare class DecisionGateError extends Error {
    readonly cause?: unknown;
    constructor(message: string, cause?: unknown);
}
export declare class DecisionGateTransportError extends DecisionGateError {
    readonly statusCode?: number;
    readonly body?: string;
    constructor(message: string, options?: {
        statusCode?: number;
        body?: string;
        cause?: unknown;
    });
}
export declare class DecisionGateProtocolError extends DecisionGateError {
    constructor(message: string, cause?: unknown);
}
export declare class DecisionGateRpcError extends DecisionGateError {
    readonly code: number;
    readonly data?: Record<string, unknown>;
    readonly requestId?: string;
    constructor(code: number, message: string, options?: {
        data?: Record<string, unknown>;
        requestId?: string;
    });
    get kind(): string | undefined;
    get retryable(): boolean | undefined;
    get retryAfterMs(): number | undefined;
}
