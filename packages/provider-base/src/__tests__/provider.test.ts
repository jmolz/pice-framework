import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import type { ProviderCapabilities } from '@pice/provider-protocol';
import { BaseProvider } from '../provider.js';
import { StdioTransport } from '../transport.js';

class TestProvider extends BaseProvider {
  getCapabilities(): ProviderCapabilities {
    return {
      workflow: false,
      evaluation: false,
      agentTeams: false,
      models: [],
    };
  }

  protected registerHandlers(transport: StdioTransport): void {
    transport.registerMethod('test/echo', async (params: unknown) => {
      this.requireInitialized(params, 0);
      return params;
    });
  }
}

describe('BaseProvider', () => {
  let provider: TestProvider;
  let stdoutWrites: string[];

  beforeEach(() => {
    stdoutWrites = [];
    vi.spyOn(process.stdout, 'write').mockImplementation((chunk: string | Uint8Array) => {
      stdoutWrites.push(chunk.toString());
      return true;
    });
    provider = new TestProvider('0.1.0');
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('is not initialized by default', () => {
    expect(provider['initialized']).toBe(false);
  });

  it('getCapabilities returns the declared capabilities', () => {
    const caps = provider.getCapabilities();
    expect(caps.workflow).toBe(false);
    expect(caps.evaluation).toBe(false);
  });

  it('transport is accessible on provider', () => {
    expect(provider['transport']).toBeInstanceOf(StdioTransport);
  });
});
