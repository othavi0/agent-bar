import * as p from '@clack/prompts';
import { getAllQuotas } from '../providers';
import { loadSettingsSync, type WindowPolicy } from '../settings';
import { catppuccin, semantic, getQuotaColor, colorize, bold } from './colors';
import type { ModelWindows, ProviderQuota, QuotaWindow } from '../providers/types';

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

function bar(pct: number | null): string {
  if (pct === null) return colorize('▱'.repeat(20), semantic.muted);
  const filled = Math.floor(pct / 5);
  const color = getQuotaColor(pct);
  return colorize('▰'.repeat(filled), color) + colorize('▱'.repeat(20 - filled), semantic.muted);
}

function indicator(val: number | null): string {
  if (val === null) return colorize(B.dotO, semantic.muted);
  return colorize(B.dot, getQuotaColor(val));
}

function pct(val: number | null): string {
  if (val === null) return '?%';
  return `${Math.round(val)}%`;
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

// Vertical bar
const v = (color: string) => colorize(B.v, color);

// Section label: ┣━ ◆ Label
const label = (text: string, providerColor: string) => 
  colorize(B.lt + B.h, providerColor) + ' ' + colorize(B.diamond + ' ' + text, catppuccin.mauve, true);

// Model line
function modelLine(name: string, window: QuotaWindow | undefined, maxLen: number, vColor: string): string {
  const rem = window?.remaining ?? null;
  const reset = window?.resetsAt ?? null;
  const nameS = colorize(name.padEnd(maxLen), catppuccin.lavender);
  const barS = bar(rem);
  const pctS = colorize(pct(rem).padStart(4), getQuotaColor(rem));
  const etaS = colorize(`→ ${eta(reset, rem)} ${resetTime(reset, rem)}`, catppuccin.teal);
  return `${v(vColor)}  ${indicator(rem)} ${nameS} ${barS} ${pctS} ${etaS}`;
}

function codexModelLine(name: string, window: QuotaWindow | undefined, maxLen: number, vColor: string): string {
  const rem = window?.remaining ?? null;
  const nameS = colorize(name.padEnd(maxLen), catppuccin.lavender);
  const barS = bar(rem);
  const pctS = colorize(pct(rem).padStart(4), getQuotaColor(rem));
  const etaS = window?.resetsAt
    ? colorize(`→ ${eta(window.resetsAt, rem)} ${resetTime(window.resetsAt, rem)}`, catppuccin.teal)
    : colorize('→ N/A', catppuccin.teal);
  return `${v(vColor)}  ${indicator(rem)} ${nameS} ${barS} ${pctS} ${etaS}`;
}

function buildClaude(p: ProviderQuota): string[] {
  const lines: string[] = [];
  const vc = catppuccin.peach;
  
  lines.push(colorize(B.tl + B.h, vc) + ' ' + colorize('Claude', vc, true) + ' ' + colorize(B.h.repeat(50), vc));
  lines.push(v(vc));
  
  if (p.error) {
    lines.push(`${v(vc)}  ${colorize('⚠️ ' + p.error, catppuccin.red)}`);
  } else {
    const maxLen = 20;
    
    if (p.primary) {
      lines.push(label('5-hour limit', vc));
      for (const m of ['Opus', 'Sonnet', 'Haiku']) {
        lines.push(modelLine(m, p.primary, maxLen, vc));
      }
    }

    if (p.weeklyModels && Object.keys(p.weeklyModels).length > 0) {
      lines.push(v(vc));
      lines.push(label('Weekly limit', vc));
      const entries = Object.entries(p.weeklyModels);
      const maxLenWeekly = Math.max(...entries.map(([name]) => name.length), maxLen);
      for (const [name, window] of entries) {
        lines.push(modelLine(name, window, maxLenWeekly, vc));
      }
    } else if (p.secondary) {
      lines.push(v(vc));
      lines.push(label('Weekly limit', vc));
      lines.push(modelLine('All Models', p.secondary, maxLen, vc));
    }

    if (p.extraUsage?.enabled && p.extraUsage.limit > 0) {
      const { remaining, used, limit } = p.extraUsage;
      lines.push(v(vc));
      lines.push(label('Extra Usage', vc));
      const nameS = colorize('Budget'.padEnd(maxLen), catppuccin.lavender);
      const barS = bar(remaining);
      const pctS = colorize(pct(remaining).padStart(4), getQuotaColor(remaining));
      const usedS = colorize(`$${(used / 100).toFixed(2)}/$${(limit / 100).toFixed(2)}`, catppuccin.teal);
      lines.push(`${v(vc)}  ${indicator(remaining)} ${nameS} ${barS} ${pctS} ${usedS}`);
    }
  }
  
  lines.push(v(vc));
  lines.push(colorize(B.bl + B.h.repeat(55), vc));
  
  return lines;
}

function buildCodex(p: ProviderQuota): string[] {
  const lines: string[] = [];
  const vc = catppuccin.green;
  const settings = loadSettingsSync();
  const policy: WindowPolicy = settings.windowPolicy?.[p.provider] ?? 'both';
  const planLabel = normalizePlanLabel(p);
  
  lines.push(colorize(B.tl + B.h, vc) + ' ' + colorize('Codex', vc, true) + ' ' + colorize(B.h.repeat(51), vc));
  lines.push(v(vc));
  
  if (p.error) {
    lines.push(`${v(vc)}  ${colorize('⚠️ ' + p.error, catppuccin.red)}`);
  } else {
    const maxLen = 20;
    lines.push(`${v(vc)}  ${colorize(`Plan: ${planLabel}`, semantic.muted)}`);

    let models = codexModelsFromQuota(p);
    models = applyCodexModelFilter(models, settings.models?.[p.provider]);

    if (models.length === 0) {
      lines.push(v(vc));
      lines.push(label('Available Models', vc));
      lines.push(`${v(vc)}  ${colorize('No models selected', semantic.muted)}`);
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
      const nameS = colorize('Balance'.padEnd(maxLen), catppuccin.lavender);
      const barS = bar(p.extraUsage.remaining);
      const pctS = colorize(pct(p.extraUsage.remaining).padStart(4), getQuotaColor(p.extraUsage.remaining));
      const infoS = p.extraUsage.limit === -1
        ? colorize('Unlimited', catppuccin.teal)
        : colorize('Balance', catppuccin.teal);
      lines.push(`${v(vc)}  ${indicator(p.extraUsage.remaining)} ${nameS} ${barS} ${pctS} ${infoS}`);
    }
  }
  
  lines.push(v(vc));
  lines.push(colorize(B.bl + B.h.repeat(55), vc));
  
  return lines;
}

function buildAmp(p: ProviderQuota): string[] {
  const lines: string[] = [];
  const vc = catppuccin.mauve;

  lines.push(colorize(B.tl + B.h, vc) + ' ' + colorize('Amp', vc, true) + ' ' + colorize(B.h.repeat(53), vc));
  lines.push(v(vc));

  if (p.error) {
    lines.push(`${v(vc)}  ${colorize('⚠️ ' + p.error, catppuccin.red)}`);
  } else if (!p.models || Object.keys(p.models).length === 0) {
    lines.push(`${v(vc)}  ${colorize('No usage data', semantic.muted)}`);
  } else {
    const entries = Object.entries(p.models);
    const maxLen = Math.max(...entries.map(([name]) => name.length), 20);

    lines.push(label('Usage', vc));
    for (const [name, window] of entries) {
      const nameS = colorize(name.padEnd(maxLen), catppuccin.lavender);
      const barS = bar(window.remaining);
      const pctS = colorize(pct(window.remaining).padStart(4), getQuotaColor(window.remaining));
      lines.push(`${v(vc)}  ${indicator(window.remaining)} ${nameS} ${barS} ${pctS}`);
    }
  }

  if (p.account) {
    lines.push(v(vc));
    lines.push(`${v(vc)}  ${colorize(`Account: ${p.account}`, semantic.muted)}`);
  }

  lines.push(v(vc));
  lines.push(colorize(B.bl + B.h.repeat(55), vc));

  return lines;
}

function buildAntigravity(p: ProviderQuota): string[] {
  const lines: string[] = [];
  const vc = catppuccin.blue;
  
  lines.push(colorize(B.tl + B.h, vc) + ' ' + colorize('Antigravity', vc, true) + ' ' + colorize(B.h.repeat(45), vc));
  lines.push(v(vc));
  
  if (p.error) {
    lines.push(`${v(vc)}  ${colorize('⚠️ ' + p.error, catppuccin.red)}`);
  } else if (!p.models || Object.keys(p.models).length === 0) {
    lines.push(`${v(vc)}  ${colorize('No models available', semantic.muted)}`);
  } else {
    const models = Object.entries(p.models)
      .filter(([name]) => isValidModel(name))
      .sort(([a], [b]) => {
        const pri = (n: string) => n.toLowerCase().includes('claude') ? 0 : n.toLowerCase().includes('gpt') ? 1 : n.toLowerCase().includes('gemini') ? 2 : 3;
        return pri(a) - pri(b) || a.localeCompare(b);
      });

    const maxLen = Math.max(...models.map(([n]) => n.length), 20);

    lines.push(label('Available Models', vc));
    for (const [name, window] of models) {
      lines.push(modelLine(name, window, maxLen, vc));
    }
  }
  
  lines.push(v(vc));
  lines.push(colorize(B.bl + B.h.repeat(55), vc));
  
  return lines;
}

export async function showListAll(): Promise<void> {
  const s = p.spinner();
  s.start(colorize('Loading quotas...', semantic.subtitle));

  const quotas = await getAllQuotas();
  
  s.stop(colorize('Quotas loaded', semantic.good));

  // Build output
  const sections: string[][] = [];
  
  for (const provider of quotas.providers) {
    if (!provider.available && !provider.error) continue;
    
    switch (provider.provider) {
      case 'claude': sections.push(buildClaude(provider)); break;
      case 'codex': sections.push(buildCodex(provider)); break;
      case 'antigravity': sections.push(buildAntigravity(provider)); break;
      case 'amp': sections.push(buildAmp(provider)); break;
    }
  }

  // Print
  console.log('');
  for (const section of sections) {
    for (const line of section) {
      console.log(line);
    }
    console.log('');
  }

  console.log(colorize('Press Enter to continue...', semantic.subtitle));

  // Wait for enter
  await new Promise<void>((resolve) => {
    process.stdin.setRawMode?.(true);
    process.stdin.resume();
    process.stdin.once('data', () => {
      process.stdin.setRawMode?.(false);
      resolve();
    });
  });
}
