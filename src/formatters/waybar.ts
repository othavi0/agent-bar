import { APP_BASE_CLASS } from '../app-identity';
import { getStatusForPercent } from '../config';
import type { AllQuotas, ProviderQuota, QuotaWindow } from '../providers/types';
import { type DisplayMode, loadSettingsSync, type WindowPolicy } from '../settings';
import { BOX, ONE_DARK, PROVIDER_HEX } from '../theme';
import { applyCodexModelFilter, codexModelsFromQuota } from './codex-helpers';
import { barSegments, type ColorToken, colorForDisplay, indicatorSegments, type Segment } from './segments';
import { etaLabel, formatEta, formatPercent, formatResetTime, normalizePlanLabel, toDisplay } from './shared';

// Uniform tooltip width — all 3 cards share the same border
const TOOLTIP_BORDER = 56; // total visual chars per line (┗ + 55 ━)
const SETTINGS_CACHE_TTL_MS = 5_000;

let settingsCache: {
  value: ReturnType<typeof loadSettingsSync>;
  expiresAt: number;
} | null = null;

function loadSettingsCached(): ReturnType<typeof loadSettingsSync> {
  const now = Date.now();
  if (settingsCache && settingsCache.expiresAt > now) {
    return settingsCache.value;
  }

  const value = loadSettingsSync();
  settingsCache = {
    value,
    expiresAt: now + SETTINGS_CACHE_TTL_MS,
  };
  return value;
}

interface WaybarOutput {
  text: string;
  tooltip: string;
  class: string;
}

