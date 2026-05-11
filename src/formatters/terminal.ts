import type {
  AllQuotas,
  AmpQuotaExtra,
  ClaudeQuotaExtra,
  CodexQuotaExtra,
  CopilotQuotaExtra,
  CopilotQuotaSnapshot,
  ProviderQuota,
  QuotaWindow,
} from '../providers/types';
import { loadSettingsSync, type WindowPolicy } from '../settings';
import { ANSI, BOX, PROVIDER_ANSI } from '../theme';
import { applyCodexModelFilter, codexModelsFromQuota } from './codex-helpers';
import { barSegments, type ColorToken, colorForDisplay, indicatorSegments, type Segment } from './segments';
import {
  type DisplayMode,
  etaLabel,
  formatEta,
  formatPercent,
  formatResetTime,
  normalizePlanLabel,
  toDisplay,
} from './shared';

const ANSI_BY_TOKEN: Record<ColorToken, string> = {
  green: ANSI.green,
  yellow: ANSI.yellow,
  orange: ANSI.orange,
  red: ANSI.red,
  comment: ANSI.comment,
  text: ANSI.text,
};

function renderAnsi(segs: Segment[]): string {
  if (segs.length === 0) return '';
  const body = segs.map((s) => `${ANSI_BY_TOKEN[s.color]}${s.bold ? ANSI.bold : ''}${s.text}`).join('');
  return `${body}${ANSI.reset}`;
}

function getColor(display: number | null, mode: DisplayMode): string {
  return ANSI_BY_TOKEN[colorForDisplay(display, mode)];
}

function bar(display: number | null, mode: DisplayMode): string {
  return renderAnsi(barSegments(display, mode));
}

function indicator(display: number | null, mode: DisplayMode): string {
  return renderAnsi(indicatorSegments(display, mode));
}

// Vertical bar with provider color
const v = (color: string) => `${color}${BOX.v}${ANSI.reset}`;

// Section label: ┣━ ◆ Label
const label = (text: string, color: string) =>
  `${color}${BOX.lt}${BOX.h}${ANSI.reset} ${ANSI.magenta}${ANSI.bold}${BOX.diamond} ${text}${ANSI.reset}`;

// Model line
function modelLine(
  name: string,
  window: QuotaWindow | undefined,
  maxLen: number,
  vColor: string,
  mode: DisplayMode,
): string {
  const rem = window?.remaining ?? null;
  const reset = window?.resetsAt ?? null;
  const disp = toDisplay(rem, mode);
  const nameS = `${ANSI.textBright}${name.padEnd(maxLen)}${ANSI.reset}`;
  const barS = bar(disp, mode);
  const pctS = `${getColor(disp, mode)}${formatPercent(disp).padStart(4)}${ANSI.reset}`;
  const etaS = `${ANSI.cyan}→ ${formatEta(reset, rem)} ${formatResetTime(reset, rem)}${ANSI.reset}`;
  return `${v(vColor)}  ${indicator(disp, mode)} ${nameS} ${barS} ${pctS} ${etaS}`;
}

function codexModelLine(
  name: string,
  window: QuotaWindow | undefined,
  maxLen: number,
  vColor: string,
  mode: DisplayMode,
): string {
  const rem = window?.remaining ?? null;
  const disp = toDisplay(rem, mode);
  const nameS = `${ANSI.textBright}${name.padEnd(maxLen)}${ANSI.reset}`;
  const barS = bar(disp, mode);
  const pctS = `${getColor(disp, mode)}${formatPercent(disp).padStart(4)}${ANSI.reset}`;
  const etaS = window?.resetsAt
    ? `${ANSI.cyan}→ ${formatEta(window.resetsAt, rem)} ${formatResetTime(window.resetsAt, rem)}${ANSI.reset}`
    : `${ANSI.cyan}→ N/A${ANSI.reset}`;
  return `${v(vColor)}  ${indicator(disp, mode)} ${nameS} ${barS} ${pctS} ${etaS}`;
}

