import { CONFIG, getColorForPercent } from '../config';
import type { AllQuotas, ProviderQuota, QuotaWindow } from '../providers/types';

// Catppuccin Mocha extended palette
const COLORS = {
  // Status colors (threshold-based)
  green: '#a6e3a1',
  yellow: '#f9e2af',
  orange: '#fab387',
  red: '#f38ba8',
  
  // UI colors
  text: '#cdd6f4',
  subtext: '#bac2de',
  muted: '#6c7086',
  surface: '#45475a',
  
  // Accent colors
  lavender: '#b4befe',
  teal: '#94e2d5',
  pink: '#f5c2e7',
  blue: '#89b4fa',
  mauve: '#cba6f7',
  peach: '#fab387',
  sapphire: '#74c7ec',
} as const;

interface WaybarOutput {
  text: string;
  tooltip: string;
  class: string;
}

interface ModelEntry {
  name: string;
  remaining: number | null;
  resetsAt: string | null;
}

/**
 * Format percentage without decimals
 */
function formatPct(pct: number | null): string {
  if (pct === null) return '?%';
  return `${Math.round(pct)}%`;
}

/**
 * Format percentage with color span
 */
function formatPctSpan(pct: number | null): string {
  const color = getColorForPercent(pct);
  return `<span foreground='${color}'>${formatPct(pct)}</span>`;
}

/**
 * Generate a 20-character progress bar
 */
function formatBar(pct: number | null): string {
  if (pct === null) {
    return `<span foreground='${COLORS.muted}'>░░░░░░░░░░░░░░░░░░░░</span>`;
  }

  const filled = Math.floor(pct / 5);
  const empty = 20 - filled;
  const color = getColorForPercent(pct);

  return `<span foreground='${color}'>${'▰'.repeat(filled)}</span><span foreground='${COLORS.muted}'>${'▱'.repeat(empty)}</span>`;
}

/**
 * Format time until reset: >24h = "Xd XXh", <24h = "XXh XXm"
 */
function formatEta(isoDate: string | null): string {
  if (!isoDate) return '?';

  const diff = new Date(isoDate).getTime() - Date.now();
  if (diff < 0) return '0m';

  const days = Math.floor(diff / 86400000);
  const hours = Math.floor((diff % 86400000) / 3600000);
  const minutes = Math.floor((diff % 3600000) / 60000);

  if (days > 0) return `${days}d ${hours.toString().padStart(2, '0')}h`;
  return `${hours}h ${minutes.toString().padStart(2, '0')}m`;
}

/**
 * Format reset time as HH:MM
 */
function formatResetTime(isoDate: string | null): string {
  if (!isoDate) return '??:??';
  const d = new Date(isoDate);
  return `${d.getHours().toString().padStart(2, '0')}:${d.getMinutes().toString().padStart(2, '0')}`;
}

/**
 * Status indicator by percentage
 */
function getStatusIndicator(pct: number | null): string {
  if (pct === null) return `<span foreground='${COLORS.muted}'>○</span>`;
  if (pct < 10) return `<span foreground='${COLORS.red}'>●</span>`;
  if (pct < 30) return `<span foreground='${COLORS.orange}'>●</span>`;
  if (pct < 60) return `<span foreground='${COLORS.yellow}'>●</span>`;
  return `<span foreground='${COLORS.green}'>●</span>`;
}

/**
 * Timeline separator
 */
const SEP = `<span foreground='#74c7ec'>│</span>`;

/**
 * Filter models: exclude internal, 2.5, and merge Thinking variants
 */
