import { describe, expect, it } from 'bun:test';
import { formatForTerminal } from '../src/formatters/terminal';
import { formatForWaybar, formatProviderForWaybar } from '../src/formatters/waybar';
import type { AllQuotas, AmpQuota, ClaudeQuota, CodexQuota, CopilotQuota, ProviderQuota } from '../src/providers/types';

// ---------------------------------------------------------------------------
// Sanitize dynamic values so snapshots remain stable across runs.
// ---------------------------------------------------------------------------

const ISO_RE = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z/g;
const TIME_HM_RE = /\d{1,2}h \d{2}m/g;
const TIME_DH_RE = /\d+d \d{2}h/g;
const PAREN_TIME_RE = /\(\d{2}:\d{2}\)/g;
const AGO_RE = /\d+[hm] ago/g;
const JUST_NOW_RE = /just now/g;
// Strip ANSI escape codes so snapshots are color-agnostic and deterministic.
// biome-ignore lint/suspicious/noControlCharactersInRegex: the ESC byte is the point — intentional ANSI stripping
const ANSI_RE = /\x1b\[[0-9;]*m/g;

function sanitize(s: string): string {
  return s
    .replace(ANSI_RE, '')
    .replace(ISO_RE, '__ISO__')
    .replace(TIME_DH_RE, '__DH__')
    .replace(TIME_HM_RE, '__HM__')
    .replace(PAREN_TIME_RE, '(__:__)')
    .replace(AGO_RE, '__AGO__')
    .replace(JUST_NOW_RE, '__AGO__');
}

// ---------------------------------------------------------------------------
// Stable timestamps used across all mock data.
// ---------------------------------------------------------------------------

const FIXED_FETCHED_AT = '2026-03-28T12:00:00.000Z';
const FIXED_RESET = '2026-03-28T14:00:00.000Z';

// ---------------------------------------------------------------------------
// Mock data factories (deterministic)
// ---------------------------------------------------------------------------

function claudeHealthy(): ClaudeQuota {
  return {
    provider: 'claude',
    displayName: 'Claude',
    available: true,
    plan: 'Pro',
    primary: { remaining: 75, resetsAt: FIXED_RESET, windowMinutes: 300 },
    secondary: { remaining: 90, resetsAt: FIXED_RESET, windowMinutes: 10080 },
  };
}

function claudeError(): ClaudeQuota {
  return {
    provider: 'claude',
    displayName: 'Claude',
    available: false,
    error: 'Token expired. Open `agent-bar menu` and choose Provider login.',
  };
}

function codexHealthy(): CodexQuota {
  return {
    provider: 'codex',
    displayName: 'Codex',
    available: true,
    plan: 'Pro',
    planType: 'pro',
    primary: { remaining: 60, resetsAt: FIXED_RESET, windowMinutes: 300 },
    secondary: { remaining: 85, resetsAt: FIXED_RESET, windowMinutes: 10080 },
    extra: {
      modelsDetailed: {
        Codex: {
          fiveHour: { remaining: 60, resetsAt: FIXED_RESET, windowMinutes: 300 },
          sevenDay: { remaining: 85, resetsAt: FIXED_RESET, windowMinutes: 10080 },
        },
      },
    },
    models: {
      Codex: { remaining: 60, resetsAt: FIXED_RESET, windowMinutes: 300 },
    },
  };
}

function codexError(): CodexQuota {
  return {
    provider: 'codex',
    displayName: 'Codex',
    available: false,
    error: 'No session data found',
  };
}

function ampHealthy(): AmpQuota {
  return {
    provider: 'amp',
    displayName: 'Amp',
    available: true,
    primary: { remaining: 70, resetsAt: FIXED_RESET },
    models: {
      'Free Tier': { remaining: 70, resetsAt: FIXED_RESET },
    },
    extra: {
      meta: {
        freeRemaining: '$3.50',
        freeTotal: '$5.00',
        replenishRate: '+$0.25/hr',
      },
    },
  };
}

function ampError(): AmpQuota {
  return {
    provider: 'amp',
    displayName: 'Amp',
    available: false,
    error: 'Amp CLI not installed. Right-click to install and log in.',
  };
}

// ---------------------------------------------------------------------------
// C1: Factories with `account` field set
// ---------------------------------------------------------------------------

function ampWithAccount(): AmpQuota {
  return { ...ampHealthy(), account: 'user@example.com' };
}

function copilotWithAccount(): CopilotQuota {
  return {
    provider: 'copilot',
    displayName: 'Copilot',
    available: true,
    account: 'dev@example.com',
    primary: { remaining: 80, resetsAt: FIXED_RESET },
    models: {
      'Premium requests': { remaining: 80, resetsAt: FIXED_RESET },
    },
    extra: {
      quotaSnapshots: {
        premium_interactions: {
          isUnlimitedEntitlement: false,
          entitlementRequests: 300,
          usedRequests: 60,
          usageAllowedWithExhaustedQuota: false,
          overage: 0,
          overageAllowedWithExhaustedQuota: false,
          remainingPercentage: 80,
          resetDate: FIXED_RESET,
        },
      },
    },
  };
}

// ---------------------------------------------------------------------------
// C3: Rich factories for builder branch coverage
// ---------------------------------------------------------------------------

function claudeWithExtras(): ClaudeQuota {
  return {
    provider: 'claude',
    displayName: 'Claude',
    available: true,
    plan: 'Pro',
    primary: { remaining: 60, resetsAt: FIXED_RESET, windowMinutes: 300 },
    secondary: { remaining: 50, resetsAt: FIXED_RESET, windowMinutes: 10080 },
    extra: {
      weeklyModels: {
        'claude-opus-4-5': { remaining: 40, resetsAt: FIXED_RESET, windowMinutes: 10080 },
        'claude-sonnet-4-5': { remaining: 65, resetsAt: FIXED_RESET, windowMinutes: 10080 },
      },
      extraUsage: {
        enabled: true,
        remaining: 55,
        limit: 5000,
        used: 2250,
      },
    },
  };
}

function ampWithCredits(): AmpQuota {
  return {
    provider: 'amp',
    displayName: 'Amp',
    available: true,
    primary: { remaining: 30, resetsAt: FIXED_RESET },
    models: {
      'Free Tier': { remaining: 30, resetsAt: FIXED_RESET },
      Credits: { remaining: 75, resetsAt: FIXED_RESET },
    },
    extra: {
      meta: {
        freeRemaining: '$1.50',
        freeTotal: '$5.00',
        replenishRate: '+$0.25/hr',
        creditsBalance: '$7.50',
      },
    },
  };
}

function ampUnknownModels(): AmpQuota {
  return {
    provider: 'amp',
    displayName: 'Amp',
    available: true,
    primary: { remaining: 45, resetsAt: FIXED_RESET },
    models: {
      'Custom Plan A': { remaining: 45, resetsAt: FIXED_RESET },
      'Custom Plan B': { remaining: 80, resetsAt: FIXED_RESET },
    },
    extra: {
      meta: {},
    },
  };
}

function wrap(...providers: ProviderQuota[]): AllQuotas {
  return { providers, fetchedAt: FIXED_FETCHED_AT };
}

// ---------------------------------------------------------------------------
// Terminal formatter snapshots
// ---------------------------------------------------------------------------

describe('Terminal formatter snapshots', () => {
  it('renders Claude healthy', () => {
    const result = sanitize(formatForTerminal(wrap(claudeHealthy())));
    expect(result).toMatchSnapshot();
  });

  it('renders Claude error', () => {
    const result = sanitize(formatForTerminal(wrap(claudeError())));
    expect(result).toMatchSnapshot();
  });

  it('renders Codex healthy', () => {
    const result = sanitize(formatForTerminal(wrap(codexHealthy())));
    expect(result).toMatchSnapshot();
  });

  it('renders Codex error', () => {
    const result = sanitize(formatForTerminal(wrap(codexError())));
    expect(result).toMatchSnapshot();
  });

  it('renders Amp healthy', () => {
    const result = sanitize(formatForTerminal(wrap(ampHealthy())));
    expect(result).toMatchSnapshot();
  });

  it('renders Amp error', () => {
    const result = sanitize(formatForTerminal(wrap(ampError())));
    expect(result).toMatchSnapshot();
  });

  it('renders all providers combined', () => {
    const result = sanitize(formatForTerminal(wrap(claudeHealthy(), codexHealthy(), ampHealthy())));
    expect(result).toMatchSnapshot();
  });

  it('renders empty providers', () => {
    const result = sanitize(formatForTerminal(wrap()));
    expect(result).toMatchSnapshot();
  });
});

// ---------------------------------------------------------------------------
// Waybar formatter snapshots
// ---------------------------------------------------------------------------

describe('Waybar formatter snapshots', () => {
  it('renders Claude healthy', () => {
    const out = formatForWaybar(wrap(claudeHealthy()));
    expect(sanitize(out.text)).toMatchSnapshot();
    expect(sanitize(out.tooltip)).toMatchSnapshot();
    expect(out.class).toMatchSnapshot();
  });

  it('renders Claude error', () => {
    const out = formatForWaybar(wrap(claudeError()));
    expect(sanitize(out.text)).toMatchSnapshot();
    expect(sanitize(out.tooltip)).toMatchSnapshot();
    expect(out.class).toMatchSnapshot();
  });

  it('renders Codex healthy', () => {
    const out = formatForWaybar(wrap(codexHealthy()));
    expect(sanitize(out.text)).toMatchSnapshot();
    expect(sanitize(out.tooltip)).toMatchSnapshot();
    expect(out.class).toMatchSnapshot();
  });

  it('renders Amp healthy', () => {
    const out = formatForWaybar(wrap(ampHealthy()));
    expect(sanitize(out.text)).toMatchSnapshot();
    expect(sanitize(out.tooltip)).toMatchSnapshot();
    expect(out.class).toMatchSnapshot();
  });

  it('renders all providers combined', () => {
    const out = formatForWaybar(wrap(claudeHealthy(), codexHealthy(), ampHealthy()));
    expect(sanitize(out.text)).toMatchSnapshot();
    expect(sanitize(out.tooltip)).toMatchSnapshot();
    expect(out.class).toMatchSnapshot();
  });

  it('renders empty providers', () => {
    const out = formatForWaybar(wrap());
    expect(sanitize(out.text)).toMatchSnapshot();
    expect(sanitize(out.tooltip)).toMatchSnapshot();
    expect(out.class).toMatchSnapshot();
  });
});

// ---------------------------------------------------------------------------
// Per-provider waybar snapshots
// ---------------------------------------------------------------------------

describe('formatProviderForWaybar snapshots', () => {
  it('renders Claude healthy', () => {
    const out = formatProviderForWaybar(claudeHealthy());
    expect(sanitize(out.text)).toMatchSnapshot();
    expect(sanitize(out.tooltip)).toMatchSnapshot();
    expect(out.class).toMatchSnapshot();
  });

  it('renders Claude disconnected', () => {
    const out = formatProviderForWaybar(claudeError());
    expect(sanitize(out.text)).toMatchSnapshot();
    expect(sanitize(out.tooltip)).toMatchSnapshot();
    expect(out.class).toMatchSnapshot();
  });

  it('renders Codex healthy', () => {
    const out = formatProviderForWaybar(codexHealthy());
    expect(sanitize(out.text)).toMatchSnapshot();
    expect(sanitize(out.tooltip)).toMatchSnapshot();
    expect(out.class).toMatchSnapshot();
  });

  it('renders Amp healthy', () => {
    const out = formatProviderForWaybar(ampHealthy());
    expect(sanitize(out.text)).toMatchSnapshot();
    expect(sanitize(out.tooltip)).toMatchSnapshot();
    expect(out.class).toMatchSnapshot();
  });
});

// ---------------------------------------------------------------------------
// displayMode='used' snapshots
// ---------------------------------------------------------------------------

describe('Terminal formatter snapshots — displayMode=used', () => {
  it('renders Claude healthy as used', () => {
    const result = sanitize(formatForTerminal(wrap(claudeHealthy()), 'used'));
    expect(result).toMatchSnapshot();
  });

  it('renders Codex healthy as used', () => {
    const result = sanitize(formatForTerminal(wrap(codexHealthy()), 'used'));
    expect(result).toMatchSnapshot();
  });

  it('renders Amp healthy as used', () => {
    const result = sanitize(formatForTerminal(wrap(ampHealthy()), 'used'));
    expect(result).toMatchSnapshot();
  });

  it('renders all providers combined as used', () => {
    const result = sanitize(formatForTerminal(wrap(claudeHealthy(), codexHealthy(), ampHealthy()), 'used'));
    expect(result).toMatchSnapshot();
  });
});

describe('Waybar formatter snapshots — displayMode=used', () => {
  it('renders Claude healthy as used', () => {
    const out = formatForWaybar(wrap(claudeHealthy()), 'used');
    expect(sanitize(out.text)).toMatchSnapshot();
    expect(sanitize(out.tooltip)).toMatchSnapshot();
    expect(out.class).toMatchSnapshot();
  });

  it('renders Codex healthy as used', () => {
    const out = formatForWaybar(wrap(codexHealthy()), 'used');
    expect(sanitize(out.text)).toMatchSnapshot();
    expect(sanitize(out.tooltip)).toMatchSnapshot();
    expect(out.class).toMatchSnapshot();
  });

  it('renders Amp healthy as used (tooltip contains "Resets in")', () => {
    const out = formatForWaybar(wrap(ampHealthy()), 'used');
    expect(out.tooltip).toContain('Resets in');
    expect(out.tooltip).not.toContain('Full in');
    expect(sanitize(out.text)).toMatchSnapshot();
    expect(sanitize(out.tooltip)).toMatchSnapshot();
  });
});

// ---------------------------------------------------------------------------
// C1: account field — Terminal snapshots
// ---------------------------------------------------------------------------

describe('Terminal formatter snapshots — account field (C1)', () => {
  it('renders Amp with account', () => {
    const result = sanitize(formatForTerminal(wrap(ampWithAccount())));
    expect(result).toMatchSnapshot();
  });

  it('renders Copilot with account', () => {
    const result = sanitize(formatForTerminal(wrap(copilotWithAccount())));
    expect(result).toMatchSnapshot();
  });
});

// ---------------------------------------------------------------------------
// C1: account field — Waybar snapshots
// ---------------------------------------------------------------------------

describe('Waybar formatter snapshots — account field (C1)', () => {
  it('renders Amp with account', () => {
    const out = formatForWaybar(wrap(ampWithAccount()));
    expect(sanitize(out.text)).toMatchSnapshot();
    expect(sanitize(out.tooltip)).toMatchSnapshot();
    expect(out.class).toMatchSnapshot();
  });

  it('renders Copilot with account', () => {
    const out = formatForWaybar(wrap(copilotWithAccount()));
    expect(sanitize(out.text)).toMatchSnapshot();
    expect(sanitize(out.tooltip)).toMatchSnapshot();
    expect(out.class).toMatchSnapshot();
  });
});

// ---------------------------------------------------------------------------
// C1: account field — formatProviderForWaybar snapshots
// ---------------------------------------------------------------------------

describe('formatProviderForWaybar snapshots — account field (C1)', () => {
  it('renders Amp with account', () => {
    const out = formatProviderForWaybar(ampWithAccount());
    expect(sanitize(out.text)).toMatchSnapshot();
    expect(sanitize(out.tooltip)).toMatchSnapshot();
    expect(out.class).toMatchSnapshot();
  });

  it('renders Copilot with account', () => {
    const out = formatProviderForWaybar(copilotWithAccount());
    expect(sanitize(out.text)).toMatchSnapshot();
    expect(sanitize(out.tooltip)).toMatchSnapshot();
    expect(out.class).toMatchSnapshot();
  });
});

// ---------------------------------------------------------------------------
// C3: Rich builder fixtures — Terminal snapshots
// ---------------------------------------------------------------------------

describe('Terminal formatter snapshots — rich builder fixtures (C3)', () => {
  it('renders Claude with weeklyModels and extraUsage', () => {
    const result = sanitize(formatForTerminal(wrap(claudeWithExtras())));
    expect(result).toMatchSnapshot();
  });

  it('renders Amp with Credits section', () => {
    const result = sanitize(formatForTerminal(wrap(ampWithCredits())));
    expect(result).toMatchSnapshot();
  });

  it('renders Amp with unknown models (fallback path)', () => {
    const result = sanitize(formatForTerminal(wrap(ampUnknownModels())));
    expect(result).toMatchSnapshot();
  });
});

// ---------------------------------------------------------------------------
// C3: Rich builder fixtures — Waybar snapshots
// ---------------------------------------------------------------------------

describe('Waybar formatter snapshots — rich builder fixtures (C3)', () => {
  it('renders Claude with weeklyModels and extraUsage', () => {
    const out = formatForWaybar(wrap(claudeWithExtras()));
    expect(sanitize(out.text)).toMatchSnapshot();
    expect(sanitize(out.tooltip)).toMatchSnapshot();
    expect(out.class).toMatchSnapshot();
  });

  it('renders Amp with Credits section', () => {
    const out = formatForWaybar(wrap(ampWithCredits()));
    expect(sanitize(out.text)).toMatchSnapshot();
    expect(sanitize(out.tooltip)).toMatchSnapshot();
    expect(out.class).toMatchSnapshot();
  });

  it('renders Amp with unknown models (fallback path)', () => {
    const out = formatForWaybar(wrap(ampUnknownModels()));
    expect(sanitize(out.text)).toMatchSnapshot();
    expect(sanitize(out.tooltip)).toMatchSnapshot();
    expect(out.class).toMatchSnapshot();
  });
});

// ---------------------------------------------------------------------------
// C3: Rich builder fixtures — formatProviderForWaybar snapshots
// ---------------------------------------------------------------------------

describe('formatProviderForWaybar snapshots — rich builder fixtures (C3)', () => {
  it('renders Claude with weeklyModels and extraUsage', () => {
    const out = formatProviderForWaybar(claudeWithExtras());
    expect(sanitize(out.text)).toMatchSnapshot();
    expect(sanitize(out.tooltip)).toMatchSnapshot();
    expect(out.class).toMatchSnapshot();
  });

  it('renders Amp with Credits section', () => {
    const out = formatProviderForWaybar(ampWithCredits());
    expect(sanitize(out.text)).toMatchSnapshot();
    expect(sanitize(out.tooltip)).toMatchSnapshot();
    expect(out.class).toMatchSnapshot();
  });

  it('renders Amp with unknown models (fallback path)', () => {
    const out = formatProviderForWaybar(ampUnknownModels());
    expect(sanitize(out.text)).toMatchSnapshot();
    expect(sanitize(out.tooltip)).toMatchSnapshot();
    expect(out.class).toMatchSnapshot();
  });
});
