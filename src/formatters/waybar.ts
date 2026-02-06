import { CONFIG, getColorForPercent } from '../config';
import type { AllQuotas, ProviderQuota, QuotaWindow } from '../providers/types';

// Catppuccin Mocha palette
const C = {
  green: '#a6e3a1',
  yellow: '#f9e2af',
  orange: '#fab387',
  red: '#f38ba8',
  text: '#cdd6f4',
  subtext: '#bac2de',
  muted: '#6c7086',
  lavender: '#b4befe',
  teal: '#94e2d5',
  blue: '#89b4fa',
  mauve: '#cba6f7',
  peach: '#fab387',
  sapphire: '#74c7ec',
  pink: '#f5c2e7',
  sky: '#89dceb',
} as const;

// Box drawing characters
const LINE = {
  h: '─',      // horizontal
  hBold: '━',  // horizontal bold
  v: '┃',      // vertical bold  
  tl: '╭',     // top left corner
  tr: '╮',     // top right corner
  bl: '╰',     // bottom left corner
  br: '╯',     // bottom right corner
};

interface WaybarOutput {
  text: string;
  tooltip: string;
  class: string;
}

/**
 * Format percentage without decimals
 */
function pct(val: number | null): string {
  return val === null ? '?%' : `${Math.round(val)}%`;
}

/**
 * Colored percentage
 */
function pctColored(val: number | null): string {
  return `<span foreground='${getColorForPercent(val)}'>${pct(val)}</span>`;
}

/**
 * Progress bar (20 chars)
 */
function bar(val: number | null): string {
  if (val === null) return `<span foreground='${C.muted}'>${'░'.repeat(20)}</span>`;
  const filled = Math.floor(val / 5);
  return `<span foreground='${getColorForPercent(val)}'>${'▰'.repeat(filled)}</span><span foreground='${C.muted}'>${'▱'.repeat(20 - filled)}</span>`;
}

/**
 * Time until reset: >24h = "Xd XXh", <24h = "XXh XXm"
 */
function eta(iso: string | null): string {
  if (!iso) return '?';
  const diff = new Date(iso).getTime() - Date.now();
  if (diff < 0) return '0m';
  const d = Math.floor(diff / 86400000);
  const h = Math.floor((diff % 86400000) / 3600000);
  const m = Math.floor((diff % 3600000) / 60000);
  return d > 0 ? `${d}d ${h.toString().padStart(2, '0')}h` : `${h}h ${m.toString().padStart(2, '0')}m`;
}

/**
 * Reset time as HH:MM
 */
function resetTime(iso: string | null): string {
  if (!iso) return '??:??';
  const d = new Date(iso);
  return `${d.getHours().toString().padStart(2, '0')}:${d.getMinutes().toString().padStart(2, '0')}`;
}

/**
 * Status indicator
 */
function indicator(val: number | null): string {
  if (val === null) return `<span foreground='${C.muted}'>○</span>`;
  if (val < 10) return `<span foreground='${C.red}'>●</span>`;
  if (val < 30) return `<span foreground='${C.orange}'>●</span>`;
  if (val < 60) return `<span foreground='${C.yellow}'>●</span>`;
  return `<span foreground='${C.green}'>●</span>`;
}

/**
 * Vertical bar (timeline)
 */
const V = `<span foreground='${C.sapphire}'>${LINE.v}</span>`;

/**
 * Create header with horizontal lines: ──── Name ────
 */
function header(name: string, lineColor: string = C.sapphire, nameColor: string = C.mauve): string {
  const line = `<span foreground='${lineColor}'>${LINE.h.repeat(5)}</span>`;
  const nameSpan = `<span foreground='${nameColor}' weight='bold'>${name}</span>`;
  return `${V} ${line} ${nameSpan} ${line}`;
}

/**
 * Filter and merge models
 */
function filterModels(models: Record<string, QuotaWindow>): Array<{name: string, remaining: number | null, resetsAt: string | null}> {
  const map = new Map<string, {name: string, remaining: number | null, resetsAt: string | null}>();
  
  for (const [name, w] of Object.entries(models)) {
    const lower = name.toLowerCase();
    if (/^(tab_|chat_|test_|internal_)/.test(lower) || /preview$/i.test(name) || /2\.5/i.test(name)) continue;
    const base = name.replace(/\s*\(Thinking\)/i, '');
    if (!map.has(base)) map.set(base, { name: base, remaining: w.remaining, resetsAt: w.resetsAt });
  }
  
  return [...map.values()].sort((a, b) => {
    const pri = (n: string) => n.toLowerCase().includes('claude') ? 0 : n.toLowerCase().includes('gpt') ? 1 : n.toLowerCase().includes('gemini') ? 2 : 3;
    return pri(a.name) - pri(b.name) || a.name.localeCompare(b.name);
  });
}

/**
 * Format model line
 */
function modelLine(name: string, val: number | null, reset: string | null, maxLen: number): string {
  const namePad = `<span foreground='${C.lavender}'>${name.padEnd(maxLen)}</span>`;
  const etaStr = `<span foreground='${C.teal}'>→ ${eta(reset)} (${resetTime(reset)})</span>`;
  return `${V}   ${indicator(val)} ${namePad} ${bar(val)} ${pctColored(val).padStart(4)} ${etaStr}`;
}

/**
 * Build Claude section
 */
