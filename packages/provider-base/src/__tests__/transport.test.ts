import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { StdioTransport } from '../transport.js';

describe('StdioTransport', () => {
  let transport: StdioTransport;
  let stdoutWrites: string[];

  beforeEach(() => {
    transport = new StdioTransport();
    stdoutWrites = [];
    vi.spyOn(process.stdout, 'write').mockImplementation((chunk: string | Uint8Array) => {
      stdoutWrites.push(chunk.toString());
      return true;
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('sendResponse writes valid JSON-RPC to stdout', () => {
    transport.sendResponse(1, { sessionId: 'abc' });
    expect(stdoutWrites).toHaveLength(1);
    const parsed = JSON.parse(stdoutWrites[0]!.trim());
    expect(parsed).toEqual({
      jsonrpc: '2.0',
      id: 1,
      result: { sessionId: 'abc' },
    });
  });

  it('sendResponse with null result', () => {
    transport.sendResponse(1, null);
    const parsed = JSON.parse(stdoutWrites[0]!.trim());
    expect(parsed.result).toBeNull();
  });

  it('sendError writes valid JSON-RPC error to stdout', () => {
    transport.sendError(1, { code: -32601, message: 'method not found: foo' });
    const parsed = JSON.parse(stdoutWrites[0]!.trim());
    expect(parsed).toEqual({
      jsonrpc: '2.0',
      id: 1,
      error: { code: -32601, message: 'method not found: foo' },
    });
  });

  it('sendError with null id for parse errors', () => {
    transport.sendError(null, { code: -32700, message: 'invalid JSON' });
    const parsed = JSON.parse(stdoutWrites[0]!.trim());
    expect(parsed.id).toBeNull();
  });

  it('sendNotification writes JSON-RPC notification without id', () => {
    transport.sendNotification('response/chunk', { sessionId: 's1', text: 'hello' });
    const parsed = JSON.parse(stdoutWrites[0]!.trim());
    expect(parsed.jsonrpc).toBe('2.0');
    expect(parsed.method).toBe('response/chunk');
    expect(parsed.params).toEqual({ sessionId: 's1', text: 'hello' });
    expect(parsed).not.toHaveProperty('id');
  });

  it('sendNotification without params omits params field', () => {
    transport.sendNotification('metrics/event');
    const parsed = JSON.parse(stdoutWrites[0]!.trim());
    expect(parsed).not.toHaveProperty('params');
  });

  it('registerMethod stores handler for dispatch', () => {
    const handler = vi.fn().mockResolvedValue({ ok: true });
    transport.registerMethod('test/method', handler);
    // Handler is registered but not called yet (dispatch happens via stdin)
    expect(handler).not.toHaveBeenCalled();
  });

  it('preserves custom error code from thrown Error', async () => {
    const err = Object.assign(new Error('provider not initialized'), { code: -32000 });
    transport.registerMethod('test/guarded', async () => {
      throw err;
    });
    // Simulate handling a request line
    await (transport as unknown as { handleLine(line: string): Promise<void> }).handleLine(
      JSON.stringify({ jsonrpc: '2.0', id: 99, method: 'test/guarded', params: {} }),
    );
    expect(stdoutWrites).toHaveLength(1);
    const parsed = JSON.parse(stdoutWrites[0]!.trim());
    expect(parsed.error.code).toBe(-32000);
    expect(parsed.error.message).toBe('provider not initialized');
  });

  it('falls back to INTERNAL_ERROR code for plain errors', async () => {
    transport.registerMethod('test/plain', async () => {
      throw new Error('something broke');
    });
    await (transport as unknown as { handleLine(line: string): Promise<void> }).handleLine(
      JSON.stringify({ jsonrpc: '2.0', id: 100, method: 'test/plain', params: {} }),
    );
    const parsed = JSON.parse(stdoutWrites[0]!.trim());
    expect(parsed.error.code).toBe(-32603);
  });

  it('all output lines are valid JSON', () => {
    transport.sendResponse(1, 'result');
    transport.sendError(2, { code: -32603, message: 'err' });
    transport.sendNotification('test', { data: true });

    for (const line of stdoutWrites) {
      expect(() => JSON.parse(line.trim())).not.toThrow();
    }
  });

  it('each output line ends with newline', () => {
    transport.sendResponse(1, {});
    for (const line of stdoutWrites) {
      expect(line.endsWith('\n')).toBe(true);
    }
  });
});
