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
  limit?: string;
  isExhausted?: boolean;
}

interface QuotaGroup {
  label: string;
  resetsAt: string | null;
  eta: string;
  models: ModelEntry[];
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
 * Generate a 20-character progress bar with dots style
 */
function formatBar(pct: number | null): string {
  if (pct === null) {
    return `<span foreground='${COLORS.muted}'>░░░░░░░░░░░░░░░░░░░░</span>`;
  }

  const filled = Math.floor(pct / 5);
  const empty = 20 - filled;
  const color = getColorForPercent(pct);

  // Use dots style like the reference image
  const filledStr = '▰'.repeat(filled);
  const emptyStr = '▱'.repeat(empty);

  return `<span foreground='${color}'>${filledStr}</span><span foreground='${COLORS.muted}'>${emptyStr}</span>`;
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
    return `${days}d ${hours}h ${minutes}m`;
  }
  if (hours > 0) {
    return `${hours}h ${minutes}m`;
  }
  return `${minutes}m`;
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
 * Format a single model line with proper alignment and colors
 */
function formatModelLine(model: ModelEntry, maxNameLen: number): string {
  const indicator = getStatusIndicator(model.remaining);
  const name = `<span foreground='${COLORS.lavender}'>${model.name.padEnd(maxNameLen)}</span>`;
  const bar = formatBar(model.remaining);
  
  const pctColor = getColorForPercent(model.remaining);
  const pctStr = model.remaining !== null 
    ? `${model.remaining.toFixed(2).padStart(6)}%`
    : '     ?%';
  const pct = `<span foreground='${pctColor}'>${pctStr}</span>`;

  return `${indicator} ${name} ${bar} ${pct}`;
}

/**
 * Filter out internal/test models
 */
function isValidModel(name: string): boolean {
  const excludePatterns = [
    /^tab_/i,
    /^chat_/i,
    /^test_/i,
    /^internal_/i,
    /preview$/i,
  ];
  return !excludePatterns.some(p => p.test(name));
}

/**
 * Generate a descriptive group label based on models
 */
function getGroupLabel(models: ModelEntry[]): string {
  // Check what types of models are in this group
  const hasClaude = models.some(m => m.name.toLowerCase().includes('claude'));
  const hasGemini = models.some(m => m.name.toLowerCase().includes('gemini'));
  const hasGpt = models.some(m => m.name.toLowerCase().includes('gpt'));
  
  const parts: string[] = [];
  if (hasClaude) parts.push('Claude');
  if (hasGpt) parts.push('GPT-OSS');
  if (hasGemini) parts.push('Gemini');
  
  if (parts.length === 0) return 'Other';
  if (parts.length === 1) return parts[0];
  return parts.join(' + ');
}

/**
 * Group models by their reset time
 */
function groupByResetTime(models: ModelEntry[]): QuotaGroup[] {
  // Filter out internal/test models
  const validModels = models.filter(m => isValidModel(m.name));
  
  const groups = new Map<string, ModelEntry[]>();
  
  for (const model of validModels) {
    const key = model.resetsAt || 'unknown';
    if (!groups.has(key)) {
      groups.set(key, []);
    }
    groups.get(key)!.push(model);
  }

  // Sort groups by reset time (soonest first), unknown last
  const sortedKeys = [...groups.keys()].sort((a, b) => {
    if (a === 'unknown') return 1;
    if (b === 'unknown') return -1;
    return new Date(a).getTime() - new Date(b).getTime();
  });

  return sortedKeys.map((key) => {
    const groupModels = groups.get(key)!;
    const resetsAt = key === 'unknown' ? null : key;
    const eta = formatEta(resetsAt);
    const resetTime = formatResetTime(resetsAt);
    
    // Sort models within group (Claude first, then GPT, then Gemini, then alphabetically)
    groupModels.sort((a, b) => {
      const aLower = a.name.toLowerCase();
      const bLower = b.name.toLowerCase();
      
      // Priority: Claude > GPT > Gemini > Others
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
    
    // Generate group label
    let label: string;
    if (resetsAt === null) {
      label = 'Unknown Reset';
    } else if (groupModels.length === 1) {
      // Single model - use model name as group
      label = groupModels[0].name;
    } else {
      // Multiple models - generate descriptive label
      label = getGroupLabel(groupModels);
    }

    return {
      label,
      resetsAt,
      eta: `→ ${eta} (${resetTime})`,
      models: groupModels,
    };
  });
}

/**
 * Build Claude section with windows
 */
function buildClaudeSection(provider: ProviderQuota): string[] {
  const lines: string[] = [];
  
  // Header
  const planStr = provider.plan ? ` <span foreground='${COLORS.subtext}'>(${provider.plan})</span>` : '';
  lines.push(`<span foreground='${COLORS.mauve}' weight='bold'>━━━ ${provider.displayName}${planStr} ━━━</span>`);

  if (provider.error) {
    lines.push(`<span foreground='${COLORS.peach}'>⚠️ ${provider.error}</span>`);
    return lines;
  }

  // 5h Window
  if (provider.primary) {
    const pct = provider.primary.remaining;
    const indicator = getStatusIndicator(pct);
    const name = `<span foreground='${COLORS.lavender}'>5h Window       </span>`;
    const bar = formatBar(pct);
    const pctColor = getColorForPercent(pct);
    const pctStr = `<span foreground='${pctColor}'>${pct.toFixed(2).padStart(6)}%</span>`;
    const eta = `<span foreground='${COLORS.teal}'>→ ${formatEta(provider.primary.resetsAt)} (${formatResetTime(provider.primary.resetsAt)})</span>`;
    
    lines.push(`${indicator} ${name} ${bar} ${pctStr} ${eta}`);
  }

  // 7-day Window
  if (provider.secondary) {
    const pct = provider.secondary.remaining;
    const indicator = getStatusIndicator(pct);
    const name = `<span foreground='${COLORS.lavender}'>Weekly          </span>`;
    const bar = formatBar(pct);
    const pctColor = getColorForPercent(pct);
    const pctStr = `<span foreground='${pctColor}'>${pct.toFixed(2).padStart(6)}%</span>`;
    const eta = `<span foreground='${COLORS.teal}'>→ ${formatEta(provider.secondary.resetsAt)} (${formatResetTime(provider.secondary.resetsAt)})</span>`;
    
    lines.push(`${indicator} ${name} ${bar} ${pctStr} ${eta}`);
  }

  // Extra Usage
  if (provider.extraUsage?.enabled) {
    const pct = provider.extraUsage.remaining;
    const used = provider.extraUsage.used;
    const limit = provider.extraUsage.limit;
    
    const indicator = getStatusIndicator(pct);
    const name = `<span foreground='${COLORS.blue}'>Extra Usage     </span>`;
    const bar = formatBar(pct);
    const pctColor = getColorForPercent(pct);
    const pctStr = `<span foreground='${pctColor}'>${pct.toFixed(2).padStart(6)}%</span>`;
    const usedStr = `<span foreground='${COLORS.subtext}'>$${(used / 100).toFixed(2)}/$${(limit / 100).toFixed(2)}</span>`;
    
    lines.push(`${indicator} ${name} ${bar} ${pctStr} ${usedStr}`);
  }

  return lines;
}

/**
 * Build Codex section
 */
function buildCodexSection(provider: ProviderQuota): string[] {
  const lines: string[] = [];
  
  // Header
  lines.push(`<span foreground='${COLORS.mauve}' weight='bold'>━━━ ${provider.displayName} ━━━</span>`);

  if (provider.error) {
    lines.push(`<span foreground='${COLORS.peach}'>⚠️ ${provider.error}</span>`);
    return lines;
  }

  // 5h Window
  if (provider.primary) {
    const pct = provider.primary.remaining;
    const indicator = getStatusIndicator(pct);
    const name = `<span foreground='${COLORS.lavender}'>5h Window       </span>`;
    const bar = formatBar(pct);
    const pctColor = getColorForPercent(pct);
    const pctStr = `<span foreground='${pctColor}'>${pct.toFixed(2).padStart(6)}%</span>`;
    const eta = `<span foreground='${COLORS.teal}'>→ ${formatEta(provider.primary.resetsAt)} (${formatResetTime(provider.primary.resetsAt)})</span>`;
    
    lines.push(`${indicator} ${name} ${bar} ${pctStr} ${eta}`);
  }

  // Weekly
  if (provider.secondary) {
    const pct = provider.secondary.remaining;
    const indicator = getStatusIndicator(pct);
    const name = `<span foreground='${COLORS.lavender}'>Weekly          </span>`;
    const bar = formatBar(pct);
    const pctColor = getColorForPercent(pct);
    const pctStr = `<span foreground='${pctColor}'>${pct.toFixed(2).padStart(6)}%</span>`;
    const eta = `<span foreground='${COLORS.teal}'>→ ${formatEta(provider.secondary.resetsAt)} (${formatResetTime(provider.secondary.resetsAt)})</span>`;
    
    lines.push(`${indicator} ${name} ${bar} ${pctStr} ${eta}`);
  }

  return lines;
}

/**
 * Build Antigravity section with grouped models
 */
function buildAntigravitySection(provider: ProviderQuota): string[] {
  const lines: string[] = [];
  
  // Header
  const accountStr = provider.account ? ` <span foreground='${COLORS.subtext}'>(${provider.account})</span>` : '';
  lines.push(`<span foreground='${COLORS.mauve}' weight='bold'>━━━ ${provider.displayName}${accountStr} ━━━</span>`);

  if (provider.error) {
    lines.push(`<span foreground='${COLORS.peach}'>⚠️ ${provider.error}</span>`);
    return lines;
  }

  if (!provider.models || Object.keys(provider.models).length === 0) {
    lines.push(`<span foreground='${COLORS.muted}'>No models available</span>`);
    return lines;
  }

  // Convert models to entries
  const modelEntries: ModelEntry[] = Object.entries(provider.models).map(([name, window]) => ({
    name,
    remaining: window.remaining,
    resetsAt: window.resetsAt,
  }));

  // Group by reset time
  const groups = groupByResetTime(modelEntries);
  
  // Find max name length for alignment
  const maxNameLen = Math.max(...modelEntries.map(m => m.name.length), 16);

  for (let i = 0; i < groups.length; i++) {
    const group = groups[i];
    
    if (i > 0) {
      // Separator between groups
      lines.push('');
    }

    // Group header with reset info
    if (group.models.length > 1 || group.label.startsWith('Group')) {
      const headerColor = COLORS.pink;
      lines.push(`<span foreground='${headerColor}'>${group.label}</span> <span foreground='${COLORS.teal}'>${group.eta}</span>`);
      
      // Model lines
      for (const model of group.models) {
        lines.push('  ' + formatModelLine(model, maxNameLen));
      }
    } else {
      // Single model - inline with reset time
      const model = group.models[0];
      const indicator = getStatusIndicator(model.remaining);
      const name = `<span foreground='${COLORS.lavender}'>${model.name.padEnd(maxNameLen)}</span>`;
      const bar = formatBar(model.remaining);
      const pctColor = getColorForPercent(model.remaining);
      const pctStr = model.remaining !== null 
        ? `${model.remaining.toFixed(2).padStart(6)}%`
        : '     ?%';
      const pct = `<span foreground='${pctColor}'>${pctStr}</span>`;
      const eta = `<span foreground='${COLORS.teal}'>${group.eta}</span>`;

      lines.push(`${indicator} ${name} ${bar} ${pct} ${eta}`);
    }
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
 * Build bar text from all quotas
 */
function buildText(quotas: AllQuotas): string {
  const parts: string[] = [];
  const sep = `<span foreground='${COLORS.muted}'>│</span>`;

  for (const provider of quotas.providers) {
    if (!provider.available) continue;

    const pct = provider.primary?.remaining ?? null;
    const abbrev = getAbbrev(provider.provider);
    parts.push(formatPctSpan(abbrev, pct));
  }

  if (parts.length === 0) {
    return `<span foreground='${COLORS.muted}'>⚡ No Providers</span>`;
  }

  return `⚡ ${parts.join(` ${sep} `)}`;
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