function filterAndMergeModels(models: Record<string, QuotaWindow>): ModelEntry[] {
  const validModels = new Map<string, ModelEntry>();
  
  for (const [name, window] of Object.entries(models)) {
    const lowerName = name.toLowerCase();
    
    // Skip internal/test models
    if (/^(tab_|chat_|test_|internal_)/.test(lowerName) || /preview$/i.test(name)) continue;
    // Skip 2.5 models
    if (/2\.5/i.test(name)) continue;
    
    // Remove "(Thinking)" suffix for grouping
    const baseName = name.replace(/\s*\(Thinking\)/i, '');
    
    // Keep the one with lower remaining (more used) or first seen
    if (!validModels.has(baseName)) {
      validModels.set(baseName, {
        name: baseName,
        remaining: window.remaining,
        resetsAt: window.resetsAt,
      });
    }
  }
  
  // Sort: Claude > GPT > Gemini > Others
  return [...validModels.values()].sort((a, b) => {
    const getPriority = (n: string) => {
      const l = n.toLowerCase();
      if (l.includes('claude')) return 0;
      if (l.includes('gpt')) return 1;
      if (l.includes('gemini')) return 2;
      return 3;
    };
    const diff = getPriority(a.name) - getPriority(b.name);
    return diff !== 0 ? diff : a.name.localeCompare(b.name);
  });
}

/**
 * Format a model line
 */
function formatModelLine(name: string, pct: number | null, resetsAt: string | null, maxLen: number): string {
  const indicator = getStatusIndicator(pct);
  const nameSpan = `<span foreground='${COLORS.lavender}'>${name.padEnd(maxLen)}</span>`;
  const bar = formatBar(pct);
  const pctSpan = `<span foreground='${getColorForPercent(pct)}'>${formatPct(pct).padStart(4)}</span>`;
  const eta = `<span foreground='${COLORS.teal}'>→ ${formatEta(resetsAt)} (${formatResetTime(resetsAt)})</span>`;
  
  return `${SEP}   ${indicator} ${nameSpan} ${bar} ${pctSpan} ${eta}`;
}

/**
 * Build Claude section - show models with shared 5h time
 */
function buildClaudeSection(provider: ProviderQuota): string[] {
  const lines: string[] = [];
  
  // Header
  const planStr = provider.plan ? ` <span foreground='${COLORS.subtext}'>(${provider.plan})</span>` : '';
  lines.push(`${SEP} <span foreground='${COLORS.mauve}' weight='bold'>Claude${planStr}</span>`);

  if (provider.error) {
    lines.push(`${SEP}   <span foreground='${COLORS.peach}'>⚠️ ${provider.error}</span>`);
    return lines;
  }

  // Models available on Claude Pro
  const claudeModels = ['Opus', 'Sonnet', 'Haiku'];
  const maxLen = 20;
  
  if (provider.primary) {
    for (const model of claudeModels) {
      lines.push(formatModelLine(model, provider.primary.remaining, provider.primary.resetsAt, maxLen));
    }
  }

  // Weekly window
  if (provider.secondary) {
    lines.push(`${SEP}`);
    lines.push(`${SEP}   <span foreground='${COLORS.subtext}'>Weekly limit:</span>`);
    const pct = provider.secondary.remaining;
    lines.push(`${SEP}   ${getStatusIndicator(pct)} <span foreground='${COLORS.lavender}'>${'All Models'.padEnd(maxLen)}</span> ${formatBar(pct)} <span foreground='${getColorForPercent(pct)}'>${formatPct(pct).padStart(4)}</span> <span foreground='${COLORS.teal}'>→ ${formatEta(provider.secondary.resetsAt)} (${formatResetTime(provider.secondary.resetsAt)})</span>`);
  }

  // Extra Usage
  if (provider.extraUsage?.enabled) {
    const pct = provider.extraUsage.remaining;
    const used = provider.extraUsage.used;
    const limit = provider.extraUsage.limit;
    lines.push(`${SEP}`);
    lines.push(`${SEP}   ${getStatusIndicator(pct)} <span foreground='${COLORS.blue}'>${'Extra Usage'.padEnd(maxLen)}</span> ${formatBar(pct)} <span foreground='${getColorForPercent(pct)}'>${formatPct(pct).padStart(4)}</span> <span foreground='${COLORS.subtext}'>$${(used / 100).toFixed(2)}/$${(limit / 100).toFixed(2)}</span>`);
  }

  return lines;
}

/**
 * Build Codex section - show model with 5h time
 */
