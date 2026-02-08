import { homedir } from 'os';
import { join } from 'path';

// XDG Base Directory paths
const XDG_CONFIG_HOME = Bun.env.XDG_CONFIG_HOME || join(homedir(), '.config');

export const CONFIG = {
  // Paths
  paths: {
    // Cache inside waybar config folder
    cache: join(XDG_CONFIG_HOME, 'waybar', 'qbar', 'cache'),
    config: join(XDG_CONFIG_HOME, 'qbar'),
    
    // Provider credential paths
    claude: {
      credentials: join(homedir(), '.claude', '.credentials.json'),
    },
    codex: {
      auth: join(homedir(), '.codex', 'auth.json'),
      sessions: join(homedir(), '.codex', 'sessions'),
    },
    antigravity: {
      accounts: join(XDG_CONFIG_HOME, 'antigravity-usage', 'accounts'),
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
    antigravityTtlMs: 900_000,   // 15 minutes for Antigravity (API is slower)
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

  // UI Colors (Catppuccin Mocha)
  colors: {
    green: '#a6e3a1',   // >= 60%
    yellow: '#f9e2af',  // >= 30%
    orange: '#fab387',  // >= 10%
    red: '#f38ba8',     // < 10%
    muted: '#6c7086',   // empty bar segments
    text: '#cdd6f4',    // default/unknown text
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
