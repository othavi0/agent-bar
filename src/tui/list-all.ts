import * as p from '@clack/prompts';
import { getAllQuotas } from '../providers';
import { catppuccin, semantic, getQuotaColor, colorize, bold } from './colors';
import type { ProviderQuota, QuotaWindow } from '../providers/types';

function formatBar(pct: number | null, width: number = 20): string {
  if (pct === null) {
    return colorize('░'.repeat(width), semantic.muted);
  }

  const filled = Math.floor((pct / 100) * width);
  const empty = width - filled;
  const color = getQuotaColor(pct);

  return colorize('█'.repeat(filled), color) + colorize('░'.repeat(empty), semantic.muted);
}

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

function formatQuotaLine(label: string, window: QuotaWindow | undefined): string {
  const pct = window?.remaining ?? null;
  const bar = formatBar(pct);
  const pctStr = pct !== null 
    ? colorize(`${pct.toString().padStart(3)}%`, getQuotaColor(pct))
    : colorize('  ?%', semantic.muted);
  const eta = window?.resetsAt 
    ? colorize(`⏱ ${formatEta(window.resetsAt)}`, semantic.subtitle)
    : '';

  return `  ${label.padEnd(16)} ${bar} ${pctStr}  ${eta}`;
}

function formatProvider(provider: ProviderQuota): string[] {
  const lines: string[] = [];

  // Header
  const headerInfo = provider.plan 
    ? `(${provider.plan})`
    : provider.account 
      ? `(${provider.account})`
      : '';
  
  lines.push('');
  lines.push(colorize(`  ━━━ ${provider.displayName} ${headerInfo} ━━━`, semantic.title));

  if (!provider.available) {
    lines.push(colorize(`  Not logged in`, semantic.muted));
    return lines;
  }

  if (provider.error) {
    lines.push(colorize(`  ⚠ ${provider.error}`, semantic.danger));
    return lines;
  }

  // Primary window
  if (provider.primary) {
    lines.push(formatQuotaLine('5h Window', provider.primary));
  }

  // Secondary window
  if (provider.secondary) {
    lines.push(formatQuotaLine('Weekly', provider.secondary));
  }

  // Additional models (Antigravity)
  if (provider.models) {
    for (const [modelName, window] of Object.entries(provider.models)) {
      // Skip if already shown as primary
      if (provider.primary && modelName.toLowerCase().includes('claude')) continue;
      lines.push(formatQuotaLine(modelName, window));
    }
  }

  return lines;
}

export async function showListAll(): Promise<void> {
  const s = p.spinner();
  s.start('Loading quotas...');

  const quotas = await getAllQuotas();
  
  s.stop('Quotas loaded');

  // Build output
  const lines: string[] = [];
  
  for (const provider of quotas.providers) {
    lines.push(...formatProvider(provider));
  }

  // Print with nice formatting
  console.log('');
  console.log(colorize(bold('  Current Quotas'), semantic.accent));
  console.log(colorize('  ─────────────────────────────────────────────────', semantic.muted));
  
  for (const line of lines) {
    console.log(line);
  }

  console.log('');
  console.log(colorize('  ─────────────────────────────────────────────────', semantic.muted));
  console.log('');

  await p.text({
    message: colorize('Press Enter to continue...', semantic.subtitle),
    placeholder: '',
    validate: () => undefined,
  });
}
