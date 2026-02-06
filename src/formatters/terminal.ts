import { CONFIG } from '../config';
import type { AllQuotas, ProviderQuota, QuotaWindow } from '../providers/types';

// ANSI color codes (Catppuccin Mocha)
const ANSI = {
  reset: '\x1b[0m',
  bold: '\x1b[1m',
  // Catppuccin colors via 256-color mode
  green: '\x1b[38;2;166;227;161m',    // #a6e3a1
  yellow: '\x1b[38;2;249;226;175m',   // #f9e2af
  orange: '\x1b[38;2;250;179;135m',   // #fab387
  red: '\x1b[38;2;243;139;168m',      // #f38ba8
  muted: '\x1b[38;2;108;112;134m',    // #6c7086
  text: '\x1b[38;2;205;214;244m',     // #cdd6f4
  subtext: '\x1b[38;2;186;194;222m',  // #bac2de
  lavender: '\x1b[38;2;180;190;254m', // #b4befe
  teal: '\x1b[38;2;148;226;213m',     // #94e2d5
  mauve: '\x1b[38;2;203;166;247m',    // #cba6f7
  blue: '\x1b[38;2;137;180;250m',     // #89b4fa
  sapphire: '\x1b[38;2;116;199;236m', // #74c7ec
  peach: '\x1b[38;2;250;179;135m',    // #fab387
};

/**
 * Get ANSI color for percentage
 */
function getAnsiColor(pct: number | null): string {
  if (pct === null) return ANSI.text;
  if (pct >= CONFIG.thresholds.green) return ANSI.green;
  if (pct >= CONFIG.thresholds.yellow) return ANSI.yellow;
  if (pct >= CONFIG.thresholds.orange) return ANSI.orange;
  return ANSI.red;
}

/**
 * Generate a colored progress bar for terminal
 */
function formatBar(pct: number | null): string {
  if (pct === null) {
    return `${ANSI.muted}░░░░░░░░░░░░░░░░░░░░${ANSI.reset}`;
  }

  const filled = Math.floor(pct / 5);
  const empty = 20 - filled;
  const color = getAnsiColor(pct);

  const filledStr = '█'.repeat(filled);
  const emptyStr = '░'.repeat(empty);

  return `${color}${filledStr}${ANSI.muted}${emptyStr}${ANSI.reset}`;
}

/**
 * Format percentage without decimals
 */
function formatPct(pct: number | null): string {
  if (pct === null) return '?%';
  return `${Math.round(pct)}%`;
}

/**
 * Format human-readable time until reset
 * >24h = "Xd XXh", <24h = "XXh XXm"
 */
function formatEta(isoDate: string | null): string {
  if (!isoDate) return '?';

  const now = Date.now();
  const resetTime = new Date(isoDate).getTime();
  const diff = resetTime - now;

  if (diff < 0) return '0m';

  const days = Math.floor(diff / 86400000);
  const hours = Math.floor((diff % 86400000) / 3600000);
  const minutes = Math.floor((diff % 3600000) / 60000);

  if (days > 0) {
    return `${days}d ${hours.toString().padStart(2, '0')}h`;
  }
  return `${hours}h ${minutes.toString().padStart(2, '0')}m`;
}

/**
 * Format reset time as HH:MM
 */
function formatResetTime(isoDate: string | null): string {
  if (!isoDate) return '??:??';

  const date = new Date(isoDate);
  const hours = date.getHours().toString().padStart(2, '0');
  const minutes = date.getMinutes().toString().padStart(2, '0');
  return `${hours}:${minutes}`;
}

/**
 * Timeline separator
 */
function sep(): string {
  return `${ANSI.sapphire}│${ANSI.reset}`;
}

/**
 * Filter out internal/test models and Gemini 2.5
 */
function isValidModel(name: string): boolean {
  const lowerName = name.toLowerCase();
  const excludePatterns = [
    /^tab_/i,
    /^chat_/i,
    /^test_/i,
    /^internal_/i,
    /preview$/i,
    /2\.5/i,
  ];
  return !excludePatterns.some(p => p.test(lowerName));
}

