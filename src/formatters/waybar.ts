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

// Box drawing - BOLD characters
const B = {
  tl: '┏',
  bl: '┗',
  lt: '┣',  // left tee for connecting labels
  h: '━',
  v: '┃',
  dot: '●',
  dotO: '○',
  diamond: '◆',
};

interface WaybarOutput {
  text: string;
  tooltip: string;
  class: string;
}

const s = (color: string, text: string, bold = false) => 
  `<span foreground='${color}'${bold ? " weight='bold'" : ''}>${text}</span>`;

function pct(val: number | null): string {
  return val === null ? '?%' : `${Math.round(val)}%`;
}

function pctColored(val: number | null): string {
  return s(getColorForPercent(val), pct(val));
}

function bar(val: number | null): string {
  if (val === null) return s(C.muted, '░'.repeat(20));
  const filled = Math.floor(val / 5);
  return s(getColorForPercent(val), '▰'.repeat(filled)) + s(C.muted, '▱'.repeat(20 - filled));
}

function eta(iso: string | null, remaining: number | null): string {
  // If full (100%), show "Full" instead of time
  if (remaining === 100) return 'Full';
  if (!iso) return '?';
  const diff = new Date(iso).getTime() - Date.now();
  if (diff < 0) return '0m';
  const d = Math.floor(diff / 86400000);
  const h = Math.floor((diff % 86400000) / 3600000);
  const m = Math.floor((diff % 3600000) / 60000);
  return d > 0 ? `${d}d ${h.toString().padStart(2, '0')}h` : `${h}h ${m.toString().padStart(2, '0')}m`;
}

function resetTime(iso: string | null, remaining: number | null): string {
  if (remaining === 100) return ''; // Full, no reset time needed
  if (!iso) return '??:??';
  const d = new Date(iso);
  return `(${d.getHours().toString().padStart(2, '0')}:${d.getMinutes().toString().padStart(2, '0')})`;
}

function indicator(val: number | null): string {
  if (val === null) return s(C.muted, B.dotO);
  if (val < 10) return s(C.red, B.dot);
  if (val < 30) return s(C.orange, B.dot);
  if (val < 60) return s(C.yellow, B.dot);
  return s(C.green, B.dot);
}

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

// Section label with connecting line: ┣━◆ Label
const label = (text: string, color: string) => 
  s(color, B.lt + B.h) + s(C.mauve, B.diamond + ' ' + text, true);

/**
 * Build Claude tooltip
 */
function buildClaudeTooltip(p: ProviderQuota): string {
  const lines: string[] = [];
  const v = s(C.peach, B.v);
  
  lines.push(s(C.peach, B.tl + B.h) + ' ' + s(C.peach, 'Claude', true) + ' ' + s(C.peach, B.h.repeat(50)));
  lines.push(v);
  
  if (p.error) {
    lines.push(v + '  ' + s(C.red, `⚠️ ${p.error}`));
  } else {
    const models = ['Opus', 'Sonnet', 'Haiku'];
    const maxLen = 20;
    
    if (p.primary) {
      lines.push(label('5-hour limit', C.peach));
      for (const m of models) {
        const name = s(C.lavender, m.padEnd(maxLen));
        const b = bar(p.primary.remaining);
        const pctS = s(getColorForPercent(p.primary.remaining), pct(p.primary.remaining).padStart(4));
        const etaS = s(C.teal, `→ ${eta(p.primary.resetsAt, p.primary.remaining)} ${resetTime(p.primary.resetsAt, p.primary.remaining)}`);
        lines.push(v + '  ' + indicator(p.primary.remaining) + ' ' + name + ' ' + b + ' ' + pctS + ' ' + etaS);
      }
    }

    if (p.secondary) {
      lines.push(v);
      lines.push(label('Weekly limit', C.peach));
      const name = s(C.lavender, 'All Models'.padEnd(20));
      const b = bar(p.secondary.remaining);
      const pctS = s(getColorForPercent(p.secondary.remaining), pct(p.secondary.remaining).padStart(4));
      const etaS = s(C.teal, `→ ${eta(p.secondary.resetsAt, p.secondary.remaining)} ${resetTime(p.secondary.resetsAt, p.secondary.remaining)}`);
      lines.push(v + '  ' + indicator(p.secondary.remaining) + ' ' + name + ' ' + b + ' ' + pctS + ' ' + etaS);
    }

    if (p.extraUsage?.enabled && p.extraUsage.limit > 0) {
      const { remaining, used, limit } = p.extraUsage;
      lines.push(v);
      lines.push(label('Extra Usage', C.peach));
      const name = s(C.lavender, 'Budget'.padEnd(20));
      const b = bar(remaining);
      const pctS = s(getColorForPercent(remaining), pct(remaining).padStart(4));
      const usedS = s(C.teal, `$${(used / 100).toFixed(2)}/$${(limit / 100).toFixed(2)}`);
      lines.push(v + '  ' + indicator(remaining) + ' ' + name + ' ' + b + ' ' + pctS + ' ' + usedS);
    }
  }
  
  lines.push(v);
  lines.push(s(C.peach, B.bl + B.h.repeat(55)));
  
  return lines.join('\n');
}

