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
} as const;

// Nerd Font icons (portable, no external files needed)
const ICONS = {
  claude: '',      // Brain
  codex: '',       // Terminal
  antigravity: '󰊤', // Google
};

interface WaybarOutput {
  text: string;
  tooltip: string;
  class: string;
}

// Colored timeline bar
const BAR = `<span foreground='${C.sapphire}'>│</span>`;

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
 * Status indicator by percentage
 */
function indicator(val: number | null): string {
  if (val === null) return `<span foreground='${C.muted}'>○</span>`;
  if (val < 10) return `<span foreground='${C.red}'>●</span>`;
  if (val < 30) return `<span foreground='${C.orange}'>●</span>`;
  if (val < 60) return `<span foreground='${C.yellow}'>●</span>`;
  return `<span foreground='${C.green}'>●</span>`;
}

/**
 * Filter and merge models (remove Thinking suffix, 2.5, internal)
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
 * Format model line with bar on left
 */
function modelLine(name: string, val: number | null, reset: string | null, maxLen: number): string {
  const namePad = `<span foreground='${C.lavender}'>${name.padEnd(maxLen)}</span>`;
  const etaStr = `<span foreground='${C.teal}'>→ ${eta(reset)} (${resetTime(reset)})</span>`;
  return `${BAR}   ${indicator(val)} ${namePad} ${bar(val)} ${pctColored(val).padStart(4)} ${etaStr}`;
}

/**
 * Build Claude section
 */
function buildClaude(p: ProviderQuota): string[] {
  const lines: string[] = [];
  const icon = `<span foreground='${C.peach}'>${ICONS.claude}</span>`;
  const plan = p.plan ? ` <span foreground='${C.subtext}'>(${p.plan})</span>` : '';
  
  lines.push(`${BAR} ${icon} <span foreground='${C.mauve}' weight='bold'>Claude${plan}</span>`);
  
  if (p.error) {
    lines.push(`${BAR}   <span foreground='${C.peach}'>⚠️ ${p.error}</span>`);
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
    lines.push(`${BAR}`);
    lines.push(`${BAR}   <span foreground='${C.subtext}'>Weekly limit:</span>`);
    lines.push(modelLine('All Models', p.secondary.remaining, p.secondary.resetsAt, maxLen));
  }

  if (p.extraUsage?.enabled) {
    const { remaining, used, limit } = p.extraUsage;
    lines.push(`${BAR}`);
    const namePad = `<span foreground='${C.blue}'>${'Extra Usage'.padEnd(maxLen)}</span>`;
    const usedStr = `<span foreground='${C.subtext}'>$${(used / 100).toFixed(2)}/$${(limit / 100).toFixed(2)}</span>`;
    lines.push(`${BAR}   ${indicator(remaining)} ${namePad} ${bar(remaining)} ${pctColored(remaining).padStart(4)} ${usedStr}`);
  }

  return lines;
}

/**
 * Build Codex section
 */
function buildCodex(p: ProviderQuota): string[] {
  const lines: string[] = [];
  const icon = `<span foreground='${C.green}'>${ICONS.codex}</span>`;
  
  lines.push(`${BAR} ${icon} <span foreground='${C.mauve}' weight='bold'>Codex</span>`);
  
  if (p.error) {
    lines.push(`${BAR}   <span foreground='${C.peach}'>⚠️ ${p.error}</span>`);
    return lines;
  }

  const maxLen = 20;
  
  if (p.primary) {
    lines.push(modelLine('GPT-5.2 Codex', p.primary.remaining, p.primary.resetsAt, maxLen));
  }

  if (p.secondary) {
    lines.push(`${BAR}`);
    lines.push(`${BAR}   <span foreground='${C.subtext}'>Weekly limit:</span>`);
    lines.push(modelLine('GPT-5.2 Codex', p.secondary.remaining, p.secondary.resetsAt, maxLen));
  }

  return lines;
}

/**
 * Build Antigravity section
 */
function buildAntigravity(p: ProviderQuota): string[] {
  const lines: string[] = [];
  const icon = `<span foreground='${C.blue}'>${ICONS.antigravity}</span>`;
  const acc = p.account ? ` <span foreground='${C.subtext}'>(${p.account})</span>` : '';
  
  lines.push(`${BAR} ${icon} <span foreground='${C.mauve}' weight='bold'>Antigravity${acc}</span>`);
  
  if (p.error) {
    lines.push(`${BAR}   <span foreground='${C.peach}'>⚠️ ${p.error}</span>`);
    return lines;
  }

  if (!p.models || Object.keys(p.models).length === 0) {
    lines.push(`${BAR}   <span foreground='${C.muted}'>No models available</span>`);
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

  // Join with empty bar line between sections
  return sections.map(s => s.join('\n')).join(`\n${BAR}\n`);
}

/**
 * Build bar text with icons
 */
function buildText(quotas: AllQuotas): string {
  const parts: string[] = [];

  for (const p of quotas.providers) {
    if (!p.available) continue;
    
    const val = p.primary?.remaining ?? null;
    let icon = '', color = C.text;
    
    switch (p.provider) {
      case 'claude': icon = ICONS.claude; color = C.peach; break;
      case 'codex': icon = ICONS.codex; color = C.green; break;
      case 'antigravity': icon = ICONS.antigravity; color = C.blue; break;
    }
    
    parts.push(`<span foreground='${color}'>${icon}</span> ${pctColored(val)}`);
  }

  if (parts.length === 0) return `<span foreground='${C.muted}'>⚡ No Providers</span>`;
  return `⚡ ${parts.join(` <span foreground='${C.muted}'>│</span> `)}`;
}

export function formatForWaybar(quotas: AllQuotas): WaybarOutput {
  return { text: buildText(quotas), tooltip: buildTooltip(quotas), class: 'llm-usage' };
}

export function outputWaybar(quotas: AllQuotas): void {
  console.log(JSON.stringify(formatForWaybar(quotas)));
}
