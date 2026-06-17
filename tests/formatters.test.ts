import { describe, expect, it } from 'bun:test';
import { etaLabel, toDisplay, toHealth } from '../src/formatters/shared';
import { formatForTerminal } from '../src/formatters/terminal';
import { formatForWaybar, formatProviderForWaybar } from '../src/formatters/waybar';
import type { AllQuotas, AmpQuota, ClaudeQuota, CodexQuota, CopilotQuota, ProviderQuota } from '../src/providers/types';
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

function mockCopilotQuota(): CopilotQuota {
  return {
    provider: 'copilot',
    displayName: 'Copilot',
    available: true,
    primary: {
      remaining: 0,
      resetsAt: new Date(Date.now() + 3600000).toISOString(),
      used: (698 / 300) * 100,
    },
    models: {
      'Premium requests': {
        remaining: 0,
        resetsAt: new Date(Date.now() + 3600000).toISOString(),
        used: (698 / 300) * 100,
      },
      Chat: {
        remaining: 100,
        resetsAt: null,
      },
    },
    extra: {
      quotaSnapshots: {
        premium_interactions: {
          isUnlimitedEntitlement: false,
          entitlementRequests: 300,
          usedRequests: 698,
          usageAllowedWithExhaustedQuota: true,
          overage: 0,
          overageAllowedWithExhaustedQuota: true,
          remainingPercentage: -132.8,
          resetDate: new Date(Date.now() + 3600000).toISOString(),
        },
        chat: {
          isUnlimitedEntitlement: true,
          entitlementRequests: 0,
          usedRequests: 0,
          usageAllowedWithExhaustedQuota: false,
          overage: 0,
          overageAllowedWithExhaustedQuota: false,
          remainingPercentage: 100,
          resetDate: null,
        },
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

  it('renders Copilot section', () => {
    const quotas = mockAllQuotas([mockCopilotQuota()]);
    const result = formatForTerminal(quotas);

    expect(result).toContain('Copilot');
    expect(result).toContain('Premium requests');
    expect(result).toContain('698 / 300 used');
  });

  it('renders Copilot over-quota usage above 100% in used mode', () => {
    const quotas = mockAllQuotas([mockCopilotQuota()]);
    const result = formatForTerminal(quotas, 'used');

    expect(result).toContain('233%');
    expect(result).toContain('698 / 300 used');
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

  it('sets critical class for exhausted Copilot quota', () => {
    const quotas = mockAllQuotas([mockCopilotQuota()]);
    const result = formatForWaybar(quotas);

    expect(result.class).toContain('copilot-critical');
    expect(result.tooltip).toContain('raw -132.8%');
  });

  it('shows Copilot over-quota usage above 100% in used mode', () => {
    const quotas = mockAllQuotas([mockCopilotQuota()]);
    const result = formatForWaybar(quotas, 'used');

    expect(result.text).toContain('233%');
    expect(result.tooltip).toContain('233%');
    expect(result.class).toContain('copilot-critical');
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

  it('returns Copilot percentage and critical state for exhausted quota', () => {
    const result = formatProviderForWaybar(mockCopilotQuota());

    expect(result.class).toContain('agent-bar-copilot');
    expect(result.class).toContain('critical');
    expect(result.text).toContain('0%');
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

  it('formatProviderForWaybar shows Copilot over-quota raw usage in used mode', () => {
    const result = formatProviderForWaybar(mockCopilotQuota(), 'used');
    expect(result.text).toContain('233%');
    expect(result.class).toContain('critical');
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
