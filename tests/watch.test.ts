import { describe, expect, it } from 'bun:test';
import type { AllQuotas } from '../src/providers/types';
import { buildWatchLine } from '../src/watch';

const quotas: AllQuotas = {
  providers: [{ provider: 'claude', displayName: 'Claude', available: true, primary: { remaining: 50, resetsAt: null } }],
  fetchedAt: '2026-06-17T19:00:00.000Z',
};

describe('buildWatchLine', () => {
  it('produces one valid NDJSON line ending in newline', () => {
    const line = buildWatchLine(quotas);
    expect(line.endsWith('\n')).toBe(true);
    expect(line.includes('\n')).toBe(true);
    expect(line.trim().split('\n')).toHaveLength(1);
    const parsed = JSON.parse(line);
    expect(parsed.schemaVersion).toBe(1);
    expect(parsed.providers[0].provider).toBe('claude');
  });

  it('contains no Pango markup', () => {
    expect(buildWatchLine(quotas)).not.toContain('<span');
  });
});
