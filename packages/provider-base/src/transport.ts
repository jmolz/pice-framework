import { createInterface } from 'node:readline';
import type {
  JsonRpcResponse,
  JsonRpcErrorResponse,
  JsonRpcError,
  JsonRpcNotification,
  RequestId,
} from '@pice/provider-protocol';
import { PARSE_ERROR, INVALID_REQUEST, METHOD_NOT_FOUND } from '@pice/provider-protocol';

export type MethodHandler = (
  params: unknown,
  id: RequestId,
) => Promise<unknown>;

/**
 * JSON-RPC 2.0 transport over stdin/stdout.
 *
 * - Reads newline-delimited JSON-RPC from stdin
 * - Routes requests to registered method handlers
 * - Writes JSON-RPC responses to stdout
 * - All logging goes to stderr (never stdout)
 */
export class StdioTransport {
  private handlers = new Map<string, MethodHandler>();
  private running = false;

  onNotification?: (method: string, params: unknown) => void;

  registerMethod(method: string, handler: MethodHandler): void {
    this.handlers.set(method, handler);
  }

  async start(): Promise<void> {
    this.running = true;
    const rl = createInterface({
      input: process.stdin,
      crlfDelay: Infinity,
    });

    for await (const line of rl) {
      if (!this.running) break;
      if (line.trim() === '') continue;
      await this.handleLine(line);
    }
  }

  stop(): void {
    this.running = false;
  }

  sendResponse(id: RequestId, result: unknown): void {
    const response: JsonRpcResponse = {
      jsonrpc: '2.0',
      id,
      result: result ?? null,
    };
    this.writeLine(response);
  }

  sendError(id: RequestId | null, error: JsonRpcError): void {
    const response: JsonRpcErrorResponse = {
      jsonrpc: '2.0',
      id,
      error,
    };
    this.writeLine(response);
  }

  sendNotification(method: string, params?: unknown): void {
    const notification: JsonRpcNotification = {
      jsonrpc: '2.0',
      method,
      ...(params !== undefined && { params }),
    };
    this.writeLine(notification);
  }

  private async handleLine(line: string): Promise<void> {
    let parsed: unknown;
    try {
      parsed = JSON.parse(line);
    } catch {
      this.sendError(null, {
        code: PARSE_ERROR,
        message: 'invalid JSON',
      });
      return;
    }

    const msg = parsed as Record<string, unknown>;

    if (msg.jsonrpc !== '2.0') {
      this.sendError(null, {
        code: INVALID_REQUEST,
        message: 'missing or invalid jsonrpc version',
      });
      return;
    }

    // Notification — defined by the absence of the `id` field (JSON-RPC 2.0 spec)
    if (!('id' in msg)) {
      this.onNotification?.(msg.method as string, msg.params);
      return;
    }

    // Request (has id)
    const id = msg.id as RequestId;
    const method = msg.method as string;

    if (!method || typeof method !== 'string') {
      this.sendError(id, {
        code: INVALID_REQUEST,
        message: 'missing or invalid method',
      });
      return;
    }

    const handler = this.handlers.get(method);
    if (!handler) {
      this.sendError(id, {
        code: METHOD_NOT_FOUND,
        message: `method not found: ${method}`,
      });
      return;
    }

    try {
      const result = await handler(msg.params, id);
      this.sendResponse(id, result);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      const code =
        err instanceof Error && 'code' in err && typeof (err as { code: unknown }).code === 'number'
          ? (err as { code: number }).code
          : -32603; // INTERNAL_ERROR
      this.sendError(id, {
        code,
        message,
      });
    }
  }

  private writeLine(data: unknown): void {
    process.stdout.write(JSON.stringify(data) + '\n');
  }
}
