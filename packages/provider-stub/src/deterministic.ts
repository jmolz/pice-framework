/**
 * PICE_STUB_SCORES parser for deterministic adaptive evaluation traces.
 *
 * Format: `score,cost;score,cost;...` — e.g. `9.5,0.02;9.1,0.02;3.0,0.02`.
 * Each entry maps to a passIndex (0-indexed). When passIndex exceeds the
 * list, the last entry is repeated (avoids OOB in longer runs).
 */

export interface StubScoreEntry {
  score: number;
  cost: number;
}

export function parseStubScores(raw: string): StubScoreEntry[] {
  if (!raw.trim()) return [];
  return raw
    .trim()
    .split(';')
    .filter((s) => s.length > 0)
    .map((entry, i) => {
      const parts = entry.split(',');
      if (parts.length !== 2) {
        throw new Error(
          `PICE_STUB_SCORES entry ${i} must be "score,cost"; got "${entry}"`,
        );
      }
      const score = Number(parts[0]);
      const cost = Number(parts[1]);
      if (!Number.isFinite(score) || !Number.isFinite(cost)) {
        throw new Error(
          `PICE_STUB_SCORES entry ${i} has non-finite values: score=${parts[0]}, cost=${parts[1]}`,
        );
      }
      return { score, cost };
    });
}

export function getStubEntry(
  entries: StubScoreEntry[],
  passIndex: number,
): StubScoreEntry | undefined {
  if (entries.length === 0) return undefined;
  const idx = Math.min(passIndex, entries.length - 1);
  return entries[idx];
}
