import { homedir } from 'os';
import { join } from 'path';
import { APP_NAME } from './app-identity';
import { ONE_DARK } from './theme';

// XDG Base Directory paths
const XDG_CONFIG_HOME = process.env.XDG_CONFIG_HOME ?? Bun.env.XDG_CONFIG_HOME ?? join(homedir(), '.config');
const XDG_CACHE_HOME = process.env.XDG_CACHE_HOME ?? Bun.env.XDG_CACHE_HOME ?? join(homedir(), '.cache');
const COPILOT_HOME = process.env.COPILOT_HOME ?? Bun.env.COPILOT_HOME ?? join(homedir(), '.copilot');

export const CONFIG = {
  // Paths
  paths: {
    cache: join(XDG_CACHE_HOME, APP_NAME),
    config: join(XDG_CONFIG_HOME, APP_NAME),

    // Provider credential paths
    claude: {
      credentials: join(homedir(), '.claude', '.credentials.json'),
    },
    codex: {
      auth: join(homedir(), '.codex', 'auth.json'),
      sessions: join(homedir(), '.codex', 'sessions'),
    },
    amp: {
      bin: join(homedir(), '.cache', '.bun', 'bin', 'amp'),
      settings: join(XDG_CONFIG_HOME, 'amp', 'settings.json'),
      threads: join(homedir(), '.local', 'share', 'amp', 'threads'),
    },
    copilot: {
      home: COPILOT_HOME,
      config: join(COPILOT_HOME, 'config.json'),
      settings: join(COPILOT_HOME, 'settings.json'),
    },
  },

  // Cache settings
  cache: {
    ttlMs: 300_000, // 5 minutes default TTL
    lockTimeoutMs: 5_000, // Lock timeout
  },

  // API settings
  api: {
    timeoutMs: 5_000, // HTTP timeout
    claude: {
      usageUrl: 'https://api.anthropic.com/api/oauth/usage',
      betaHeader: 'oauth-2025-04-20',
      // The OAuth usage endpoint is Claude Code's own undocumented endpoint.
      // Requests without a `claude-code/<version>` User-Agent land in a stricter
      // rate-limit bucket and hit persistent 429s. Mimic the expected client.
      userAgent: 'claude-code/2.1.179',
    },
  },

  // UI Colors (One Dark)
  colors: {
    green: ONE_DARK.green, // >= 60%
    yellow: ONE_DARK.yellow, // >= 30%
    orange: ONE_DARK.orange, // >= 10%
    red: ONE_DARK.red, // < 10%
    muted: ONE_DARK.comment, // empty bar segments
    text: ONE_DARK.text, // default/unknown text
  },

  // Thresholds for color coding (percentage remaining)
  thresholds: {
    green: 60,
    yellow: 30,
    orange: 10,
  },
} as const;

export type Config = typeof CONFIG;

// Get color based on percentage remaining
export function getColorForPercent(pct: number | null): string {
  if (pct === null) return CONFIG.colors.text;
  if (pct >= CONFIG.thresholds.green) return CONFIG.colors.green;
  if (pct >= CONFIG.thresholds.yellow) return CONFIG.colors.yellow;
  if (pct >= CONFIG.thresholds.orange) return CONFIG.colors.orange;
  return CONFIG.colors.red;
}

/** Status bucket derived from health percentage (0-100). Matches `CONFIG.thresholds`. */
export type HealthStatus = 'ok' | 'low' | 'warn' | 'critical';

export function getStatusForPercent(pct: number | null): HealthStatus {
  if (pct === null) return 'ok';
  if (pct < CONFIG.thresholds.orange) return 'critical';
  if (pct < CONFIG.thresholds.yellow) return 'warn';
  if (pct < CONFIG.thresholds.green) return 'low';
  return 'ok';
}
