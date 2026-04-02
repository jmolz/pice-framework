---
paths:
  - "crates/pice-protocol/**"
  - "packages/provider-protocol/**"
---

# JSON-RPC Provider Protocol Rules

## Protocol Basics

- JSON-RPC 2.0 over stdio (stdin/stdout)
- One JSON object per line (newline-delimited)
- Requests have `id` (core waits for response). Notifications have no `id` (fire-and-forget).

## Sync Requirement

The Rust `pice-protocol` crate and the TS `@pice/provider-protocol` package define the SAME types. They MUST stay in sync:

- Every message type change requires updating BOTH packages
- CI must include a roundtrip test: serialize in Rust → deserialize in TS, and vice versa
- Version numbers of both packages must match

## Adding a New Method

1. Define the request/response types in `pice-protocol` (Rust)
2. Mirror the types in `@pice/provider-protocol` (TS)
3. Add roundtrip serialization tests in both languages
4. Update the provider capability declaration if the method requires a new capability
5. Update `@pice/provider-base` handler routing
6. Implement in at least one provider
7. Update `docs/providers/protocol.md`

## Streaming

Streaming responses use JSON-RPC notifications (no `id`):
- `response/chunk` — text chunks from AI session
- `evaluate/result` — evaluation findings

The core accumulates chunks via the `ProviderHost::on_notification()` callback. In the Rust host, notifications received during `request()` are forwarded to the registered handler. Without a handler, notifications are logged at `debug` level.

On the TS side, `StdioTransport.onNotification` is an optional callback for incoming notifications. Notifications are identified by the absence of the `id` field per JSON-RPC 2.0 spec.

## Error Codes

| Code | Meaning |
|------|---------|
| -32700 | Parse error (invalid JSON) |
| -32600 | Invalid request |
| -32601 | Method not found |
| -32602 | Invalid params |
| -32603 | Internal error |
| -32000 | Provider not initialized |
| -32001 | Session not found |
| -32002 | Authentication failed |
| -32003 | Rate limited (include retry-after in data) |
| -32004 | Model not available |
