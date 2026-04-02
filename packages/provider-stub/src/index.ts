import type { ProviderCapabilities } from '@pice/provider-protocol';
import { BaseProvider, StdioTransport } from '@pice/provider-base';

let nextSessionId = 1;

/**
 * Stub/echo provider for testing the PICE protocol.
 *
 * - Responds to `session/create` with a fake session ID
 * - Responds to `session/send` by echoing the message back as
 *   `response/chunk` notifications followed by `response/complete`
 * - Declares no real capabilities (workflow: false, evaluation: false)
 */
export class StubProvider extends BaseProvider {
  getCapabilities(): ProviderCapabilities {
    return {
      workflow: false,
      evaluation: false,
      agentTeams: false,
      models: ['stub-echo'],
    };
  }

  protected registerHandlers(transport: StdioTransport): void {
    transport.registerMethod('session/create', async (_params: unknown) => {
      this.requireInitialized();
      const sessionId = `stub-session-${nextSessionId++}`;
      return { sessionId };
    });

    transport.registerMethod('session/send', async (params: unknown) => {
      this.requireInitialized();
      const { sessionId, message } = params as { sessionId: string; message: string };

      // Echo the message back as a chunk notification
      transport.sendNotification('response/chunk', {
        sessionId,
        text: message,
      });

      // Send completion
      transport.sendNotification('response/complete', {
        sessionId,
        result: { echo: message },
      });

      return { ok: true };
    });

    transport.registerMethod('session/destroy', async (_params: unknown) => {
      this.requireInitialized();
      return null;
    });
  }
}
