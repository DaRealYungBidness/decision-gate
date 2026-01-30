import { GeneratedDecisionGateClient } from "./_generated.js";
export type FetchLike = (input: RequestInfo, init?: RequestInit) => Promise<Response>;
export interface DecisionGateClientOptions {
    endpoint?: string;
    authToken?: string;
    timeoutMs?: number;
    headers?: Record<string, string>;
    fetch?: FetchLike;
    userAgent?: string;
}
export declare class DecisionGateClient extends GeneratedDecisionGateClient {
    private readonly endpoint;
    private readonly authToken?;
    private readonly timeoutMs;
    private readonly headers;
    private readonly fetcher;
    private readonly userAgent?;
    private requestId;
    constructor(options?: DecisionGateClientOptions);
    protected callTool<T>(name: string, arguments_: object): Promise<T>;
    private nextRequestId;
    private rpcErrorFromPayload;
}