function buildClaude(p: ProviderQuota, mode: DisplayMode): string[] {
  const lines: string[] = [];
  const vc = PROVIDER_ANSI.claude;

  lines.push(
    `${vc}${BOX.tl}${BOX.h}${ANSI.reset} ${vc}${ANSI.bold}Claude${ANSI.reset} ${vc}${BOX.h.repeat(50)}${ANSI.reset}`,
  );
  lines.push(v(vc));

  if (p.error) {
    lines.push(`${v(vc)}  ${ANSI.red}⚠️ ${p.error}${ANSI.reset}`);
  } else {
    const maxLen = 20;

    if (p.primary) {
      lines.push(label('5-hour limit (shared)', vc));
      lines.push(modelLine('All Models', p.primary, maxLen, vc, mode));
    }

    // Per-model weekly quotas (when API provides them)
    const weeklyModels = p.provider === 'claude' ? p.extra?.weeklyModels : undefined;
    if (weeklyModels && Object.keys(weeklyModels).length > 0) {
      lines.push(v(vc));
      lines.push(label('Weekly per model', vc));
      const entries = Object.entries(weeklyModels);
      const maxLenWeekly = Math.max(...entries.map(([name]) => name.length), maxLen);
      for (const [name, window] of entries) {
        lines.push(modelLine(name, window, maxLenWeekly, vc, mode));
      }
    }

    // Generic weekly (shared)
    if (p.secondary) {
      lines.push(v(vc));
      lines.push(label('Weekly limit (shared)', vc));
      lines.push(modelLine('All Models', p.secondary, maxLen, vc, mode));
    }

    const _claudeExtra = p.provider === 'claude' ? (p.extra as ClaudeQuotaExtra | undefined) : undefined;
    if (_claudeExtra?.extraUsage?.enabled && _claudeExtra.extraUsage.limit > 0) {
      const { remaining, used, limit } = _claudeExtra.extraUsage;
      const disp = toDisplay(remaining, mode);
      lines.push(v(vc));
      lines.push(label('Extra Usage', vc));
      const nameS = `${ANSI.textBright}${'Budget'.padEnd(maxLen)}${ANSI.reset}`;
      const barS = bar(disp, mode);
      const pctS = `${getColor(disp, mode)}${formatPercent(disp).padStart(4)}${ANSI.reset}`;
      const usedS = `${ANSI.cyan}$${(used / 100).toFixed(2)}/$${(limit / 100).toFixed(2)}${ANSI.reset}`;
      lines.push(`${v(vc)}  ${indicator(disp, mode)} ${nameS} ${barS} ${pctS} ${usedS}`);
    }
  }

  lines.push(v(vc));
  lines.push(`${vc}${BOX.bl}${BOX.h.repeat(55)}${ANSI.reset}`);

  return lines;
}

function buildCodex(p: ProviderQuota, mode: DisplayMode): string[] {
  const lines: string[] = [];
  const vc = PROVIDER_ANSI.codex;
  const settings = loadSettingsSync();
  const policy: WindowPolicy = settings.windowPolicy?.[p.provider] ?? 'both';
  const planLabel = normalizePlanLabel(p);

  lines.push(
    `${vc}${BOX.tl}${BOX.h}${ANSI.reset} ${vc}${ANSI.bold}Codex${ANSI.reset} ${vc}${BOX.h.repeat(51)}${ANSI.reset}`,
  );
  lines.push(v(vc));

  if (p.error) {
    lines.push(`${v(vc)}  ${ANSI.red}⚠️ ${p.error}${ANSI.reset}`);
  } else {
    const maxLen = 20;
    lines.push(`${v(vc)}  ${ANSI.muted}Plan: ${planLabel}${ANSI.reset}`);

    let models = codexModelsFromQuota(p);
    models = applyCodexModelFilter(models, settings.models?.[p.provider]);

    if (models.length === 0) {
      lines.push(v(vc));
      lines.push(label('Available Models', vc));
      lines.push(`${v(vc)}  ${ANSI.comment}No models selected${ANSI.reset}`);
    } else {
      const modelLen = Math.max(...models.map((m) => m.name.length), maxLen);

      if (policy !== 'seven_day') {
        lines.push(v(vc));
        lines.push(label('5-hour limit', vc));
        for (const model of models) {
          lines.push(codexModelLine(model.name, model.windows.fiveHour, modelLen, vc, mode));
        }
      }

      if (policy !== 'five_hour') {
        lines.push(v(vc));
        lines.push(label('7-day limit', vc));
        for (const model of models) {
          lines.push(codexModelLine(model.name, model.windows.sevenDay, modelLen, vc, mode));
        }
      }
    }

    const _codexExtra = p.provider === 'codex' ? (p.extra as CodexQuotaExtra | undefined) : undefined;
    if (_codexExtra?.extraUsage?.enabled) {
      const codexExtraUsage = _codexExtra.extraUsage;
      const disp = toDisplay(codexExtraUsage.remaining, mode);
      lines.push(v(vc));
      lines.push(label('Credits', vc));
      const nameS = `${ANSI.textBright}${'Balance'.padEnd(maxLen)}${ANSI.reset}`;
      const barS = bar(disp, mode);
      const pctS = `${getColor(disp, mode)}${formatPercent(disp).padStart(4)}${ANSI.reset}`;
      const infoS =
        codexExtraUsage.limit === -1 ? `${ANSI.cyan}Unlimited${ANSI.reset}` : `${ANSI.cyan}Balance${ANSI.reset}`;
      lines.push(`${v(vc)}  ${indicator(disp, mode)} ${nameS} ${barS} ${pctS} ${infoS}`);
    }
  }

  lines.push(v(vc));
  lines.push(`${vc}${BOX.bl}${BOX.h.repeat(55)}${ANSI.reset}`);

  return lines;
}

