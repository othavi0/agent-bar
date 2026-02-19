import { CONFIG } from '../config';
import { loadSettingsSync, type WindowPolicy } from '../settings';
import type { AllQuotas, ModelWindows, ProviderQuota, QuotaWindow } from '../providers/types';

// ANSI color codes (Catppuccin Mocha)
const C = {
  reset: '\x1b[0m',
  bold: '\x1b[1m',
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

// Box drawing characters
const B = {
  tl: '┏',
  bl: '┗',
  lt: '┣',
  h: '━',
  v: '┃',
  dot: '●',
  dotO: '○',
  diamond: '◆',
};

function getColor(pct: number | null): string {
  if (pct === null) return C.text;
  if (pct >= CONFIG.thresholds.green) return C.green;
  if (pct >= CONFIG.thresholds.yellow) return C.yellow;
  if (pct >= CONFIG.thresholds.orange) return C.orange;
  return C.red;
}

function bar(pct: number | null): string {
  if (pct === null) return `${C.muted}${'▱'.repeat(20)}${C.reset}`;
  const filled = Math.floor(pct / 5);
  const color = getColor(pct);
  return `${color}${'▰'.repeat(filled)}${C.muted}${'▱'.repeat(20 - filled)}${C.reset}`;
}

function pct(val: number | null): string {
  if (val === null) return '?%';
  return `${Math.round(val)}%`;
}

function indicator(val: number | null): string {
  if (val === null) return `${C.muted}${B.dotO}${C.reset}`;
  const color = getColor(val);
  return `${color}${B.dot}${C.reset}`;
}

function eta(iso: string | null, remaining: number | null): string {
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
  if (remaining === 100) return '';
  if (!iso) return '(??:??)';
  const d = new Date(iso);
  return `(${d.getHours().toString().padStart(2, '0')}:${d.getMinutes().toString().padStart(2, '0')})`;
}

function isValidModel(name: string): boolean {
  const lower = name.toLowerCase();
  return !/^(tab_|chat_|test_|internal_)/.test(lower) && !/preview$/i.test(name) && !/2\.5/i.test(name);
}

function applyModelFilter(models: Array<{name: string, remaining: number, resetsAt: string | null}>,
  allowed?: string[]): Array<{name: string, remaining: number, resetsAt: string | null}> {
  if (!allowed || allowed.length === 0) return models;
  return models.filter(m => allowed.includes(m.name) || allowed.includes(m.name.replace(/\s*\(Thinking\)/i, '')));
}

function classifyWindow(minutes: number | null | undefined): 'fiveHour' | 'sevenDay' | 'other' {
  if (!minutes || minutes <= 0) return 'other';
  if (Math.abs(minutes - 300) <= 90) return 'fiveHour';
  if (Math.abs(minutes - 10080) <= 1440) return 'sevenDay';
  return 'other';
}

function normalizePlanLabel(p: ProviderQuota): string {
  if (p.plan?.trim()) return p.plan;
  const raw = p.planType?.trim();
  if (!raw) return 'Unknown';

  const key = raw.toLowerCase();
  const map: Record<string, string> = {
    free: 'Free',
    go: 'Go',
    plus: 'Plus',
    pro: 'Pro',
    business: 'Business',
    team: 'Business',
    enterprise: 'Enterprise',
    edu: 'Edu',
    education: 'Edu',
    apikey: 'API Key',
    api_key: 'API Key',
  };
  return map[key] ?? raw;
}

function codexModelsFromQuota(p: ProviderQuota): Array<{ name: string; windows: ModelWindows; severity: number }> {
  const models: Record<string, ModelWindows> = {};

  if (p.modelsDetailed) {
    for (const [name, windows] of Object.entries(p.modelsDetailed)) {
      models[name] = windows;
    }
  }

  if (p.models) {
    for (const [name, window] of Object.entries(p.models)) {
      if (!models[name]) models[name] = {};
      const kind = classifyWindow(window.windowMinutes);
      if (kind === 'fiveHour' && !models[name].fiveHour) models[name].fiveHour = window;
      else if (kind === 'sevenDay' && !models[name].sevenDay) models[name].sevenDay = window;
      else {
        if (!models[name].other) models[name].other = [];
        models[name].other!.push(window);
      }
    }
  }

  if (Object.keys(models).length === 0 && (p.primary || p.secondary)) {
    const fallback: ModelWindows = {};
    for (const window of [p.primary, p.secondary]) {
      if (!window) continue;
      const kind = classifyWindow(window.windowMinutes);
      if (kind === 'fiveHour' && !fallback.fiveHour) fallback.fiveHour = window;
      else if (kind === 'sevenDay' && !fallback.sevenDay) fallback.sevenDay = window;
      else {
        if (!fallback.other) fallback.other = [];
        fallback.other.push(window);
      }
    }
    models['Codex'] = fallback;
  }

  return Object.entries(models)
    .map(([name, windows]) => {
      const values = [
        windows.fiveHour?.remaining,
        windows.sevenDay?.remaining,
        ...(windows.other?.map((w) => w.remaining) ?? []),
      ].filter((v): v is number => v !== undefined && v !== null);

      return {
        name,
        windows,
        severity: values.length > 0 ? Math.min(...values) : 101,
      };
    })
    .sort((a, b) => a.severity - b.severity || a.name.localeCompare(b.name));
}

function applyCodexModelFilter(
  models: Array<{ name: string; windows: ModelWindows; severity: number }>,
  allowed?: string[]
): Array<{ name: string; windows: ModelWindows; severity: number }> {
  if (!allowed || allowed.length === 0) return models;
  return models.filter((m) => allowed.includes(m.name));
}

// Vertical bar with provider color
const v = (color: string) => `${color}${B.v}${C.reset}`;

// Section label: ┣━ ◆ Label
const label = (text: string, color: string) => 
  `${color}${B.lt}${B.h}${C.reset} ${C.mauve}${C.bold}${B.diamond} ${text}${C.reset}`;

function windowLabel(minutes: number | null | undefined, fallback: string): string {
  if (!minutes || minutes <= 0) return fallback;
  const day = 60 * 24;
  if (minutes % day === 0) return `${minutes / day}-day limit`;
  if (minutes % 60 === 0) return `${minutes / 60}-hour limit`;
  return `${minutes}-minute limit`;
}

// Model line
function modelLine(name: string, window: QuotaWindow | undefined, maxLen: number, vColor: string): string {
  const rem = window?.remaining ?? null;
  const reset = window?.resetsAt ?? null;
  const nameS = `${C.lavender}${name.padEnd(maxLen)}${C.reset}`;
  const barS = bar(rem);
  const pctS = `${getColor(rem)}${pct(rem).padStart(4)}${C.reset}`;
  const etaS = `${C.teal}→ ${eta(reset, rem)} ${resetTime(reset, rem)}${C.reset}`;
  return `${v(vColor)}  ${indicator(rem)} ${nameS} ${barS} ${pctS} ${etaS}`;
}

function codexModelLine(name: string, window: QuotaWindow | undefined, maxLen: number, vColor: string): string {
  const rem = window?.remaining ?? null;
  const nameS = `${C.lavender}${name.padEnd(maxLen)}${C.reset}`;
  const barS = bar(rem);
  const pctS = `${getColor(rem)}${pct(rem).padStart(4)}${C.reset}`;
  const etaS = window?.resetsAt
    ? `${C.teal}→ ${eta(window.resetsAt, rem)} ${resetTime(window.resetsAt, rem)}${C.reset}`
    : `${C.teal}→ N/A${C.reset}`;
  return `${v(vColor)}  ${indicator(rem)} ${nameS} ${barS} ${pctS} ${etaS}`;
}

function buildClaude(p: ProviderQuota): string[] {
  const lines: string[] = [];
  const vc = C.peach;
  
  lines.push(`${vc}${B.tl}${B.h}${C.reset} ${vc}${C.bold}Claude${C.reset} ${vc}${B.h.repeat(50)}${C.reset}`);
  lines.push(v(vc));
  
  if (p.error) {
    lines.push(`${v(vc)}  ${C.red}⚠️ ${p.error}${C.reset}`);
  } else {
    const maxLen = 20;
    
    if (p.primary) {
      lines.push(label('5-hour limit (shared)', vc));
      lines.push(modelLine('All Models', p.primary, maxLen, vc));
    }

    // Per-model weekly quotas (when API provides them)
    if (p.weeklyModels && Object.keys(p.weeklyModels).length > 0) {
      lines.push(v(vc));
      lines.push(label('Weekly per model', vc));
      const entries = Object.entries(p.weeklyModels);
      const maxLenWeekly = Math.max(...entries.map(([name]) => name.length), maxLen);
      for (const [name, window] of entries) {
        lines.push(modelLine(name, window, maxLenWeekly, vc));
      }
    }
    
    // Generic weekly (shared)
    if (p.secondary) {
      lines.push(v(vc));
      lines.push(label('Weekly limit (shared)', vc));
      lines.push(modelLine('All Models', p.secondary, maxLen, vc));
    }

    if (p.extraUsage?.enabled && p.extraUsage.limit > 0) {
      const { remaining, used, limit } = p.extraUsage;
      lines.push(v(vc));
      lines.push(label('Extra Usage', vc));
      const nameS = `${C.lavender}${'Budget'.padEnd(maxLen)}${C.reset}`;
      const barS = bar(remaining);
      const pctS = `${getColor(remaining)}${pct(remaining).padStart(4)}${C.reset}`;
      const usedS = `${C.teal}$${(used / 100).toFixed(2)}/$${(limit / 100).toFixed(2)}${C.reset}`;
      lines.push(`${v(vc)}  ${indicator(remaining)} ${nameS} ${barS} ${pctS} ${usedS}`);
    }
  }
  
  lines.push(v(vc));
  lines.push(`${vc}${B.bl}${B.h.repeat(55)}${C.reset}`);
  
  return lines;
}

function buildCodex(p: ProviderQuota): string[] {
  const lines: string[] = [];
  const vc = C.green;
  const settings = loadSettingsSync();
  const policy: WindowPolicy = settings.windowPolicy?.[p.provider] ?? 'both';
  const planLabel = normalizePlanLabel(p);
  
  lines.push(`${vc}${B.tl}${B.h}${C.reset} ${vc}${C.bold}Codex${C.reset} ${vc}${B.h.repeat(51)}${C.reset}`);
  lines.push(v(vc));
  
  if (p.error) {
    lines.push(`${v(vc)}  ${C.red}⚠️ ${p.error}${C.reset}`);
  } else {
    const maxLen = 20;
    lines.push(`${v(vc)}  ${C.subtext}Plan: ${planLabel}${C.reset}`);

    let models = codexModelsFromQuota(p);
    models = applyCodexModelFilter(models, settings.models?.[p.provider]);

    if (models.length === 0) {
      lines.push(v(vc));
      lines.push(label('Available Models', vc));
      lines.push(`${v(vc)}  ${C.muted}No models selected${C.reset}`);
    } else {
      const modelLen = Math.max(...models.map((m) => m.name.length), maxLen);

      if (policy !== 'seven_day') {
        lines.push(v(vc));
        lines.push(label('5-hour limit', vc));
        for (const model of models) {
          lines.push(codexModelLine(model.name, model.windows.fiveHour, modelLen, vc));
        }
      }

      if (policy !== 'five_hour') {
        lines.push(v(vc));
        lines.push(label('7-day limit', vc));
        for (const model of models) {
          lines.push(codexModelLine(model.name, model.windows.sevenDay, modelLen, vc));
        }
      }
    }

    if (p.extraUsage?.enabled) {
      lines.push(v(vc));
      lines.push(label('Credits', vc));
      const nameS = `${C.lavender}${'Balance'.padEnd(maxLen)}${C.reset}`;
      const barS = bar(p.extraUsage.remaining);
      const pctS = `${getColor(p.extraUsage.remaining)}${pct(p.extraUsage.remaining).padStart(4)}${C.reset}`;
      const infoS = p.extraUsage.limit === -1
        ? `${C.teal}Unlimited${C.reset}`
        : `${C.teal}Balance${C.reset}`;
      lines.push(`${v(vc)}  ${indicator(p.extraUsage.remaining)} ${nameS} ${barS} ${pctS} ${infoS}`);
    }
  }
  
  lines.push(v(vc));
  lines.push(`${vc}${B.bl}${B.h.repeat(55)}${C.reset}`);
  
  return lines;
}

function buildAntigravity(p: ProviderQuota): string[] {
  const lines: string[] = [];
  const vc = C.blue;
  const settings = loadSettingsSync();
  
  lines.push(`${vc}${B.tl}${B.h}${C.reset} ${vc}${C.bold}Antigravity${C.reset} ${vc}${B.h.repeat(45)}${C.reset}`);
  lines.push(v(vc));
  
  if (p.error) {
    lines.push(`${v(vc)}  ${C.red}⚠️ ${p.error}${C.reset}`);
  } else if (!p.models || Object.keys(p.models).length === 0) {
    lines.push(`${v(vc)}  ${C.muted}No models available${C.reset}`);
  } else {
    let models = Object.entries(p.models)
      .filter(([name]) => isValidModel(name))
      .sort(([a], [b]) => {
        const pri = (n: string) => n.toLowerCase().includes('claude') ? 0 : n.toLowerCase().includes('gpt') ? 1 : n.toLowerCase().includes('gemini') ? 2 : 3;
        return pri(a) - pri(b) || a.localeCompare(b);
      })
      .map(([name, window]) => ({ name, remaining: window.remaining ?? 0, resetsAt: window.resetsAt }));

    models = applyModelFilter(models, settings.models?.[p.provider]);

    if (models.length === 0) {
      lines.push(label('Available Models', vc));
      lines.push(`${v(vc)}  ${C.muted}No models selected${C.reset}`);
    } else {
      const maxLen = Math.max(...models.map(({ name }) => name.length), 20);

      lines.push(label('Available Models', vc));
      for (const model of models) {
        lines.push(modelLine(model.name, { remaining: model.remaining, resetsAt: model.resetsAt }, maxLen, vc));
      }
    }
  }
  
  lines.push(v(vc));
  lines.push(`${vc}${B.bl}${B.h.repeat(55)}${C.reset}`);
  
  return lines;
}

function buildAmp(p: ProviderQuota): string[] {
  const lines: string[] = [];
  const vc = C.mauve;
  const m = p.meta ?? {};

  lines.push(`${vc}${B.tl}${B.h}${C.reset} ${vc}${C.bold}Amp${C.reset} ${vc}${B.h.repeat(53)}${C.reset}`);
  lines.push(v(vc));

  if (p.error) {
    lines.push(`${v(vc)}  ${C.red}⚠️ ${p.error}${C.reset}`);
  } else {
    // Thin tree connectors
    const tee = `${C.muted}├─${C.reset}`;
    const end = `${C.muted}└─${C.reset}`;

    // Free Tier
    const free = p.models?.['Free Tier'];
    if (free) {
      lines.push(label('Free Tier', vc));
      const barS = bar(free.remaining);
      const pctS = `${getColor(free.remaining)}${pct(free.remaining).padStart(4)}${C.reset}`;
      lines.push(`${v(vc)}  ${indicator(free.remaining)} ${barS} ${pctS}`);

      // Build sub-details
      const subs: string[] = [];

      const dollarParts: string[] = [];
      if (m.replenishRate) dollarParts.push(`${C.teal}${m.replenishRate}${C.reset}`);
      const dollars = [m.freeRemaining, m.freeTotal].filter(Boolean).join(' / ');
      if (dollars) dollarParts.push(`${C.text}( ${dollars} )${C.reset}`);
      if (m.bonus) dollarParts.push(`${C.teal}${m.bonus}${C.reset}`);
      if (dollarParts.length > 0) subs.push(dollarParts.join('  '));

      if (free.resetsAt && free.remaining !== 100) {
        subs.push(`${C.teal}Full in ${eta(free.resetsAt, free.remaining)}  ${resetTime(free.resetsAt, free.remaining)}${C.reset}`);
      }

      for (let i = 0; i < subs.length; i++) {
        const conn = i === subs.length - 1 ? end : tee;
        lines.push(`${v(vc)}  ${conn} ${subs[i]}`);
      }
    }

    // Credits
    const credits = p.models?.['Credits'];
    if (credits) {
      lines.push(v(vc));
      lines.push(label('Credits', vc));
      const balance = m.creditsBalance ?? '$0';
      const color = credits.remaining > 0 ? C.green : C.muted;
      lines.push(`${v(vc)}  ${indicator(credits.remaining)} ${color}${balance}${C.reset}`);
    }

    // Fallback for unknown models
    if (!free && !credits && p.models && Object.keys(p.models).length > 0) {
      const entries = Object.entries(p.models);
      const maxLen = Math.max(...entries.map(([name]) => name.length), 20);
      lines.push(label('Usage', vc));
      for (const [name, window] of entries) {
        const nameS = `${C.lavender}${name.padEnd(maxLen)}${C.reset}`;
        const barS = bar(window.remaining);
        const pctS = `${getColor(window.remaining)}${pct(window.remaining).padStart(4)}${C.reset}`;
        lines.push(`${v(vc)}  ${indicator(window.remaining)} ${nameS} ${barS} ${pctS}`);
      }
    }
  }

  if (p.account) {
    lines.push(v(vc));
    lines.push(`${v(vc)}  ${C.muted}Account: ${p.account}${C.reset}`);
  }

  lines.push(v(vc));
  lines.push(`${vc}${B.bl}${B.h.repeat(55)}${C.reset}`);

  return lines;
}

export function formatForTerminal(quotas: AllQuotas): string {
  const sections: string[][] = [];

  for (const p of quotas.providers) {
    if (!p.available && !p.error) continue;
    
    switch (p.provider) {
      case 'claude': sections.push(buildClaude(p)); break;
      case 'codex': sections.push(buildCodex(p)); break;
      case 'antigravity': sections.push(buildAntigravity(p)); break;
      case 'amp': sections.push(buildAmp(p)); break;
    }
  }

  if (sections.length === 0) {
    return `${C.muted}No providers connected${C.reset}`;
  }

  return sections.map(s => s.join('\n')).join('\n\n');
}

export function outputTerminal(quotas: AllQuotas): void {
  console.log(formatForTerminal(quotas));
}
