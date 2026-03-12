import { homedir } from 'os';
import { join } from 'path';
import { ONE_DARK } from './theme';

// XDG Base Directory paths
const XDG_CONFIG_HOME = Bun.env.XDG_CONFIG_HOME || join(homedir(), '.config');
const XDG_CACHE_HOME = Bun.env.XDG_CACHE_HOME || join(homedir(), '.cache');

export const CONFIG = {
  // Paths
  paths: {
    cache: join(XDG_CACHE_HOME, 'qbar'),
    legacyCache: join(XDG_CONFIG_HOME, 'waybar', 'qbar', 'cache'),
    config: join(XDG_CONFIG_HOME, 'qbar'),
    
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
  },

  // Cache settings
  cache: {
    ttlMs: 300_000,              // 5 minutes default TTL
    codexTtlMs: 300_000,         // 5 minutes for Codex
    lockTimeoutMs: 5_000,        // Lock timeout
  },

  // API settings
  api: {
    timeoutMs: 5_000,        // HTTP timeout
    claude: {
      usageUrl: 'https://api.anthropic.com/api/oauth/usage',
      betaHeader: 'oauth-2025-04-20',
    },
  },

  // UI Colors (One Dark)
  colors: {
    green: ONE_DARK.green,   // >= 60%
    yellow: ONE_DARK.yellow, // >= 30%
    orange: ONE_DARK.orange, // >= 10%
    red: ONE_DARK.red,       // < 10%
    muted: ONE_DARK.comment, // empty bar segments
    text: ONE_DARK.text,     // default/unknown text
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
