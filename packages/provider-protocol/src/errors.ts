// JSON-RPC 2.0 standard error codes
export const PARSE_ERROR = -32700;
export const INVALID_REQUEST = -32600;
export const METHOD_NOT_FOUND = -32601;
export const INVALID_PARAMS = -32602;
export const INTERNAL_ERROR = -32603;

// PICE-specific error codes (-32000 to -32099)
export const PROVIDER_NOT_INITIALIZED = -32000;
export const SESSION_NOT_FOUND = -32001;
export const AUTH_FAILED = -32002;
export const RATE_LIMITED = -32003;
export const MODEL_NOT_AVAILABLE = -32004;