function buildAmp(p: ProviderQuota, mode: DisplayMode): string[] {
  const lines: string[] = [];
  const vc = PROVIDER_ANSI.amp;
  const _ampMeta: Record<string, string> | undefined =
    p.provider === 'amp' ? (p.extra as AmpQuotaExtra | undefined)?.meta : undefined;
  const m: Record<string, string> = _ampMeta !== undefined ? _ampMeta : {};

  lines.push(
    `${vc}${BOX.tl}${BOX.h}${ANSI.reset} ${vc}${ANSI.bold}Amp${ANSI.reset} ${vc}${BOX.h.repeat(53)}${ANSI.reset}`,
  );
  lines.push(v(vc));

  if (p.error) {
    lines.push(`${v(vc)}  ${ANSI.red}⚠️ ${p.error}${ANSI.reset}`);
  } else {
    // Thin tree connectors
    const tee = `${ANSI.comment}├─${ANSI.reset}`;
    const end = `${ANSI.comment}└─${ANSI.reset}`;

    // Free Tier
    const free = p.models?.['Free Tier'];
    if (free) {
      const disp = toDisplay(free.remaining, mode);
      lines.push(label('Free Tier', vc));
      const barS = bar(disp, mode);
      const pctS = `${getColor(disp, mode)}${formatPercent(disp).padStart(4)}${ANSI.reset}`;
      lines.push(`${v(vc)}  ${indicator(disp, mode)} ${barS} ${pctS}`);

      // Build sub-details
      const subs: string[] = [];

      const dollarParts: string[] = [];
      if (m.replenishRate) dollarParts.push(`${ANSI.cyan}${m.replenishRate}${ANSI.reset}`);
      const dollars = [m.freeRemaining, m.freeTotal].filter(Boolean).join(' / ');
      if (dollars) dollarParts.push(`${ANSI.text}( ${dollars} )${ANSI.reset}`);
      if (m.bonus) dollarParts.push(`${ANSI.cyan}${m.bonus}${ANSI.reset}`);
      if (dollarParts.length > 0) subs.push(dollarParts.join('  '));

      if (free.resetsAt && free.remaining !== 100) {
        subs.push(
          `${ANSI.cyan}${etaLabel(mode)} ${formatEta(free.resetsAt, free.remaining)}  ${formatResetTime(free.resetsAt, free.remaining)}${ANSI.reset}`,
        );
      }

      for (let i = 0; i < subs.length; i++) {
        const conn = i === subs.length - 1 ? end : tee;
        lines.push(`${v(vc)}  ${conn} ${subs[i]}`);
      }
    }

    // Credits
    const credits = p.models?.Credits;
    if (credits) {
      lines.push(v(vc));
      const balance = m.creditsBalance ?? '$0';
      const color = credits.remaining > 0 ? ANSI.green : ANSI.comment;
      lines.push(label('Credits', vc));
      lines.push(`${v(vc)}  ${indicator(toDisplay(credits.remaining, mode), mode)} ${color}${balance}${ANSI.reset}`);
    }

    // Fallback for unknown models
    if (!free && !credits && p.models && Object.keys(p.models).length > 0) {
      const entries = Object.entries(p.models);
      const maxLen = Math.max(...entries.map(([name]) => name.length), 20);
      lines.push(label('Usage', vc));
      for (const [name, window] of entries) {
        const disp = toDisplay(window.remaining, mode);
        const nameS = `${ANSI.textBright}${name.padEnd(maxLen)}${ANSI.reset}`;
        const barS = bar(disp, mode);
        const pctS = `${getColor(disp, mode)}${formatPercent(disp).padStart(4)}${ANSI.reset}`;
        lines.push(`${v(vc)}  ${indicator(disp, mode)} ${nameS} ${barS} ${pctS}`);
      }
    }
  }

  if (p.account) {
    lines.push(v(vc));
    lines.push(`${v(vc)}  ${ANSI.comment}Account: ${p.account}${ANSI.reset}`);
  }

  lines.push(v(vc));
  lines.push(`${vc}${BOX.bl}${BOX.h.repeat(55)}${ANSI.reset}`);

  return lines;
}

