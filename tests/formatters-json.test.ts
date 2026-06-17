import { describe, expect, it } from 'bun:test';
import { SCHEMA_VERSION, toJsonOutput, toProviderOutput } from '../src/formatters/json';
import type { AllQuotas, ProviderQuota } from '../src/providers/types';

const claude: ProviderQuota = {
  provider: 'claude',
  displayName: 'Claude',
  available: true,
  plan: 'Max',
  primary: { remaining: 30, used: 70, resetsAt: '2026-06-17T20:00:00Z', windowMinutes: 300 },
  secondary: { remaining: 65, resetsAt: '2026-06-19T22:00:00Z' },
  models: { Sonnet: { remaining: 89, resetsAt: '2026-06-19T22:00:00Z' } },
  extra: { weeklyModels: { Sonnet: { remaining: 89, resetsAt: '2026-06-19T22:00:00Z' } } },
};

const ampError: ProviderQuota = {
  provider: 'amp',
  displayName: 'Amp',
  available: false,
  error: 'Not logged in.',
};

const allQuotas: AllQuotas = { providers: [claude, ampError], fetchedAt: '2026-06-17T19:00:00.000Z' };

describe('json formatter', () => {
  it('wraps providers in a versioned envelope', () => {
    const out = toJsonOutput(allQuotas);
    expect(SCHEMA_VERSION).toBe(1);
    expect(out.schemaVersion).toBe(SCHEMA_VERSION);
    expect(out.fetchedAt).toBe('2026-06-17T19:00:00.000Z');
    expect(out.providers).toHaveLength(2);
  });

  it('maps primary/secondary/models/used and keeps extra', () => {
    const p = toProviderOutput(claude);
    expect(p.primary).toEqual({ remaining: 30, used: 70, resetsAt: '2026-06-17T20:00:00Z', windowMinutes: 300 });
    expect(p.secondary).toEqual({ remaining: 65, resetsAt: '2026-06-19T22:00:00Z' });
    expect(p.models?.Sonnet.remaining).toBe(89);
    expect(p.extra?.weeklyModels).toBeDefined();
  });

  it('omits absent optional fields (no null, no undefined key) on an available provider', () => {
    const p = toProviderOutput(claude);
    expect('error' in p).toBe(false);
    expect('account' in p).toBe(false);
  });

  it('includes error and omits primary on a failed provider', () => {
    const p = toProviderOutput(ampError);
    expect(p.available).toBe(false);
    expect(p.error).toBe('Not logged in.');
    expect('primary' in p).toBe(false);
  });

  it('never contains Pango markup', () => {
    expect(JSON.stringify(toJsonOutput(allQuotas))).not.toContain('<span');
  });
});