function buildCodexSection(provider: ProviderQuota): string[] {
  const lines: string[] = [];
  
  lines.push(`${SEP} <span foreground='${COLORS.mauve}' weight='bold'>Codex</span>`);

  if (provider.error) {
    lines.push(`${SEP}   <span foreground='${COLORS.peach}'>⚠️ ${provider.error}</span>`);
    return lines;
  }

  const maxLen = 20;

  // 5h Window as GPT-5.2 Codex
  if (provider.primary) {
    lines.push(formatModelLine('GPT-5.2 Codex', provider.primary.remaining, provider.primary.resetsAt, maxLen));
  }

  // Weekly
  if (provider.secondary) {
    lines.push(`${SEP}`);
    lines.push(`${SEP}   <span foreground='${COLORS.subtext}'>Weekly limit:</span>`);
    const pct = provider.secondary.remaining;
    lines.push(`${SEP}   ${getStatusIndicator(pct)} <span foreground='${COLORS.lavender}'>${'GPT-5.2 Codex'.padEnd(maxLen)}</span> ${formatBar(pct)} <span foreground='${getColorForPercent(pct)}'>${formatPct(pct).padStart(4)}</span> <span foreground='${COLORS.teal}'>→ ${formatEta(provider.secondary.resetsAt)} (${formatResetTime(provider.secondary.resetsAt)})</span>`);
  }

  return lines;
}

/**
 * Build Antigravity section - models merged (no Thinking suffix)
 */
function buildAntigravitySection(provider: ProviderQuota): string[] {
  const lines: string[] = [];
  
  const accountStr = provider.account ? ` <span foreground='${COLORS.subtext}'>(${provider.account})</span>` : '';
  lines.push(`${SEP} <span foreground='${COLORS.mauve}' weight='bold'>Antigravity${accountStr}</span>`);

  if (provider.error) {
    lines.push(`${SEP}   <span foreground='${COLORS.peach}'>⚠️ ${provider.error}</span>`);
    return lines;
  }

  if (!provider.models || Object.keys(provider.models).length === 0) {
    lines.push(`${SEP}   <span foreground='${COLORS.muted}'>No models available</span>`);
    return lines;
  }

  const models = filterAndMergeModels(provider.models);
  const maxLen = Math.max(...models.map(m => m.name.length), 20);

  for (const model of models) {
    lines.push(formatModelLine(model.name, model.remaining, model.resetsAt, maxLen));
  }

  return lines;
}

/**
 * Build tooltip
 */
function buildTooltip(quotas: AllQuotas): string {
  const sections: string[][] = [];

  for (const provider of quotas.providers) {
    if (!provider.available && !provider.error) continue;

    let section: string[];
    switch (provider.provider) {
      case 'claude': section = buildClaudeSection(provider); break;
      case 'codex': section = buildCodexSection(provider); break;
      case 'antigravity': section = buildAntigravitySection(provider); break;
      default: continue;
    }

    if (section.length > 0) sections.push(section);
  }

  return sections.map(s => s.join('\n')).join('\n\n');
}

/**
 * Build bar text - using provider abbreviations (icons need CSS)
 */
function buildText(quotas: AllQuotas): string {
  const parts: string[] = [];

  for (const provider of quotas.providers) {
    if (!provider.available) continue;

    const pct = provider.primary?.remaining ?? null;
    
    // Short names for bar
    let label = '';
    let iconColor = COLORS.text;
    switch (provider.provider) {
      case 'claude': label = 'Cld'; iconColor = COLORS.peach; break;
      case 'codex': label = 'Cdx'; iconColor = COLORS.green; break;
      case 'antigravity': label = 'AG'; iconColor = COLORS.blue; break;
      default: label = provider.provider.slice(0, 3);
    }
    
    parts.push(`<span foreground='${iconColor}'>${label}</span> ${formatPctSpan(pct)}`);
  }

  if (parts.length === 0) {
    return `<span foreground='${COLORS.muted}'>⚡ No Providers</span>`;
  }

  return `⚡ ${parts.join(` <span foreground='${COLORS.muted}'>│</span> `)}`;
}

/**
 * Format for Waybar
 */
export function formatForWaybar(quotas: AllQuotas): WaybarOutput {
  return {
    text: buildText(quotas),
    tooltip: buildTooltip(quotas),
    class: 'llm-usage',
  };
}

/**
 * Output Waybar JSON
 */
export function outputWaybar(quotas: AllQuotas): void {
  console.log(JSON.stringify(formatForWaybar(quotas)));
}
