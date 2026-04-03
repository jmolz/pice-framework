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
  private evalContracts = new Map<string, unknown>();

  getCapabilities(): ProviderCapabilities {
    return {
      workflow: true,
      evaluation: true,
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

    transport.registerMethod('evaluate/create', async (params: unknown) => {
      this.requireInitialized();
      const sessionId = `stub-eval-${nextSessionId++}`;
      // Store the contract so evaluate/score can return matching criterion names
      const { contract } = params as { contract?: { criteria?: Array<{ name: string; threshold: number }> } };
      this.evalContracts.set(sessionId, contract);
      return { sessionId };
    });

    transport.registerMethod('evaluate/score', async (params: unknown) => {
      this.requireInitialized();
      const { sessionId } = params as { sessionId: string };

      // Build scores matching the contract criteria names (if available)
      const contract = this.evalContracts.get(sessionId) as
        | { criteria?: Array<{ name: string; threshold: number }> }
        | undefined;
      const criteria = contract?.criteria ?? [];
      const scores = criteria.length > 0
        ? criteria.map((c: { name: string; threshold: number }) => ({
            name: c.name,
            score: 8,
            threshold: c.threshold,
            passed: true,
            findings: 'Stub evaluation — criterion passes by default',
          }))
        : [{ name: 'stub-criterion', score: 8, threshold: 7, passed: true, findings: 'Stub evaluation' }];

      transport.sendNotification('evaluate/result', {
        sessionId,
        scores,
        passed: true,
        summary: 'Stub evaluation complete',
      });

      this.evalContracts.delete(sessionId);
      return { ok: true };
    });
  }
}
