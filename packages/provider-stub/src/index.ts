import type { ProviderCapabilities, EvaluateCreateParams } from '@pice/provider-protocol';
import { BaseProvider, StdioTransport } from '@pice/provider-base';
import { parseStubScores, getStubEntry, type StubScoreEntry } from './deterministic.js';

let nextSessionId = 1;

/**
 * Stub/echo provider for testing the PICE protocol.
 *
 * - Responds to `session/create` with a fake session ID
 * - Responds to `session/send` by echoing the message back as
 *   `response/chunk` notifications followed by `response/complete`
 * - Declares no real capabilities (workflow: false, evaluation: false)
 */
/**
 * Per-session state for stub evaluations. The score used at `evaluate/score`
 * time depends on the pass index declared at `evaluate/create` time — so we
 * resolve the `PICE_STUB_SCORES` entry once at create and stash it here.
 */
interface StubEvalState {
  contract: unknown;
  /** 0-indexed pass position from `evaluate/create` params (defaults to 0). */
  passIndex: number;
  /**
   * Pre-resolved stub entry for this pass. `undefined` when `PICE_STUB_SCORES`
   * is unset — `evaluate/score` then falls back to `defaultScore = 8`.
   */
  stubEntry?: StubScoreEntry;
}

export class StubProvider extends BaseProvider {
  private evalContracts = new Map<string, StubEvalState>();
  private stubScores: StubScoreEntry[];

  constructor(version: string) {
    super(version);
    const raw = process.env['PICE_STUB_SCORES'];
    this.stubScores = raw ? parseStubScores(raw) : [];
  }

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
      const p = params as EvaluateCreateParams;

      const passIndex = p.passIndex ?? 0;
      const entry = getStubEntry(this.stubScores, passIndex);

      this.evalContracts.set(sessionId, {
        contract: p.contract,
        passIndex,
        stubEntry: entry,
      });

      return {
        sessionId,
        ...(entry ? { costUsd: entry.cost, confidence: entry.score / 10.0 } : {}),
      };
    });

    transport.registerMethod('evaluate/score', async (params: unknown) => {
      this.requireInitialized();
      const { sessionId } = params as { sessionId: string };

      // Default score if `PICE_STUB_SCORES` is not configured. Kept at 8 for
      // backward compatibility with pre-Phase-4 tests; the Phase 4 adaptive
      // loop integration tests SHOULD set `PICE_STUB_SCORES` for determinism.
      const defaultScore = 8;
      const state = this.evalContracts.get(sessionId);
      // Use the per-pass stub score (rounded to nearest integer for the
      // 0–10 `CriterionScore.score` wire type) when set; else fall back.
      const rawScore = state?.stubEntry?.score ?? defaultScore;
      const passScore = Math.max(0, Math.min(10, Math.round(rawScore)));
      const contract = state?.contract as
        | { criteria?: Array<{ name: string; threshold: number }> }
        | undefined;
      const criteria = contract?.criteria ?? [];
      const scores = criteria.length > 0
        ? criteria.map((c: { name: string; threshold: number }) => ({
            name: c.name,
            score: passScore,
            threshold: c.threshold,
            passed: passScore >= c.threshold,
            findings: 'Stub evaluation — scored via PICE_STUB_SCORES or default',
          }))
        : [{
            name: 'stub-criterion',
            score: passScore,
            threshold: 7,
            passed: passScore >= 7,
            findings: 'Stub evaluation',
          }];

      transport.sendNotification('evaluate/result', {
        sessionId,
        scores,
        // `passed` reflects the effective pass score, not a hard-coded true.
        // Phase 4 tests that expect SPRT-rejected need this to swing false.
        passed: scores.every((s) => s.passed),
        summary: 'Stub evaluation complete',
      });

      this.evalContracts.delete(sessionId);
      return { ok: true };
    });
  }
}
