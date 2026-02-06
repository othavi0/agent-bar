import { CONFIG, getColorForPercent } from '../config';
import type { AllQuotas, ProviderQuota, QuotaWindow } from '../providers/types';

interface WaybarOutput {
  text: string;
  tooltip: string;
  class: string;
}

/**
 * Format percentage with color span for Pango markup
 */
function formatPctSpan(label: string, pct: number | null): string {
  const color = getColorForPercent(pct);
  const display = pct !== null ? `${pct}%` : '?%';
  return `<span foreground='${color}'>${label} ${display}</span>`;
}

/**
 * Generate a 20-character progress bar
 */
function formatBar(pct: number | null): string {
  if (pct === null) {
    return `<span foreground='${CONFIG.colors.muted}'>░░░░░░░░░░░░░░░░░░░░</span>`;
  }

  const filled = Math.floor(pct / 5);
  const empty = 20 - filled;
  const color = getColorForPercent(pct);

  const filledStr = '█'.repeat(filled);
  const emptyStr = '░'.repeat(empty);

  return `<span foreground='${color}'>${filledStr}</span><span foreground='${CONFIG.colors.muted}'>${emptyStr}</span>`;
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
 * Format a single quota line for tooltip
 */
function formatQuotaLine(
  label: string,
  window: QuotaWindow | undefined,
  isWeekly: boolean = false
): string {
  const pct = window?.remaining ?? null;
  const bar = formatBar(pct);
  const eta = formatEta(window?.resetsAt ?? null);
  const resetTime = formatResetTime(window?.resetsAt ?? null, isWeekly);
  const pctStr = pct !== null ? `${pct.toString().padStart(3)}%` : '  ?%';

  return `${label.padEnd(14)} ${bar} ${pctStr} - ${eta.padEnd(7)} (${resetTime})`;
}

/**
 * Build tooltip content from all quotas
 */
function buildTooltip(quotas: AllQuotas): string {
  const lines: string[] = [];

  for (const provider of quotas.providers) {
    if (!provider.available) continue;

    // Provider header
    const header = provider.plan 
      ? `━━━ ${provider.displayName} (${provider.plan}) ━━━`
      : provider.account
        ? `━━━ ${provider.displayName} (${provider.account}) ━━━`
        : `━━━ ${provider.displayName} ━━━`;
    
    if (lines.length > 0) lines.push('');
    lines.push(header);

    // Error message if any
    if (provider.error) {
      lines.push(`⚠️ ${provider.error}`);
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

    // Extra Usage (Claude Pro feature)
    if (provider.extraUsage?.enabled) {
      const pct = provider.extraUsage.remaining;
      const bar = formatBar(pct);
      const usedStr = `$${(provider.extraUsage.used / 100).toFixed(2)} used`;
      lines.push(`${'Extra Usage'.padEnd(14)} ${bar} ${pct.toString().padStart(3)}% - ${usedStr}`);
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

  return lines.join('\n');
}

/**
 * Build bar text from all quotas
 */
function buildText(quotas: AllQuotas): string {
  const parts: string[] = [];
  const sep = "<span weight='bold'>·</span>";

  for (const provider of quotas.providers) {
    if (!provider.available) continue;

    const pct = provider.primary?.remaining ?? null;
    const abbrev = getAbbrev(provider.provider);
    parts.push(formatPctSpan(abbrev, pct));
  }

  if (parts.length === 0) {
    return `| <span foreground='${CONFIG.colors.text}'>Connect to Provider</span> |`;
  }

  return `| ${parts.join(` ${sep} `)} |`;
}

/**
 * Get short abbreviation for provider
 */
function getAbbrev(providerId: string): string {
  switch (providerId) {
    case 'claude': return 'Cld';
    case 'codex': return 'Cdx';
    case 'antigravity': return 'AG';
    default: return providerId.slice(0, 3);
  }
}

/**
 * Format quotas for Waybar JSON output
 */
export function formatForWaybar(quotas: AllQuotas): WaybarOutput {
  return {
    text: buildText(quotas),
    tooltip: buildTooltip(quotas),
    class: 'llm-usage',
  };
}

/**
 * Output Waybar JSON to stdout
 */
export function outputWaybar(quotas: AllQuotas): void {
  const output = formatForWaybar(quotas);
  console.log(JSON.stringify(output));
}