/**
 * Build Codex tooltip
 */
function buildCodexTooltip(p: ProviderQuota): string {
  const lines: string[] = [];
  const v = s(C.green, B.v);
  
  lines.push(s(C.green, B.tl + B.h) + ' ' + s(C.green, 'Codex', true) + ' ' + s(C.green, B.h.repeat(51)));
  lines.push(v);
  
  if (p.error) {
    lines.push(v + '  ' + s(C.red, `⚠️ ${p.error}`));
  } else {
    const maxLen = 20;
    
    if (p.primary) {
      lines.push(label('5-hour limit', C.green));
      const name = s(C.lavender, 'GPT-5.2 Codex'.padEnd(maxLen));
      const b = bar(p.primary.remaining);
      const pctS = s(getColorForPercent(p.primary.remaining), pct(p.primary.remaining).padStart(4));
      const etaS = s(C.teal, `→ ${eta(p.primary.resetsAt, p.primary.remaining)} ${resetTime(p.primary.resetsAt, p.primary.remaining)}`);
      lines.push(v + '  ' + indicator(p.primary.remaining) + ' ' + name + ' ' + b + ' ' + pctS + ' ' + etaS);
    }

    if (p.secondary) {
      lines.push(v);
      lines.push(label('Weekly limit', C.green));
      const name = s(C.lavender, 'GPT-5.2 Codex'.padEnd(20));
      const b = bar(p.secondary.remaining);
      const pctS = s(getColorForPercent(p.secondary.remaining), pct(p.secondary.remaining).padStart(4));
      const etaS = s(C.teal, `→ ${eta(p.secondary.resetsAt, p.secondary.remaining)} ${resetTime(p.secondary.resetsAt, p.secondary.remaining)}`);
      lines.push(v + '  ' + indicator(p.secondary.remaining) + ' ' + name + ' ' + b + ' ' + pctS + ' ' + etaS);
    }
  }
  
  lines.push(v);
  lines.push(s(C.green, B.bl + B.h.repeat(55)));
  
  return lines.join('\n');
}

/**
 * Build Antigravity tooltip
 */
function buildAntigravityTooltip(p: ProviderQuota): string {
  const lines: string[] = [];
  const v = s(C.blue, B.v);
  
  lines.push(s(C.blue, B.tl + B.h) + ' ' + s(C.blue, 'Antigravity', true) + ' ' + s(C.blue, B.h.repeat(45)));
  lines.push(v);
  
  if (p.error) {
    lines.push(v + '  ' + s(C.red, `⚠️ ${p.error}`));
  } else if (!p.models || Object.keys(p.models).length === 0) {
    lines.push(v + '  ' + s(C.muted, 'No models available'));
  } else {
    const models = filterModels(p.models);
    const maxLen = Math.max(...models.map(m => m.name.length), 20);

    lines.push(label('Available Models', C.blue));
    for (const m of models) {
      const name = s(C.lavender, m.name.padEnd(maxLen));
      const b = bar(m.remaining);
      const pctS = s(getColorForPercent(m.remaining), pct(m.remaining).padStart(4));
      const etaS = s(C.teal, `→ ${eta(m.resetsAt, m.remaining)} ${resetTime(m.resetsAt, m.remaining)}`);
      lines.push(v + '  ' + indicator(m.remaining) + ' ' + name + ' ' + b + ' ' + pctS + ' ' + etaS);
    }
  }
  
  lines.push(v);
  lines.push(s(C.blue, B.bl + B.h.repeat(55)));
  
  return lines.join('\n');
}

function buildTooltip(quotas: AllQuotas): string {
  const sections: string[] = [];

  for (const p of quotas.providers) {
    if (!p.available && !p.error) continue;
    
    switch (p.provider) {
      case 'claude': sections.push(buildClaudeTooltip(p)); break;
      case 'codex': sections.push(buildCodexTooltip(p)); break;
      case 'antigravity': sections.push(buildAntigravityTooltip(p)); break;
    }
  }

  return sections.join('\n\n');
}

function buildText(quotas: AllQuotas): string {
  const parts: string[] = [];

  for (const p of quotas.providers) {
    if (!p.available) continue;
    const val = p.primary?.remaining ?? null;
    parts.push(pctColored(val));
  }

  if (parts.length === 0) return s(C.muted, 'No Providers');
  return parts.join(' ' + s(C.muted, '│') + ' ');
}

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
    case 'claude': tooltip = buildClaudeTooltip(quota); break;
    case 'codex': tooltip = buildCodexTooltip(quota); break;
    case 'antigravity': tooltip = buildAntigravityTooltip(quota); break;
  }
  
  return {
    text: pctColored(val),
    tooltip,
    class: `qbar-${quota.provider} ${status}`,
  };
}
