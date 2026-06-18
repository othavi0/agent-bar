import { describe, expect, it } from 'bun:test';
import { etaLabel, toDisplay, toHealth } from '../src/formatters/shared';
import { formatForTerminal } from '../src/formatters/terminal';
import { formatForWaybar, formatProviderForWaybar } from '../src/formatters/waybar';
import type { AllQuotas, AmpQuota, ClaudeQuota, CodexQuota, ProviderQuota } from '../src/providers/types';
import { ANSI, BOX, ONE_DARK } from '../src/theme';

function mockClaudeQuota(remaining: number): ClaudeQuota {
  return {
    provider: 'claude',
    displayName: 'Claude',
    available: true,
    primary: {
      remaining,
      limit: 100,
      used: 100 - remaining,
      windowMinutes: 300,
      resetsAt: new Date(Date.now() + 3600000).toISOString(),
    },
  };
}

function mockCodexQuota(remaining: number): CodexQuota {
  return {
    provider: 'codex',
    displayName: 'Codex',
    available: true,
    primary: {
      remaining,
      limit: 100,
      used: 100 - remaining,
      windowMinutes: 300,
      resetsAt: new Date(Date.now() + 3600000).toISOString(),
    },
  };
}

function mockAmpQuota(): AmpQuota {
  return {
    provider: 'amp',
    displayName: 'Amp',
    available: true,
    models: {
      'Free Tier': {
        remaining: 75,
        limit: 100,
        used: 25,
        windowMinutes: 1440,
        resetsAt: new Date(Date.now() + 7200000).toISOString(),
      },
    },
  };
}

function mockAllQuotas(providers: ProviderQuota[]): AllQuotas {
  return {
    providers,
    fetchedAt: new Date().toISOString(),
  };
}

describe('formatForTerminal', () => {
  it("returns 'No providers connected' when empty", () => {
    const result = formatForTerminal({ providers: [], fetchedAt: new Date().toISOString() });
    expect(result).toContain('No providers connected');
  });

  it('renders Claude section with box-drawing chars', () => {
    const quotas = mockAllQuotas([mockClaudeQuota(80)]);
    const result = formatForTerminal(quotas);

    expect(result).toContain('Claude');
    expect(result).toContain(BOX.tl);
    expect(result).toContain(BOX.bl);
    expect(result).toContain('█');
  });

  it('renders Codex section', () => {
    const quotas = mockAllQuotas([mockCodexQuota(45)]);
    const result = formatForTerminal(quotas);

    expect(result).toContain('Codex');
    expect(result).toContain(BOX.tl);
  });

  it('renders Amp section', () => {
    const quotas = mockAllQuotas([mockAmpQuota()]);
    const result = formatForTerminal(quotas);

    expect(result).toContain('Amp');
    expect(result).toContain('Free Tier');
  });

  it('renders multiple providers separated by double newline', () => {
    const quotas = mockAllQuotas([mockClaudeQuota(80), mockCodexQuota(45)]);
    const result = formatForTerminal(quotas);

    expect(result).toContain('Claude');
    expect(result).toContain('Codex');
    expect(result).toContain('\n\n');
  });

  it('shows error message for provider with error', () => {
    const quota: ProviderQuota = {
      provider: 'claude',
      available: true,
      error: 'Token expired',
    };
    const result = formatForTerminal(mockAllQuotas([quota]));

    expect(result).toContain('Token expired');
  });

  it('skips unavailable providers without errors', () => {
    const quota: ProviderQuota = {
      provider: 'claude',
      available: false,
    };
    const result = formatForTerminal(mockAllQuotas([quota]));

    expect(result).toContain('No providers connected');
  });

  it('uses ANSI color codes in output', () => {
    const quotas = mockAllQuotas([mockClaudeQuota(80)]);
    const result = formatForTerminal(quotas);

    if (process.env.NO_COLOR) {
      expect(result).not.toContain('\x1b[');
      expect(ANSI.reset).toBe('');
    } else {
      expect(result).toContain('\x1b[');
      expect(result).toContain(ANSI.reset);
    }
  });
});