function formatCount(value: number): string {
  if (!Number.isFinite(value)) return '0';
  return Number.isInteger(value) ? value.toString() : value.toFixed(1);
}

function formatRawPercent(value: number): string {
  if (!Number.isFinite(value)) return '0%';
  return `${Number.isInteger(value) ? value : value.toFixed(1)}%`;
}

function boundedPercent(value: number | null): number | null {
  if (value === null) return null;
  return Math.max(0, Math.min(100, value));
}

function copilotUsedPercent(snapshot: CopilotQuotaSnapshot | undefined): number | null {
  if (!snapshot || snapshot.isUnlimitedEntitlement || snapshot.entitlementRequests <= 0) {
    return null;
  }
  return (snapshot.usedRequests / snapshot.entitlementRequests) * 100;
}

function copilotDisplayValue(
  snapshot: CopilotQuotaSnapshot | undefined,
  remaining: number | null,
  mode: DisplayMode,
): number | null {
  if (mode === 'used') {
    const used = copilotUsedPercent(snapshot);
    if (used !== null) return used;
  }
  return toDisplay(remaining, mode);
}

function copilotSnapshotDetail(snapshot: CopilotQuotaSnapshot): string {
  const parts: string[] = [];

  if (snapshot.isUnlimitedEntitlement) {
    parts.push(`${ANSI.cyan}Unlimited${ANSI.reset}`);
  } else {
    parts.push(
      `${ANSI.text}${formatCount(snapshot.usedRequests)} / ${formatCount(snapshot.entitlementRequests)} used${ANSI.reset}`,
    );
    parts.push(`${ANSI.comment}raw ${formatRawPercent(snapshot.remainingPercentage)}${ANSI.reset}`);
  }

  if (snapshot.overage > 0) {
    parts.push(`${ANSI.orange}${formatCount(snapshot.overage)} overage${ANSI.reset}`);
  }

  if (snapshot.usageAllowedWithExhaustedQuota || snapshot.overageAllowedWithExhaustedQuota) {
    parts.push(`${ANSI.cyan}usage allowed${ANSI.reset}`);
  }

  return parts.join(`${ANSI.comment}  |  ${ANSI.reset}`);
}