function buildClaude(p: ProviderQuota): string[] {
  const lines: string[] = [];
  
  lines.push(header('Claude', C.peach, C.peach));
  
  if (p.error) {
    lines.push(`${V}   <span foreground='${C.peach}'>⚠️ ${p.error}</span>`);
    return lines;
  }

  const maxLen = 20;
  const models = ['Opus', 'Sonnet', 'Haiku'];
  
  if (p.primary) {
    for (const m of models) {
      lines.push(modelLine(m, p.primary.remaining, p.primary.resetsAt, maxLen));
    }
  }

  if (p.secondary) {
    lines.push(`${V}`);
    lines.push(`${V}   <span foreground='${C.subtext}'>Weekly:</span>`);
    lines.push(modelLine('All Models', p.secondary.remaining, p.secondary.resetsAt, maxLen));
  }

  if (p.extraUsage?.enabled) {
    const { remaining, used, limit } = p.extraUsage;
    lines.push(`${V}`);
    const namePad = `<span foreground='${C.blue}'>${'Extra Usage'.padEnd(maxLen)}</span>`;
    const usedStr = `<span foreground='${C.subtext}'>$${(used / 100).toFixed(2)}/$${(limit / 100).toFixed(2)}</span>`;
    lines.push(`${V}   ${indicator(remaining)} ${namePad} ${bar(remaining)} ${pctColored(remaining).padStart(4)} ${usedStr}`);
  }

  return lines;
}

/**
 * Build Codex section
 */
function buildCodex(p: ProviderQuota): string[] {
  const lines: string[] = [];
  
  lines.push(header('Codex', C.green, C.green));
  
  if (p.error) {
    lines.push(`${V}   <span foreground='${C.peach}'>⚠️ ${p.error}</span>`);
    return lines;
  }

  const maxLen = 20;
  
  if (p.primary) {
    lines.push(modelLine('GPT-5.2 Codex', p.primary.remaining, p.primary.resetsAt, maxLen));
  }

  if (p.secondary) {
    lines.push(`${V}`);
    lines.push(`${V}   <span foreground='${C.subtext}'>Weekly:</span>`);
    lines.push(modelLine('GPT-5.2 Codex', p.secondary.remaining, p.secondary.resetsAt, maxLen));
  }

  return lines;
}

/**
 * Build Antigravity section
 */
function buildAntigravity(p: ProviderQuota): string[] {
  const lines: string[] = [];
  
  lines.push(header('Antigravity', C.blue, C.blue));
  
  if (p.error) {
    lines.push(`${V}   <span foreground='${C.peach}'>⚠️ ${p.error}</span>`);
    return lines;
  }

  if (!p.models || Object.keys(p.models).length === 0) {
    lines.push(`${V}   <span foreground='${C.muted}'>No models available</span>`);
    return lines;
  }

  const models = filterModels(p.models);
  const maxLen = Math.max(...models.map(m => m.name.length), 20);

  for (const m of models) {
    lines.push(modelLine(m.name, m.remaining, m.resetsAt, maxLen));
  }

  return lines;
}

/**
 * Build full tooltip
 */
function buildTooltip(quotas: AllQuotas): string {
  const sections: string[][] = [];

  for (const p of quotas.providers) {
    if (!p.available && !p.error) continue;
    
    switch (p.provider) {
      case 'claude': sections.push(buildClaude(p)); break;
      case 'codex': sections.push(buildCodex(p)); break;
      case 'antigravity': sections.push(buildAntigravity(p)); break;
    }
  }

  return sections.map(s => s.join('\n')).join(`\n${V}\n`);
}

/**
 * Build bar text - simple text, icons will be added via CSS
 */
function buildText(quotas: AllQuotas): string {
  const parts: string[] = [];

  for (const p of quotas.providers) {
    if (!p.available) continue;
    const val = p.primary?.remaining ?? null;
    parts.push(pctColored(val));
  }

  if (parts.length === 0) return `<span foreground='${C.muted}'>No Providers</span>`;
  return parts.join(` <span foreground='${C.muted}'>│</span> `);
}

/**
 * Get CSS class based on provider states
 */
function getClass(quotas: AllQuotas): string {
  const classes: string[] = ['llm-usage'];
  
  for (const p of quotas.providers) {
    if (!p.available) continue;
    const val = p.primary?.remaining ?? 100;
    let status = 'ok';
    if (val < 10) status = 'critical';
    else if (val < 30) status = 'warn';
    else if (val < 60) status = 'low';
    classes.push(`${p.provider}-${status}`);
  }
  
  return classes.join(' ');
}

export function formatForWaybar(quotas: AllQuotas): WaybarOutput {
  return { 
    text: buildText(quotas), 
    tooltip: buildTooltip(quotas), 
    class: getClass(quotas),
  };
}

export function outputWaybar(quotas: AllQuotas): void {
  console.log(JSON.stringify(formatForWaybar(quotas)));
}

/**
 * Output for individual provider module (used by qbar --provider X)
 */
export function formatProviderForWaybar(quota: ProviderQuota): WaybarOutput {
  const val = quota.primary?.remaining ?? null;
  let status = 'ok';
  if (val !== null) {
    if (val < 10) status = 'critical';
    else if (val < 30) status = 'warn';
    else if (val < 60) status = 'low';
  }
  
  let tooltip = '';
  switch (quota.provider) {
    case 'claude': tooltip = buildClaude(quota).join('\n'); break;
    case 'codex': tooltip = buildCodex(quota).join('\n'); break;
    case 'antigravity': tooltip = buildAntigravity(quota).join('\n'); break;
  }
  
  return {
    text: pctColored(val),
    tooltip,
    class: `qbar-${quota.provider} ${status}`,
  };
}