describe('formatForWaybar', () => {
  it('returns WaybarOutput shape', () => {
    const quotas = mockAllQuotas([mockClaudeQuota(80)]);
    const result = formatForWaybar(quotas);

    expect(result).toHaveProperty('text');
    expect(result).toHaveProperty('tooltip');
    expect(result).toHaveProperty('class');
  });

  it('uses Pango markup in tooltip', () => {
    const quotas = mockAllQuotas([mockClaudeQuota(80)]);
    const result = formatForWaybar(quotas);

    expect(result.tooltip).toContain('<span');
    expect(result.tooltip).toContain('foreground=');
    expect(result.tooltip).toContain('</span>');
  });

  it('uses hex colors (not ANSI) in tooltip', () => {
    const quotas = mockAllQuotas([mockClaudeQuota(80)]);
    const result = formatForWaybar(quotas);

    // Should contain hex colors like #d19a66
    expect(result.tooltip).toMatch(/#[0-9a-f]{6}/i);
    // Should NOT contain ANSI escape sequences
    expect(result.tooltip).not.toContain('\x1b[');
  });

  it('includes box-drawing chars in tooltip', () => {
    const quotas = mockAllQuotas([mockClaudeQuota(80)]);
    const result = formatForWaybar(quotas);

    expect(result.tooltip).toContain(BOX.tl);
    expect(result.tooltip).toContain(BOX.bl);
    expect(result.tooltip).toContain(BOX.v);
  });

  it('sets class with provider status', () => {
    const quotas = mockAllQuotas([mockClaudeQuota(80)]);
    const result = formatForWaybar(quotas);

    expect(result.class).toContain('agent-bar');
    expect(result.class).toContain('claude-ok');
  });

  it('sets critical class for very low quota', () => {
    const quotas = mockAllQuotas([mockClaudeQuota(5)]);
    const result = formatForWaybar(quotas);

    expect(result.class).toContain('claude-critical');
  });

  it('sets warn class for low quota', () => {
    const quotas = mockAllQuotas([mockClaudeQuota(20)]);
    const result = formatForWaybar(quotas);

    expect(result.class).toContain('claude-warn');
  });

  it("shows 'No Providers' when empty", () => {
    const quotas = mockAllQuotas([]);
    const result = formatForWaybar(quotas);

    expect(result.text).toContain('No Providers');
  });
});

describe('formatProviderForWaybar', () => {
  it('returns disconnected state for unavailable provider', () => {
    const quota: ProviderQuota = {
      provider: 'claude',
      available: false,
      error: 'No credentials',
    };
    const result = formatProviderForWaybar(quota);

    expect(result.class).toContain('disconnected');
    expect(result.text).toContain(ONE_DARK.red);
  });

  it('returns percentage for available provider', () => {
    const result = formatProviderForWaybar(mockClaudeQuota(80));

    expect(result.class).toContain('agent-bar-claude');
    expect(result.tooltip).toContain('Claude');
  });

  it('emits alt (health state) and displayMode-aware percentage', () => {
    const out = formatProviderForWaybar(mockClaudeQuota(70), 'remaining');
    expect(out.alt).toBe('ok'); // 70 >= 60
    expect(out.percentage).toBe(70);

    const used = formatProviderForWaybar(mockClaudeQuota(70), 'used');
    expect(used.percentage).toBe(30); // mirrors text in used mode
    expect(used.alt).toBe('ok'); // health from raw remaining, not display value
  });

  it('maps health buckets to alt', () => {
    expect(formatProviderForWaybar(mockClaudeQuota(50)).alt).toBe('low'); // 30..59
    expect(formatProviderForWaybar(mockClaudeQuota(20)).alt).toBe('warn'); // 10..29
    expect(formatProviderForWaybar(mockClaudeQuota(5)).alt).toBe('critical'); // < 10
  });

  it('marks disconnected and omits percentage', () => {
    const quota: ProviderQuota = { provider: 'claude', available: false, error: 'x' } as ProviderQuota;
    const out = formatProviderForWaybar(quota);
    expect(out.alt).toBe('disconnected');
    expect('percentage' in out).toBe(false);
  });

  it('aggregate output (formatForWaybar) emits no alt or percentage', () => {
    const result = formatForWaybar(mockAllQuotas([mockClaudeQuota(80)]));
    expect('alt' in result).toBe(false);
    expect('percentage' in result).toBe(false);
  });

  it('omits alt and percentage when available but has no window data', () => {
    const quota: ProviderQuota = { provider: 'claude', displayName: 'Claude', available: true } as ProviderQuota;
    const out = formatProviderForWaybar(quota);
    expect('alt' in out).toBe(false);
    expect('percentage' in out).toBe(false);
  });

  it('clamps percentage to 100 on overage (used > 100)', () => {
    const quota: ProviderQuota = {
      provider: 'codex',
      displayName: 'Codex',
      available: true,
      primary: { remaining: -7, used: 107, resetsAt: null },
    } as ProviderQuota;
    const out = formatProviderForWaybar(quota, 'used');
    expect(out.percentage).toBe(100);
  });
});

describe('formatForTerminal displayMode=used', () => {
  it('shows used percentage (100 - remaining)', () => {
    const quotas = mockAllQuotas([mockClaudeQuota(80)]); // 20% used
    const result = formatForTerminal(quotas, 'used');
    expect(result).toContain('20%');
    expect(result).not.toMatch(/\b80%\b/);
  });

  it('colors used=95 as red (low health) not green', () => {
    const quotas = mockAllQuotas([mockClaudeQuota(5)]); // 5% remaining = 95% used → health=5 → red
    const result = formatForTerminal(quotas, 'used');
    if (!process.env.NO_COLOR) {
      expect(result).toContain(ANSI.red);
    }
  });

  it('default arg keeps remaining behavior', () => {
    const quotas = mockAllQuotas([mockClaudeQuota(80)]);
    const result = formatForTerminal(quotas);
    expect(result).toContain('80%');
  });
});

describe('formatForWaybar displayMode=used', () => {
  it('text shows used (100 - remaining)', () => {
    const result = formatForWaybar(mockAllQuotas([mockClaudeQuota(80)]), 'used');
    expect(result.text).toContain('20%');
  });

  it('class still uses health thresholds', () => {
    const result = formatForWaybar(mockAllQuotas([mockClaudeQuota(5)]), 'used');
    expect(result.class).toContain('claude-critical');
  });

  it('formatProviderForWaybar respects mode', () => {
    const result = formatProviderForWaybar(mockClaudeQuota(80), 'used');
    expect(result.text).toContain('20%');
  });

  it('default arg keeps remaining behavior', () => {
    const result = formatForWaybar(mockAllQuotas([mockClaudeQuota(80)]));
    expect(result.text).toContain('80%');
  });
});

describe('display mode helpers', () => {
  it('toDisplay: remaining passes through', () => {
    expect(toDisplay(80, 'remaining')).toBe(80);
    expect(toDisplay(0, 'remaining')).toBe(0);
    expect(toDisplay(null, 'remaining')).toBe(null);
  });

  it('toDisplay: used inverts', () => {
    expect(toDisplay(80, 'used')).toBe(20);
    expect(toDisplay(0, 'used')).toBe(100);
    expect(toDisplay(100, 'used')).toBe(0);
    expect(toDisplay(null, 'used')).toBe(null);
  });

  it('toHealth: roundtrips display value back to health', () => {
    expect(toHealth(20, 'used')).toBe(80);
    expect(toHealth(80, 'remaining')).toBe(80);
    expect(toHealth(null, 'used')).toBe(null);
  });

  it('etaLabel: "Full in" when remaining, "Resets in" when used', () => {
    expect(etaLabel('remaining')).toBe('Full in');
    expect(etaLabel('used')).toBe('Resets in');
  });
});