function buildCopilot(p: ProviderQuota, mode: DisplayMode): string[] {
  const lines: string[] = [];
  const vc = PROVIDER_ANSI.copilot;
  const extra = p.provider === 'copilot' ? (p.extra as CopilotQuotaExtra | undefined) : undefined;
  const snapshots = extra?.quotaSnapshots ?? {};

  lines.push(
    `${vc}${BOX.tl}${BOX.h}${ANSI.reset} ${vc}${ANSI.bold}Copilot${ANSI.reset} ${vc}${BOX.h.repeat(49)}${ANSI.reset}`,
  );
  lines.push(v(vc));

  if (p.error) {
    lines.push(`${v(vc)}  ${ANSI.red}⚠️ ${p.error}${ANSI.reset}`);
  } else {
    const orderedBuckets = [
      ...['premium_interactions', 'chat', 'completions'].filter((bucket) => snapshots[bucket]),
      ...Object.keys(snapshots).filter((bucket) => !['premium_interactions', 'chat', 'completions'].includes(bucket)),
    ];

    if (orderedBuckets.length === 0) {
      lines.push(`${v(vc)}  ${ANSI.comment}No usage data${ANSI.reset}`);
    } else {
      const labels = orderedBuckets.map((bucket) => {
        if (bucket === 'premium_interactions') return 'Premium requests';
        if (bucket === 'chat') return 'Chat';
        if (bucket === 'completions') return 'Completions';
        return bucket.replace(/[_-]+/g, ' ').replace(/\b\w/g, (char) => char.toUpperCase());
      });
      const maxLen = Math.max(...labels.map((name) => name.length), 20);

      lines.push(label('Usage', vc));
      for (let i = 0; i < orderedBuckets.length; i++) {
        const bucket = orderedBuckets[i];
        const name = labels[i];
        const snapshot = snapshots[bucket];
        const window = p.models?.[name];
        const rem = window?.remaining ?? null;
        const disp = copilotDisplayValue(snapshot, rem, mode);
        const nameS = `${ANSI.textBright}${name.padEnd(maxLen)}${ANSI.reset}`;
        const barS = bar(boundedPercent(disp), mode);
        const pctS = `${getColor(disp, mode)}${formatPercent(disp).padStart(4)}${ANSI.reset}`;
        const etaS = window?.resetsAt
          ? `${ANSI.cyan}→ ${formatEta(window.resetsAt, rem)} ${formatResetTime(window.resetsAt, rem)}${ANSI.reset}`
          : `${ANSI.cyan}→ N/A${ANSI.reset}`;

        lines.push(`${v(vc)}  ${indicator(disp, mode)} ${nameS} ${barS} ${pctS} ${etaS}`);
        lines.push(`${v(vc)}  ${ANSI.comment}${BOX.dotO}${ANSI.reset} ${copilotSnapshotDetail(snapshot)}`);
      }
    }
  }

  if (p.account) {
    lines.push(v(vc));
    lines.push(`${v(vc)}  ${ANSI.comment}Account: ${p.account}${ANSI.reset}`);
  }

  lines.push(v(vc));
  lines.push(`${vc}${BOX.bl}${BOX.h.repeat(55)}${ANSI.reset}`);

  return lines;
}

// ---------------------------------------------------------------------------
// Terminal builder registry
// ---------------------------------------------------------------------------

type TerminalBuilder = (p: ProviderQuota, mode: DisplayMode) => string[];

const TERMINAL_BUILDERS: Record<string, TerminalBuilder> = {
  claude: buildClaude,
  codex: buildCodex,
  copilot: buildCopilot,
  amp: buildAmp,
};

function buildGenericTerminal(p: ProviderQuota, mode: DisplayMode): string[] {
  const vc = ANSI.text;
  const vi = (c: string) => `${c}${BOX.v}${ANSI.reset}`;
  const lines: string[] = [];
  const name = p.displayName ?? p.provider;

  lines.push(
    `${vc}${BOX.tl}${BOX.h}${ANSI.reset} ${vc}${name}${ANSI.reset} ${vc}${BOX.h.repeat(Math.max(1, 55 - name.length - 3))}${ANSI.reset}`,
  );

  if (p.error) {
    lines.push(`${vi(vc)}  ${ANSI.red}${p.error}${ANSI.reset}`);
  } else if (p.primary) {
    const rem = p.primary.remaining;
    const disp = toDisplay(rem, mode);
    const color = getColor(disp, mode);
    const suffix = mode === 'used' ? 'used' : 'remaining';
    lines.push(`${vi(vc)}  ${color}${formatPercent(disp)} ${suffix}${ANSI.reset}`);
  }

  lines.push(`${vc}${BOX.bl}${BOX.h.repeat(55)}${ANSI.reset}`);
  return lines;
}

export function formatForTerminal(quotas: AllQuotas, mode: DisplayMode = 'remaining'): string {
  const sections: string[][] = [];

  for (const p of quotas.providers) {
    if (!p.available && !p.error) continue;
    const builder = TERMINAL_BUILDERS[p.provider];
    sections.push(builder ? builder(p, mode) : buildGenericTerminal(p, mode));
  }

  if (sections.length === 0) {
    return `${ANSI.comment}No providers connected${ANSI.reset}`;
  }

  return sections.map((s) => s.join('\n')).join('\n\n');
}

export function outputTerminal(quotas: AllQuotas, mode: DisplayMode = 'remaining'): void {
  console.log(formatForTerminal(quotas, mode));
}
