import { describe, expect, it } from 'bun:test';
import { formatForTerminal } from '../src/formatters/terminal';
import { formatForWaybar, formatProviderForWaybar } from '../src/formatters/waybar';
import type { AllQuotas, ProviderQuota } from '../src/providers/types';

// ---------------------------------------------------------------------------
// Sanitize dynamic values so snapshots remain stable across runs.
// ---------------------------------------------------------------------------

const ISO_RE = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z/g;
const TIME_HM_RE = /\d{1,2}h \d{2}m/g;
const TIME_DH_RE = /\d+d \d{2}h/g;
const PAREN_TIME_RE = /\(\d{2}:\d{2}\)/g;
const AGO_RE = /\d+[hm] ago/g;
const JUST_NOW_RE = /just now/g;

function sanitize(s: string): string {
  return s
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

function claudeHealthy(): ProviderQuota {
  return {
    provider: 'claude',
    displayName: 'Claude',
    available: true,
    plan: 'Pro',
    primary: { remaining: 75, resetsAt: FIXED_RESET, windowMinutes: 300 },
    secondary: { remaining: 90, resetsAt: FIXED_RESET, windowMinutes: 10080 },
  };
}

function claudeError(): ProviderQuota {
  return {
    provider: 'claude',
    displayName: 'Claude',
    available: false,
    error: 'Token expired. Open `agent-bar-omarchy menu` and choose Provider login.',
  };
}

function codexHealthy(): ProviderQuota {
  return {
    provider: 'codex',
    displayName: 'Codex',
    available: true,
    plan: 'Pro',
    planType: 'pro',
    primary: { remaining: 60, resetsAt: FIXED_RESET, windowMinutes: 300 },
    secondary: { remaining: 85, resetsAt: FIXED_RESET, windowMinutes: 10080 },
    modelsDetailed: {
      Codex: {
        fiveHour: { remaining: 60, resetsAt: FIXED_RESET, windowMinutes: 300 },
        sevenDay: { remaining: 85, resetsAt: FIXED_RESET, windowMinutes: 10080 },
      },
    },
    models: {
      Codex: { remaining: 60, resetsAt: FIXED_RESET, windowMinutes: 300 },
    },
  };
}

function codexError(): ProviderQuota {
  return {
    provider: 'codex',
    displayName: 'Codex',
    available: false,
    error: 'No session data found',
  };
}

function ampHealthy(): ProviderQuota {
  return {
    provider: 'amp',
    displayName: 'Amp',
    available: true,
    primary: { remaining: 70, resetsAt: FIXED_RESET },
    models: {
      'Free Tier': { remaining: 70, resetsAt: FIXED_RESET },
    },
    meta: {
      freeRemaining: '$3.50',
      freeTotal: '$5.00',
      replenishRate: '+$0.25/hr',
    },
  };
}

function ampError(): ProviderQuota {
  return {
    provider: 'amp',
    displayName: 'Amp',
    available: false,
    error: 'Amp CLI not installed. Right-click to install and log in.',
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