/** Escape special XML characters in dynamic content before embedding in Pango markup */
function escapeXml(str: string): string {
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/'/g, '&#39;')
    .replace(/"/g, '&quot;');
}

const s = (color: string, text: string, bold = false) =>
  `<span foreground='${color}'${bold ? " weight='bold'" : ''}>${text}</span>`;

const HEX_BY_TOKEN: Record<ColorToken, string> = {
  green: ONE_DARK.green,
  yellow: ONE_DARK.yellow,
  orange: ONE_DARK.orange,
  red: ONE_DARK.red,
  comment: ONE_DARK.comment,
  text: ONE_DARK.text,
};

function renderPango(segs: Segment[]): string {
  return segs.map((seg) => s(HEX_BY_TOKEN[seg.color], seg.text, seg.bold ?? false)).join('');
}

function colorFor(display: number | null, mode: DisplayMode): string {
  return HEX_BY_TOKEN[colorForDisplay(display, mode)];
}

function pctColored(display: number | null, mode: DisplayMode): string {
  return s(colorFor(display, mode), formatPercent(display));
}

function bar(display: number | null, mode: DisplayMode): string {
  return renderPango(barSegments(display, mode));
}

function indicator(display: number | null, mode: DisplayMode): string {
  return renderPango(indicatorSegments(display, mode));
}

function codexModelLine(
  name: string,
  window: QuotaWindow | undefined,
  maxLen: number,
  v: string,
  mode: DisplayMode,
): string {
  const rem = window?.remaining ?? null;
  const disp = toDisplay(rem, mode);
  const nameS = s(ONE_DARK.textBright, name.padEnd(maxLen));
  const b = bar(disp, mode);
  const pctS = s(colorFor(disp, mode), formatPercent(disp).padStart(4));
  const etaS = window?.resetsAt
    ? s(ONE_DARK.cyan, `→ ${formatEta(window.resetsAt, rem)} ${formatResetTime(window.resetsAt, rem)}`)
    : s(ONE_DARK.cyan, '→ N/A');
  return `${v}  ${indicator(disp, mode)} ${nameS} ${b} ${pctS} ${etaS}`;
}

// Section label with connecting line: ┣━ ◆ Label (uses provider color)
const label = (text: string, color: string) =>
  `${s(color, BOX.lt + BOX.h)} ${s(color, `${BOX.diamond} ${text}`, true)}`;

function formatAgo(iso: string): string {
  const diffMs = Date.now() - new Date(iso).getTime();
  if (diffMs < 60000) return 'just now';
  const mins = Math.floor(diffMs / 60000);
  if (mins < 60) return `${mins}m ago`;
  return `${Math.floor(mins / 60)}h ago`;
}

function buildHeader(title: string, subtitle: string | undefined, color: string): string {
  const fullTitle = subtitle ? `${title} · ${subtitle}` : title;
  const fill = Math.max(1, TOOLTIP_BORDER - 4 - fullTitle.length);
  return `${s(color, BOX.tl + BOX.h)} ${s(color, fullTitle, true)} ${s(color, BOX.h.repeat(fill))}`;
}

function buildFooter(color: string, fetchedAt?: string): string {
  if (!fetchedAt) {
    return s(color, BOX.bl + BOX.h.repeat(TOOLTIP_BORDER - 1));
  }
  const ago = formatAgo(fetchedAt);
  const stamp = ` cached · ${ago} `;
  const totalDashes = TOOLTIP_BORDER - 1 - stamp.length;
  const left = Math.max(1, Math.floor(totalDashes / 2));
  const right = Math.max(1, totalDashes - left);
  return s(color, BOX.bl + BOX.h.repeat(left)) + s(ONE_DARK.comment, stamp) + s(color, BOX.h.repeat(right));
}

/**
 * Build Claude tooltip
 */
function buildClaudeTooltip(p: ProviderQuota, fetchedAt: string | undefined, mode: DisplayMode): string {
  const lines: string[] = [];
  const vc = PROVIDER_HEX.claude;
  const v = s(vc, BOX.v);
  const planLabel = normalizePlanLabel(p);

  lines.push(buildHeader('Claude', planLabel !== 'Unknown' ? planLabel : undefined, vc));
  lines.push(v);

  if (p.error) {
    lines.push(`${v}  ${s(ONE_DARK.red, `⚠️ ${escapeXml(p.error)}`)}`);
  } else {
    const maxLen = 20;

    if (p.primary) {
      lines.push(label('5-hour limit (shared)', vc));
      const rem = p.primary.remaining;
      const disp = toDisplay(rem, mode);
      const name = s(ONE_DARK.textBright, 'All Models'.padEnd(maxLen));
      const b = bar(disp, mode);
      const pctS = s(colorFor(disp, mode), formatPercent(disp).padStart(4));
      const etaS = s(
        ONE_DARK.cyan,
        `→ ${formatEta(p.primary.resetsAt, rem)} ${formatResetTime(p.primary.resetsAt, rem)}`,
      );
      lines.push(`${v}  ${indicator(disp, mode)} ${name} ${b} ${pctS} ${etaS}`);
    }

    // Per-model weekly quotas (when API provides them)
    if (p.weeklyModels && Object.keys(p.weeklyModels).length > 0) {
      lines.push(v);
      lines.push(label('Weekly per model', vc));
      const entries = Object.entries(p.weeklyModels);
      const wMaxLen = Math.max(...entries.map(([name]) => name.length), 20);

      for (const [name, window] of entries) {
        const rem = window.remaining;
        const disp = toDisplay(rem, mode);
        const nameS = s(ONE_DARK.textBright, name.padEnd(wMaxLen));
        const b = bar(disp, mode);
        const pctS = s(colorFor(disp, mode), formatPercent(disp).padStart(4));
        const etaS = s(ONE_DARK.cyan, `→ ${formatEta(window.resetsAt, rem)} ${formatResetTime(window.resetsAt, rem)}`);
        lines.push(`${v}  ${indicator(disp, mode)} ${nameS} ${b} ${pctS} ${etaS}`);
      }
    }

    // Generic weekly (shared)
    if (p.secondary) {
      lines.push(v);
      lines.push(label('Weekly limit (shared)', vc));
      const rem = p.secondary.remaining;
      const disp = toDisplay(rem, mode);
      const name = s(ONE_DARK.textBright, 'All Models'.padEnd(20));
      const b = bar(disp, mode);
      const pctS = s(colorFor(disp, mode), formatPercent(disp).padStart(4));
      const etaS = s(
        ONE_DARK.cyan,
        `→ ${formatEta(p.secondary.resetsAt, rem)} ${formatResetTime(p.secondary.resetsAt, rem)}`,
      );
      lines.push(`${v}  ${indicator(disp, mode)} ${name} ${b} ${pctS} ${etaS}`);
    }

    if (p.extraUsage?.enabled && p.extraUsage.limit > 0) {
      const { remaining, used, limit } = p.extraUsage;
      const disp = toDisplay(remaining, mode);
      lines.push(v);
      lines.push(label('Extra Usage', vc));
      const name = s(ONE_DARK.textBright, 'Budget'.padEnd(20));
      const b = bar(disp, mode);
      const pctS = s(colorFor(disp, mode), formatPercent(disp).padStart(4));
      const usedS = s(ONE_DARK.cyan, `$${(used / 100).toFixed(2)}/$${(limit / 100).toFixed(2)}`);
      lines.push(`${v}  ${indicator(disp, mode)} ${name} ${b} ${pctS} ${usedS}`);
    }
  }

  lines.push(v);
  lines.push(buildFooter(vc, fetchedAt));

  return lines.join('\n');
}

/**
 * Build Codex tooltip
 */
function buildCodexTooltip(p: ProviderQuota, fetchedAt: string | undefined, mode: DisplayMode): string {
  const lines: string[] = [];
  const vc = PROVIDER_HEX.codex;
  const v = s(vc, BOX.v);
  const settings = loadSettingsCached();
  const policy: WindowPolicy = settings.windowPolicy?.[p.provider] ?? 'both';
  const planLabel = normalizePlanLabel(p);

  lines.push(buildHeader('Codex', planLabel !== 'Unknown' ? planLabel : undefined, vc));
  lines.push(v);

  if (p.error) {
    lines.push(`${v}  ${s(ONE_DARK.red, `⚠️ ${escapeXml(p.error)}`)}`);
  } else {
    let models = codexModelsFromQuota(p);
    models = applyCodexModelFilter(models, settings.models?.[p.provider]);

    if (models.length === 0) {
      lines.push(v);
      lines.push(label('Available Models', vc));
      lines.push(`${v}  ${s(ONE_DARK.comment, 'No models selected')}`);
    } else {
      const maxLen = Math.max(...models.map((m) => m.name.length), 20);

      if (policy !== 'seven_day') {
        lines.push(v);
        lines.push(label('5-hour limit', vc));
        for (const model of models) {
          lines.push(codexModelLine(model.name, model.windows.fiveHour, maxLen, v, mode));
        }
      }

      if (policy !== 'five_hour') {
        lines.push(v);
        lines.push(label('7-day limit', vc));
        for (const model of models) {
          lines.push(codexModelLine(model.name, model.windows.sevenDay, maxLen, v, mode));
        }
      }
    }

    if (p.extraUsage?.enabled) {
      const rem = p.extraUsage.remaining;
      const disp = toDisplay(rem, mode);
      lines.push(v);
      lines.push(label('Credits', vc));
      const name = s(ONE_DARK.textBright, 'Balance'.padEnd(20));
      const b = bar(disp, mode);
      const pctS = s(colorFor(disp, mode), formatPercent(disp).padStart(4));
      const limitS = p.extraUsage.limit === -1 ? s(ONE_DARK.cyan, 'Unlimited') : s(ONE_DARK.cyan, 'Balance');
      lines.push(`${v}  ${indicator(disp, mode)} ${name} ${b} ${pctS} ${limitS}`);
    }
  }

  lines.push(v);
  lines.push(buildFooter(vc, fetchedAt));

  return lines.join('\n');
}

/**
 * Build Amp tooltip
 */
function buildAmpTooltip(p: ProviderQuota, fetchedAt: string | undefined, mode: DisplayMode): string {
  const lines: string[] = [];
  const vc = PROVIDER_HEX.amp;
  const v = s(vc, BOX.v);
  const m = p.meta ?? {};

  // Account goes in header for better hierarchy
  const accountShort = p.account ? escapeXml(p.account) : undefined;
  lines.push(buildHeader('Amp', accountShort, vc));
  lines.push(v);

  if (p.error) {
    lines.push(`${v}  ${s(ONE_DARK.red, `⚠️ ${escapeXml(p.error)}`)}`);
  } else {
    // --- Free Tier ---
    const free = p.models?.['Free Tier'];
    if (free) {
      const rem = free.remaining;
      const disp = toDisplay(rem, mode);
      const b = bar(disp, mode);
      const pctS = s(colorFor(disp, mode), formatPercent(disp).padStart(4));

      // ETA inline with bar (same style as Claude/Codex)
      const etaParts: string[] = [];
      if (free.resetsAt && rem !== 100) {
        etaParts.push(
          s(
            ONE_DARK.cyan,
            `→ ${etaLabel(mode)} ${formatEta(free.resetsAt, rem)} ${formatResetTime(free.resetsAt, rem)}`,
          ),
        );
      }
      const etaS = etaParts.length > 0 ? `  ${etaParts[0]}` : '';

      lines.push(label('Free Tier', vc));
      lines.push(`${v}  ${indicator(disp, mode)} ${b} ${pctS}${etaS}`);

      // Rate / balance info on second line with ○ indicator
      const infoParts: string[] = [];
      if (m.replenishRate) infoParts.push(s(ONE_DARK.cyan, m.replenishRate));
      const dollars = [m.freeRemaining, m.freeTotal].filter(Boolean).join(' / ');
      if (dollars) infoParts.push(s(ONE_DARK.text, dollars));
      if (m.bonus) infoParts.push(s(ONE_DARK.cyan, m.bonus));
      if (infoParts.length > 0) {
        lines.push(`${v}  ${s(ONE_DARK.comment, BOX.dotO)} ${infoParts.join(s(ONE_DARK.comment, '  |  '))}`);
      }
    }

    // --- Credits ---
    const credits = p.models?.Credits;
    if (credits) {
      lines.push(v);
      const balance = m.creditsBalance ?? '$0';
      const color = credits.remaining > 0 ? ONE_DARK.green : ONE_DARK.comment;
      lines.push(label('Credits', vc));
      lines.push(`${v}  ${indicator(toDisplay(credits.remaining, mode), mode)} ${s(color, `${balance} remaining`)}`);
    }
  }

  lines.push(v);
  lines.push(buildFooter(vc, fetchedAt));

  return lines.join('\n');
}

// ---------------------------------------------------------------------------
// Tooltip builder registry
// ---------------------------------------------------------------------------

type TooltipBuilder = (p: ProviderQuota, fetchedAt: string | undefined, mode: DisplayMode) => string;

const TOOLTIP_BUILDERS: Record<string, TooltipBuilder> = {
  claude: buildClaudeTooltip,
  codex: buildCodexTooltip,
  amp: buildAmpTooltip,
};

function buildGenericTooltip(p: ProviderQuota, fetchedAt: string | undefined, mode: DisplayMode): string {
  const color = ONE_DARK.text;
  const v = s(color, BOX.v);
  const lines: string[] = [];

  lines.push(buildHeader(p.displayName ?? p.provider, undefined, color));

  if (p.error) {
    lines.push(`${v}  ${s(ONE_DARK.red, p.error)}`);
  } else if (p.primary) {
    const rem = p.primary.remaining;
    const disp = toDisplay(rem, mode);
    lines.push(`${v}  ${indicator(disp, mode)} ${bar(disp, mode)} ${s(colorFor(disp, mode), formatPercent(disp))}`);
  }

  lines.push(buildFooter(color, fetchedAt));
  return lines.join('\n');
}

function buildProviderTooltip(p: ProviderQuota, fetchedAt: string | undefined, mode: DisplayMode): string {
  const builder = TOOLTIP_BUILDERS[p.provider];
  if (builder) return builder(p, fetchedAt, mode);
  return buildGenericTooltip(p, fetchedAt, mode);
}

function buildTooltip(quotas: AllQuotas, mode: DisplayMode): string {
  const sections: string[] = [];
  const fetchedAt = quotas.fetchedAt;

  for (const p of quotas.providers) {
    if (!p.available && !p.error) continue;
    sections.push(buildProviderTooltip(p, fetchedAt, mode));
  }

  return sections.join('\n\n');
}

function buildText(quotas: AllQuotas, mode: DisplayMode): string {
  const parts: string[] = [];

  for (const p of quotas.providers) {
    if (!p.available) continue;
    const rem = p.primary?.remaining ?? null;
    const disp = toDisplay(rem, mode);
    parts.push(pctColored(disp, mode));
  }

  if (parts.length === 0) return s(ONE_DARK.comment, 'No Providers');
  return parts.join(` ${s(ONE_DARK.comment, '│')} `);
}

function getClass(quotas: AllQuotas): string {
  const classes: string[] = [APP_BASE_CLASS];

  for (const p of quotas.providers) {
    if (!p.available) continue;
    const val = p.primary?.remaining ?? 100;
    const status = getStatusForPercent(val);
    classes.push(`${p.provider}-${status}`);
  }

  return classes.join(' ');
}

export function formatForWaybar(quotas: AllQuotas, mode: DisplayMode = 'remaining'): WaybarOutput {
  return {
    text: buildText(quotas, mode),
    tooltip: buildTooltip(quotas, mode),
    class: getClass(quotas),
  };
}

export function outputWaybar(quotas: AllQuotas, mode: DisplayMode = 'remaining'): void {
  console.log(JSON.stringify(formatForWaybar(quotas, mode)));
}

export function formatProviderForWaybar(quota: ProviderQuota, mode: DisplayMode = 'remaining'): WaybarOutput {
  // Disconnected is a terminal status — class omits health prefix and `mode` only affects tooltip.
  if (!quota.available || quota.error) {
    return {
      text: `<span foreground='${ONE_DARK.red}'>󱘖</span>`,
      tooltip: buildProviderTooltip(quota, undefined, mode),
      class: `${APP_BASE_CLASS}-${quota.provider} disconnected`,
    };
  }

  const rem = quota.primary?.remaining ?? null;
  const disp = toDisplay(rem, mode);
  // class based on health (raw remaining), not display value
  const health = rem ?? 100;
  const status = getStatusForPercent(health);

  return {
    text: pctColored(disp, mode),
    tooltip: buildProviderTooltip(quota, undefined, mode),
    class: `${APP_BASE_CLASS}-${quota.provider} ${status}`,
  };
}
