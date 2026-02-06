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
  text: '#cdd6f4',        // Main text
  subtext: '#bac2de',     // Secondary text
  muted: '#6c7086',       // Dimmed/empty
  surface: '#45475a',     // Borders/separators
  
  // Accent colors for differentiation
  lavender: '#b4befe',    // Model names
  teal: '#94e2d5',        // Reset times/ETA
  pink: '#f5c2e7',        // Group headers
  blue: '#89b4fa',        // Limits/Extra
  mauve: '#cba6f7',       // Provider names
  peach: '#fab387',       // Warnings
  sky: '#89dceb',         // Percentages accent
  sapphire: '#74c7ec',    // Timeline bars
} as const;

// Nerd Font icons for providers
const PROVIDER_ICONS: Record<string, string> = {
  claude: '',      // Anthropic/brain icon
  codex: '',       // Terminal/code icon  
  antigravity: '󰊤', // Google icon
};

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
 * Format percentage with color span for Pango markup (bar text)
 */
function formatPctSpan(pct: number | null): string {
  const color = getColorForPercent(pct);
  return `<span foreground='${color}'>${formatPct(pct)}</span>`;
}

/**
 * Generate a 20-character progress bar with dots style
 */
function formatBar(pct: number | null): string {
  if (pct === null) {
    return `<span foreground='${COLORS.muted}'>░░░░░░░░░░░░░░░░░░░░</span>`;
  }

  const filled = Math.floor(pct / 5);
  const empty = 20 - filled;
  const color = getColorForPercent(pct);

  const filledStr = '▰'.repeat(filled);
  const emptyStr = '▱'.repeat(empty);

  return `<span foreground='${color}'>${filledStr}</span><span foreground='${COLORS.muted}'>${emptyStr}</span>`;
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
 * Get status indicator based on percentage
 */
function getStatusIndicator(pct: number | null): string {
  if (pct === null) return `<span foreground='${COLORS.muted}'>○</span>`;
  if (pct <= 0) return `<span foreground='${COLORS.red}'>●</span>`;
  if (pct < 10) return `<span foreground='${COLORS.red}'>●</span>`;
  if (pct < 30) return `<span foreground='${COLORS.orange}'>●</span>`;
  if (pct < 60) return `<span foreground='${COLORS.yellow}'>●</span>`;
  return `<span foreground='${COLORS.green}'>●</span>`;
}

/**
 * Timeline separator (vertical bar)
 */
function timelineSep(): string {
  return `<span foreground='${COLORS.sapphire}'>│</span>`;
}

/**
 * Filter out internal/test models and Gemini 2.5
 */
function isValidModel(name: string): boolean {
  const lowerName = name.toLowerCase();
  
  // Exclude patterns
  const excludePatterns = [
    /^tab_/i,
    /^chat_/i,
    /^test_/i,
    /^internal_/i,
    /preview$/i,
    /2\.5/i,  // Exclude Gemini 2.5 models
  ];
  
  return !excludePatterns.some(p => p.test(lowerName));
}

/**
 * Format a single model line with individual reset time
 */
function formatModelLine(model: ModelEntry, maxNameLen: number): string {
  const indicator = getStatusIndicator(model.remaining);
  const name = `<span foreground='${COLORS.lavender}'>${model.name.padEnd(maxNameLen)}</span>`;
  const bar = formatBar(model.remaining);
  
  const pctColor = getColorForPercent(model.remaining);
  const pctStr = `<span foreground='${pctColor}'>${formatPct(model.remaining).padStart(4)}</span>`;
  
  const eta = `<span foreground='${COLORS.teal}'>→ ${formatEta(model.resetsAt)} (${formatResetTime(model.resetsAt)})</span>`;

  return `${indicator} ${name} ${bar} ${pctStr} ${eta}`;
}

/**
 * Build Claude section with windows
 */
function buildClaudeSection(provider: ProviderQuota): string[] {
  const lines: string[] = [];
  const sep = timelineSep();
  
  // Header with icon
  const icon = `<span foreground='${COLORS.peach}'>${PROVIDER_ICONS.claude}</span>`;
  const planStr = provider.plan ? ` <span foreground='${COLORS.subtext}'>(${provider.plan})</span>` : '';
  lines.push(`${sep} ${icon} <span foreground='${COLORS.mauve}' weight='bold'>${provider.displayName}${planStr}</span>`);

  if (provider.error) {
    lines.push(`${sep}   <span foreground='${COLORS.peach}'>⚠️ ${provider.error}</span>`);
    return lines;
  }

  // 5h Window
  if (provider.primary) {
    const pct = provider.primary.remaining;
    const indicator = getStatusIndicator(pct);
    const name = `<span foreground='${COLORS.lavender}'>5h Window</span>`;
    const bar = formatBar(pct);
    const pctColor = getColorForPercent(pct);
    const pctStr = `<span foreground='${pctColor}'>${formatPct(pct).padStart(4)}</span>`;
    const eta = `<span foreground='${COLORS.teal}'>→ ${formatEta(provider.primary.resetsAt)} (${formatResetTime(provider.primary.resetsAt)})</span>`;
    
    lines.push(`${sep}   ${indicator} ${name.padEnd(40)} ${bar} ${pctStr} ${eta}`);
  }

  // 7-day Window
  if (provider.secondary) {
    const pct = provider.secondary.remaining;
    const indicator = getStatusIndicator(pct);
    const name = `<span foreground='${COLORS.lavender}'>Weekly</span>`;
    const bar = formatBar(pct);
    const pctColor = getColorForPercent(pct);
    const pctStr = `<span foreground='${pctColor}'>${formatPct(pct).padStart(4)}</span>`;
    const eta = `<span foreground='${COLORS.teal}'>→ ${formatEta(provider.secondary.resetsAt)} (${formatResetTime(provider.secondary.resetsAt)})</span>`;
    
    lines.push(`${sep}   ${indicator} ${name.padEnd(40)} ${bar} ${pctStr} ${eta}`);
  }

  // Extra Usage
  if (provider.extraUsage?.enabled) {
    const pct = provider.extraUsage.remaining;
    const used = provider.extraUsage.used;
    const limit = provider.extraUsage.limit;
    
    const indicator = getStatusIndicator(pct);
    const name = `<span foreground='${COLORS.blue}'>Extra Usage</span>`;
    const bar = formatBar(pct);
    const pctColor = getColorForPercent(pct);
    const pctStr = `<span foreground='${pctColor}'>${formatPct(pct).padStart(4)}</span>`;
    const usedStr = `<span foreground='${COLORS.subtext}'>$${(used / 100).toFixed(2)}/$${(limit / 100).toFixed(2)}</span>`;
    
    lines.push(`${sep}   ${indicator} ${name.padEnd(40)} ${bar} ${pctStr} ${usedStr}`);
  }

  return lines;
}

/**
 * Build Codex section
 */
function buildCodexSection(provider: ProviderQuota): string[] {
  const lines: string[] = [];
  const sep = timelineSep();
  
  // Header with icon
  const icon = `<span foreground='${COLORS.green}'>${PROVIDER_ICONS.codex}</span>`;
  lines.push(`${sep} ${icon} <span foreground='${COLORS.mauve}' weight='bold'>${provider.displayName}</span>`);

  if (provider.error) {
    lines.push(`${sep}   <span foreground='${COLORS.peach}'>⚠️ ${provider.error}</span>`);
    return lines;
  }

  // 5h Window
  if (provider.primary) {
    const pct = provider.primary.remaining;
    const indicator = getStatusIndicator(pct);
    const name = `<span foreground='${COLORS.lavender}'>5h Window</span>`;
    const bar = formatBar(pct);
    const pctColor = getColorForPercent(pct);
    const pctStr = `<span foreground='${pctColor}'>${formatPct(pct).padStart(4)}</span>`;
    const eta = `<span foreground='${COLORS.teal}'>→ ${formatEta(provider.primary.resetsAt)} (${formatResetTime(provider.primary.resetsAt)})</span>`;
    
    lines.push(`${sep}   ${indicator} ${name.padEnd(40)} ${bar} ${pctStr} ${eta}`);
  }

  // Weekly
  if (provider.secondary) {
    const pct = provider.secondary.remaining;
    const indicator = getStatusIndicator(pct);
    const name = `<span foreground='${COLORS.lavender}'>Weekly</span>`;
    const bar = formatBar(pct);
    const pctColor = getColorForPercent(pct);
    const pctStr = `<span foreground='${pctColor}'>${formatPct(pct).padStart(4)}</span>`;
    const eta = `<span foreground='${COLORS.teal}'>→ ${formatEta(provider.secondary.resetsAt)} (${formatResetTime(provider.secondary.resetsAt)})</span>`;
    
    lines.push(`${sep}   ${indicator} ${name.padEnd(40)} ${bar} ${pctStr} ${eta}`);
  }

  return lines;
}

/**
 * Build Antigravity section with individual model times
 */
function buildAntigravitySection(provider: ProviderQuota): string[] {
  const lines: string[] = [];
  const sep = timelineSep();
  
  // Header with icon
  const icon = `<span foreground='${COLORS.blue}'>${PROVIDER_ICONS.antigravity}</span>`;
  const accountStr = provider.account ? ` <span foreground='${COLORS.subtext}'>(${provider.account})</span>` : '';
  lines.push(`${sep} ${icon} <span foreground='${COLORS.mauve}' weight='bold'>${provider.displayName}${accountStr}</span>`);

  if (provider.error) {
    lines.push(`${sep}   <span foreground='${COLORS.peach}'>⚠️ ${provider.error}</span>`);
    return lines;
  }

  if (!provider.models || Object.keys(provider.models).length === 0) {
    lines.push(`${sep}   <span foreground='${COLORS.muted}'>No models available</span>`);
    return lines;
  }

  // Convert models to entries and filter
  const modelEntries: ModelEntry[] = Object.entries(provider.models)
    .filter(([name]) => isValidModel(name))
    .map(([name, window]) => ({
      name,
      remaining: window.remaining,
      resetsAt: window.resetsAt,
    }));

  // Sort models: Claude first, then GPT, then Gemini, then alphabetically
  modelEntries.sort((a, b) => {
    const aLower = a.name.toLowerCase();
    const bLower = b.name.toLowerCase();
    
    const getPriority = (name: string): number => {
      if (name.includes('claude')) return 0;
      if (name.includes('gpt')) return 1;
      if (name.includes('gemini')) return 2;
      return 3;
    };
    
    const aPriority = getPriority(aLower);
    const bPriority = getPriority(bLower);
    
    if (aPriority !== bPriority) return aPriority - bPriority;
    return a.name.localeCompare(b.name);
  });

  // Find max name length for alignment
  const maxNameLen = Math.max(...modelEntries.map(m => m.name.length), 20);

  // Add each model with individual reset time
  for (const model of modelEntries) {
    lines.push(`${sep}   ${formatModelLine(model, maxNameLen)}`);
  }

  return lines;
}

/**
 * Build tooltip content from all quotas
 */
function buildTooltip(quotas: AllQuotas): string {
  const sections: string[][] = [];

  for (const provider of quotas.providers) {
    if (!provider.available && !provider.error) continue;

    let section: string[];
    
    switch (provider.provider) {
      case 'claude':
        section = buildClaudeSection(provider);
        break;
      case 'codex':
        section = buildCodexSection(provider);
        break;
      case 'antigravity':
        section = buildAntigravitySection(provider);
        break;
      default:
        continue;
    }

    if (section.length > 0) {
      sections.push(section);
    }
  }

  // Join sections with spacing
  return sections.map(s => s.join('\n')).join('\n\n');
}

/**
 * Build bar text from all quotas with icons
 */
function buildText(quotas: AllQuotas): string {
  const parts: string[] = [];

  for (const provider of quotas.providers) {
    if (!provider.available) continue;

    const pct = provider.primary?.remaining ?? null;
    const icon = PROVIDER_ICONS[provider.provider] || '';
    const pctSpan = formatPctSpan(pct);
    
    // Icon colored by provider
    let iconColor = COLORS.text;
    if (provider.provider === 'claude') iconColor = COLORS.peach;
    if (provider.provider === 'codex') iconColor = COLORS.green;
    if (provider.provider === 'antigravity') iconColor = COLORS.blue;
    
    const iconSpan = `<span foreground='${iconColor}'>${icon}</span>`;
    parts.push(`${iconSpan} ${pctSpan}`);
  }

  if (parts.length === 0) {
    return `<span foreground='${COLORS.muted}'>⚡ No Providers</span>`;
  }

  const sep = `<span foreground='${COLORS.muted}'>│</span>`;
  return `⚡ ${parts.join(` ${sep} `)}`;
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