/**
 * Format a single quota line for terminal
 */
function formatQuotaLine(
  label: string,
  window: QuotaWindow | undefined,
  maxLabelLen: number = 14
): string {
  const pct = window?.remaining ?? null;
  const bar = formatBar(pct);
  const color = getAnsiColor(pct);
  const eta = formatEta(window?.resetsAt ?? null);
  const resetTime = formatResetTime(window?.resetsAt ?? null);
  const pctStr = formatPct(pct).padStart(4);

  return `${sep()}   ${ANSI.lavender}${label.padEnd(maxLabelLen)}${ANSI.reset} ${bar} ${color}${pctStr}${ANSI.reset} ${ANSI.teal}→ ${eta.padEnd(8)} (${resetTime})${ANSI.reset}`;
}

/**
 * Format quotas for terminal output
 */
export function formatForTerminal(quotas: AllQuotas): string {
  const lines: string[] = [];

  for (const provider of quotas.providers) {
    if (!provider.available && !provider.error) continue;

    // Provider header
    let header = provider.displayName;
    if (provider.plan) header += ` ${ANSI.subtext}(${provider.plan})${ANSI.reset}`;
    else if (provider.account) header += ` ${ANSI.subtext}(${provider.account})${ANSI.reset}`;
    
    if (lines.length > 0) lines.push('');
    lines.push(`${sep()} ${ANSI.mauve}${ANSI.bold}${header}${ANSI.reset}`);

    // Error message if any
    if (provider.error) {
      lines.push(`${sep()}   ${ANSI.peach}⚠️ ${provider.error}${ANSI.reset}`);
      continue;
    }

    // Primary/secondary windows (only for non-Antigravity providers)
    if (provider.provider !== 'antigravity') {
      if (provider.primary) {
        lines.push(formatQuotaLine('5h Window', provider.primary, 20));
      }

      if (provider.secondary) {
        lines.push(formatQuotaLine('Weekly', provider.secondary, 20));
      }
    }

    // Extra Usage (Claude)
    if (provider.extraUsage?.enabled) {
      const pct = provider.extraUsage.remaining;
      const used = provider.extraUsage.used;
      const limit = provider.extraUsage.limit;
      const bar = formatBar(pct);
      const color = getAnsiColor(pct);
      const pctStr = formatPct(pct).padStart(4);
      const usedStr = `$${(used / 100).toFixed(2)}/$${(limit / 100).toFixed(2)}`;
      
      lines.push(`${sep()}   ${ANSI.blue}${'Extra Usage'.padEnd(20)}${ANSI.reset} ${bar} ${color}${pctStr}${ANSI.reset} ${ANSI.subtext}${usedStr}${ANSI.reset}`);
    }

    // Additional models (Antigravity) - with individual times
    if (provider.models) {
      const models = Object.entries(provider.models)
        .filter(([name]) => isValidModel(name))
        .sort(([a], [b]) => {
          const getPriority = (name: string): number => {
            const lower = name.toLowerCase();
            if (lower.includes('claude')) return 0;
            if (lower.includes('gpt')) return 1;
            if (lower.includes('gemini')) return 2;
            return 3;
          };
          const aPri = getPriority(a);
          const bPri = getPriority(b);
          if (aPri !== bPri) return aPri - bPri;
          return a.localeCompare(b);
        });

      const maxLen = Math.max(...models.map(([name]) => name.length), 20);
      
      for (const [modelName, window] of models) {
        lines.push(formatQuotaLine(modelName, window, maxLen));
      }
    }
  }

  if (lines.length === 0) {
    return `${ANSI.muted}No providers connected${ANSI.reset}`;
  }

  return lines.join('\n');
}

/**
 * Output to terminal (stdout)
 */
export function outputTerminal(quotas: AllQuotas): void {
  console.log(formatForTerminal(quotas));
}
