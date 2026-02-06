import { CONFIG } from '../config';
import type { AllQuotas, ProviderQuota, QuotaWindow } from '../providers/types';

// ANSI color codes
const ANSI = {
  reset: '\x1b[0m',
  bold: '\x1b[1m',
  green: '\x1b[32m',
  yellow: '\x1b[33m',
  red: '\x1b[31m',
  gray: '\x1b[90m',
  white: '\x1b[37m',
};

/**
 * Get ANSI color for percentage
 */
function getAnsiColor(pct: number | null): string {
  if (pct === null) return ANSI.white;
  if (pct >= CONFIG.thresholds.green) return ANSI.green;
  if (pct >= CONFIG.thresholds.yellow) return ANSI.yellow;
  return ANSI.red;
}

/**
 * Generate a colored progress bar for terminal
 */
function formatBar(pct: number | null): string {
  if (pct === null) {
    return `${ANSI.gray}░░░░░░░░░░░░░░░░░░░░${ANSI.reset}`;
  }

  const filled = Math.floor(pct / 5);
  const empty = 20 - filled;
  const color = getAnsiColor(pct);

  const filledStr = '█'.repeat(filled);
  const emptyStr = '░'.repeat(empty);

  return `${color}${filledStr}${ANSI.gray}${emptyStr}${ANSI.reset}`;
}

/**
 * Format human-readable time until reset
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
 * Format reset time as HH:MM or DD/MM
 */
function formatResetTime(isoDate: string | null, isWeekly: boolean = false): string {
  if (!isoDate) return '?';

  const date = new Date(isoDate);
  if (isWeekly) {
    const day = date.getDate().toString().padStart(2, '0');
    const month = (date.getMonth() + 1).toString().padStart(2, '0');
    return `${day}/${month}`;
  }
  
  const hours = date.getHours().toString().padStart(2, '0');
  const minutes = date.getMinutes().toString().padStart(2, '0');
  return `${hours}:${minutes}`;
}

/**
 * Format a single quota line for terminal
 */
function formatQuotaLine(
  label: string,
  window: QuotaWindow | undefined,
  isWeekly: boolean = false
): string {
  const pct = window?.remaining ?? null;
  const bar = formatBar(pct);
  const color = getAnsiColor(pct);
  const eta = formatEta(window?.resetsAt ?? null);
  const resetTime = formatResetTime(window?.resetsAt ?? null, isWeekly);
  const pctStr = pct !== null ? `${pct.toString().padStart(3)}%` : '  ?%';

  return `${label.padEnd(14)} ${bar} ${color}${pctStr}${ANSI.reset} - ${eta.padEnd(7)} (${resetTime})`;
}

/**
 * Format quotas for terminal output
 */
export function formatForTerminal(quotas: AllQuotas): string {
  const lines: string[] = [];

  for (const provider of quotas.providers) {
    if (!provider.available) continue;

    // Provider header
    const header = provider.plan 
      ? `${provider.displayName} (${provider.plan})`
      : provider.account
        ? `${provider.displayName} (${provider.account})`
        : provider.displayName;
    
    if (lines.length > 0) lines.push('');
    lines.push(`${ANSI.bold}━━━ ${header} ━━━${ANSI.reset}`);

    // Error message if any
    if (provider.error) {
      lines.push(`${ANSI.yellow}⚠️ ${provider.error}${ANSI.reset}`);
      continue;
    }

    // Primary/secondary windows
    if (provider.primary) {
      const label = provider.provider === 'codex' ? '5h Window' : '5h Window';
      lines.push(formatQuotaLine(label, provider.primary));
    }

    if (provider.secondary) {
      lines.push(formatQuotaLine('Weekly', provider.secondary, true));
    }

    // Additional models (Antigravity)
    if (provider.models) {
      for (const [modelName, window] of Object.entries(provider.models)) {
        // Skip if already shown as primary
        if (provider.primary && modelName.toLowerCase().includes('claude')) continue;
        lines.push(formatQuotaLine(modelName, window));
      }
    }
  }

  if (lines.length === 0) {
    return `${ANSI.gray}No providers connected${ANSI.reset}`;
  }

  return lines.join('\n');
}

/**
 * Output to terminal (stdout)
 */
export function outputTerminal(quotas: AllQuotas): void {
  console.log(formatForTerminal(quotas));
}
