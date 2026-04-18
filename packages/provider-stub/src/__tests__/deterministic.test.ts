import { describe, it, expect } from 'vitest';
import { parseStubScores, getStubEntry } from '../deterministic.js';

describe('parseStubScores', () => {
  it('parses a well-formed env var', () => {
    const entries = parseStubScores('9.5,0.02;3.0,0.03;8.0,0.01');
    expect(entries).toEqual([
      { score: 9.5, cost: 0.02 },
      { score: 3.0, cost: 0.03 },
      { score: 8.0, cost: 0.01 },
    ]);
  });

  it('returns empty for empty string', () => {
    expect(parseStubScores('')).toEqual([]);
    expect(parseStubScores('  ')).toEqual([]);
  });

  it('handles a single entry', () => {
    const entries = parseStubScores('9.0,0.01');
    expect(entries).toEqual([{ score: 9.0, cost: 0.01 }]);
  });

  it('throws on malformed entry (missing cost)', () => {
    expect(() => parseStubScores('9.0')).toThrow('must be "score,cost"');
  });

  it('throws on non-finite value', () => {
    expect(() => parseStubScores('NaN,0.01')).toThrow('non-finite');
    expect(() => parseStubScores('9.0,Infinity')).toThrow('non-finite');
  });

  it('tolerates trailing semicolons', () => {
    const entries = parseStubScores('9.0,0.01;');
    expect(entries).toEqual([{ score: 9.0, cost: 0.01 }]);
  });
});

describe('getStubEntry', () => {
  const entries = parseStubScores('9.5,0.02;3.0,0.03;8.0,0.01');

  it('returns the entry at the given passIndex', () => {
    expect(getStubEntry(entries, 0)).toEqual({ score: 9.5, cost: 0.02 });
    expect(getStubEntry(entries, 1)).toEqual({ score: 3.0, cost: 0.03 });
    expect(getStubEntry(entries, 2)).toEqual({ score: 8.0, cost: 0.01 });
  });

  it('clamps to last entry when passIndex exceeds length', () => {
    expect(getStubEntry(entries, 99)).toEqual({ score: 8.0, cost: 0.01 });
  });

  it('returns undefined for empty entries', () => {
    expect(getStubEntry([], 0)).toBeUndefined();
  });
});
