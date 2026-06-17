import { describe, expect, it } from 'bun:test';
import { levelFor, type ProviderNotifyState, planNotifications } from '../src/notify';
import type { AllQuotas, ProviderQuota } from '../src/providers/types';

function wrap(...providers: ProviderQuota[]): AllQuotas {
  return { providers, fetchedAt: '2026-06-17T00:00:00.000Z' };
}

function claude(primaryRemaining: number, extra?: Partial<ProviderQuota>): ProviderQuota {
  return {
    provider: 'claude',
    displayName: 'Claude',
    available: true,
    primary: { remaining: primaryRemaining, resetsAt: null },
    ...extra,
  };
}

describe('levelFor', () => {
  it('classifies by used percent', () => {
    expect(levelFor(0)).toBe('ok');
    expect(levelFor(89)).toBe('ok');
    expect(levelFor(90)).toBe('low');
    expect(levelFor(94)).toBe('low');
    expect(levelFor(95)).toBe('critical');
    expect(levelFor(100)).toBe('critical');
    expect(levelFor(232)).toBe('critical');
  });
});

describe('planNotifications', () => {
  it('fires low when a window first crosses 90% used', () => {
    const plan = planNotifications(wrap(claude(8)), {}); // 92% used
    expect(plan.fires).toHaveLength(1);
    expect(plan.fires[0]).toMatchObject({ provider: 'claude', level: 'low', label: 'primary' });
    expect(plan.nextStates.claude.windows.primary).toBe('low');
    expect(plan.changed.has('claude')).toBe(true);
  });

  it('does not re-fire when already at the same level (dedup)', () => {
    const prev: Record<string, ProviderNotifyState> = { claude: { windows: { primary: 'low' } } };
    const plan = planNotifications(wrap(claude(8)), prev);
    expect(plan.fires).toHaveLength(0);
    expect(plan.changed.has('claude')).toBe(false);
  });

  it('escalates low → critical and fires again', () => {
    const prev: Record<string, ProviderNotifyState> = { claude: { windows: { primary: 'low' } } };
    const plan = planNotifications(wrap(claude(3)), prev); // 97% used
    expect(plan.fires).toHaveLength(1);
    expect(plan.fires[0].level).toBe('critical');
    expect(plan.nextStates.claude.windows.primary).toBe('critical');
  });

  it('re-arms on recovery without firing', () => {
    const prev: Record<string, ProviderNotifyState> = { claude: { windows: { primary: 'low' } } };
    const plan = planNotifications(wrap(claude(80)), prev); // 20% used → ok
    expect(plan.fires).toHaveLength(0);
    expect(plan.nextStates.claude.windows.primary).toBe('ok');
    expect(plan.changed.has('claude')).toBe(true);
  });

  it('fires for any window — a low secondary while primary is fine', () => {
    const p = claude(50, { secondary: { remaining: 4, resetsAt: null } });
    const plan = planNotifications(wrap(p), {});
    expect(plan.fires).toHaveLength(1);
    expect(plan.fires[0]).toMatchObject({ label: 'secondary', level: 'critical' });
  });

  it('fires for a model window', () => {
    const p = claude(50, { models: { Sonnet: { remaining: 9, resetsAt: null } } });
    const plan = planNotifications(wrap(p), {});
    expect(plan.fires).toHaveLength(1);
    expect(plan.fires[0]).toMatchObject({ label: 'Sonnet', level: 'low' });
  });

  it('honors a provider-supplied used percent over 100', () => {
    const p = claude(50, { primary: { remaining: 0, used: 232, resetsAt: null } });
    const plan = planNotifications(wrap(p), {});
    expect(plan.fires[0]).toMatchObject({ label: 'primary', level: 'critical', used: 232 });
  });

  it('skips unavailable providers', () => {
    const p: ProviderQuota = { provider: 'amp', displayName: 'Amp', available: false, error: 'x' };
    const plan = planNotifications(wrap(p), {});
    expect(plan.fires).toHaveLength(0);
    expect(plan.nextStates.amp).toBeUndefined();
  });
});
