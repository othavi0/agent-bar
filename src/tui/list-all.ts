import * as p from '@clack/prompts';
import { applyCodexModelFilter, codexModelsFromQuota } from '../formatters/codex-helpers';
import { formatEta, formatPercent, formatResetTime, normalizePlanLabel } from '../formatters/shared';
import { getAllQuotas } from '../providers';
import type {
  ClaudeQuotaExtra,
  CodexQuotaExtra,
  CopilotQuotaExtra,
  CopilotQuotaSnapshot,
  ProviderQuota,
  QuotaWindow,
} from '../providers/types';
import { loadSettingsSync, type WindowPolicy } from '../settings';
import { BOX as B } from '../theme';
import { colorize, getQuotaColor, oneDark, semantic } from './colors';

function bar(pct: number | null): string {
  if (pct === null) return colorize('░'.repeat(20), semantic.muted);
  const filled = Math.floor(pct / 5);
  const color = getQuotaColor(pct);
  return colorize('█'.repeat(filled), color) + colorize('░'.repeat(20 - filled), semantic.muted);
}

function indicator(val: number | null): string {
  if (val === null) return colorize(B.dotO, semantic.muted);
  return colorize(B.dot, getQuotaColor(val));
}

// Vertical bar
const v = (color: string) => colorize(B.v, color);

// Section label: ┣━ ◆ Label
const label = (text: string, providerColor: string) =>
  `${colorize(B.lt + B.h, providerColor)} ${colorize(`${B.diamond} ${text}`, oneDark.blue, true)}`;

// Model line
function modelLine(name: string, window: QuotaWindow | undefined, maxLen: number, vColor: string): string {
  const rem = window?.remaining ?? null;
  const reset = window?.resetsAt ?? null;
  const nameS = colorize(name.padEnd(maxLen), oneDark.textBright);
  const barS = bar(rem);
  const pctS = colorize(formatPercent(rem).padStart(4), getQuotaColor(rem));
  const etaS = colorize(`→ ${formatEta(reset, rem)} ${formatResetTime(reset, rem)}`, oneDark.cyan);
  return `${v(vColor)}  ${indicator(rem)} ${nameS} ${barS} ${pctS} ${etaS}`;
}

function codexModelLine(name: string, window: QuotaWindow | undefined, maxLen: number, vColor: string): string {
  const rem = window?.remaining ?? null;
  const nameS = colorize(name.padEnd(maxLen), oneDark.textBright);
  const barS = bar(rem);
  const pctS = colorize(formatPercent(rem).padStart(4), getQuotaColor(rem));
  const etaS = window?.resetsAt
    ? colorize(`→ ${formatEta(window.resetsAt, rem)} ${formatResetTime(window.resetsAt, rem)}`, oneDark.cyan)
    : colorize('→ N/A', oneDark.cyan);
  return `${v(vColor)}  ${indicator(rem)} ${nameS} ${barS} ${pctS} ${etaS}`;
}

function buildClaude(p: ProviderQuota): string[] {
  const lines: string[] = [];
  const vc = oneDark.orange;

  lines.push(`${colorize(B.tl + B.h, vc)} ${colorize('Claude', vc, true)} ${colorize(B.h.repeat(50), vc)}`);
  lines.push(v(vc));

  if (p.error) {
    lines.push(`${v(vc)}  ${colorize(`⚠️ ${p.error}`, oneDark.red)}`);
  } else {
    const maxLen = 20;

    if (p.primary) {
      lines.push(label('5-hour limit', vc));
      for (const m of ['Opus', 'Sonnet', 'Haiku']) {
        lines.push(modelLine(m, p.primary, maxLen, vc));
      }
    }

    const _claudeExtra = p.provider === 'claude' ? (p.extra as ClaudeQuotaExtra | undefined) : undefined;
    const weeklyModels = _claudeExtra?.weeklyModels;
    if (weeklyModels && Object.keys(weeklyModels).length > 0) {
      lines.push(v(vc));
      lines.push(label('Weekly limit', vc));
      const entries = Object.entries(weeklyModels);
      const maxLenWeekly = Math.max(...entries.map(([name]) => name.length), maxLen);
      for (const [name, window] of entries) {
        lines.push(modelLine(name, window, maxLenWeekly, vc));
      }
    } else if (p.secondary) {
      lines.push(v(vc));
      lines.push(label('Weekly limit', vc));
      lines.push(modelLine('All Models', p.secondary, maxLen, vc));
    }

    if (_claudeExtra?.extraUsage?.enabled && _claudeExtra.extraUsage.limit > 0) {
      const { remaining, used, limit } = _claudeExtra.extraUsage;
      lines.push(v(vc));
      lines.push(label('Extra Usage', vc));
      const nameS = colorize('Budget'.padEnd(maxLen), oneDark.textBright);
      const barS = bar(remaining);
      const pctS = colorize(formatPercent(remaining).padStart(4), getQuotaColor(remaining));
      const usedS = colorize(`$${(used / 100).toFixed(2)}/$${(limit / 100).toFixed(2)}`, oneDark.cyan);
      lines.push(`${v(vc)}  ${indicator(remaining)} ${nameS} ${barS} ${pctS} ${usedS}`);
    }
  }

  lines.push(v(vc));
  lines.push(colorize(B.bl + B.h.repeat(55), vc));

  return lines;
}

function buildCodex(p: ProviderQuota): string[] {
  const lines: string[] = [];
  const vc = oneDark.green;
  const settings = loadSettingsSync();
  const policy: WindowPolicy = settings.windowPolicy?.[p.provider] ?? 'both';
  const planLabel = normalizePlanLabel(p);

  lines.push(`${colorize(B.tl + B.h, vc)} ${colorize('Codex', vc, true)} ${colorize(B.h.repeat(51), vc)}`);
  lines.push(v(vc));

  if (p.error) {
    lines.push(`${v(vc)}  ${colorize(`⚠️ ${p.error}`, oneDark.red)}`);
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

    const _codexExtra = p.provider === 'codex' ? (p.extra as CodexQuotaExtra | undefined) : undefined;
    if (_codexExtra?.extraUsage?.enabled) {
      const codexExtraUsage = _codexExtra.extraUsage;
      lines.push(v(vc));
      lines.push(label('Credits', vc));
      const nameS = colorize('Balance'.padEnd(maxLen), oneDark.textBright);
      const barS = bar(codexExtraUsage.remaining);
      const pctS = colorize(
        formatPercent(codexExtraUsage.remaining).padStart(4),
        getQuotaColor(codexExtraUsage.remaining),
      );
      const infoS =
        codexExtraUsage.limit === -1 ? colorize('Unlimited', oneDark.cyan) : colorize('Balance', oneDark.cyan);
      lines.push(`${v(vc)}  ${indicator(codexExtraUsage.remaining)} ${nameS} ${barS} ${pctS} ${infoS}`);
    }
  }

  lines.push(v(vc));
  lines.push(colorize(B.bl + B.h.repeat(55), vc));

  return lines;
}

function buildAmp(p: ProviderQuota): string[] {
  const lines: string[] = [];
  const vc = oneDark.magenta;

  lines.push(`${colorize(B.tl + B.h, vc)} ${colorize('Amp', vc, true)} ${colorize(B.h.repeat(53), vc)}`);
  lines.push(v(vc));

  if (p.error) {
    lines.push(`${v(vc)}  ${colorize(`⚠️ ${p.error}`, oneDark.red)}`);
  } else if (!p.models || Object.keys(p.models).length === 0) {
    lines.push(`${v(vc)}  ${colorize('No usage data', semantic.muted)}`);
  } else {
    const entries = Object.entries(p.models);
    const maxLen = Math.max(...entries.map(([name]) => name.length), 20);

    lines.push(label('Usage', vc));
    for (const [name, window] of entries) {
      const nameS = colorize(name.padEnd(maxLen), oneDark.textBright);
      const barS = bar(window.remaining);
      const pctS = colorize(formatPercent(window.remaining).padStart(4), getQuotaColor(window.remaining));
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

function formatCount(value: number): string {
  if (!Number.isFinite(value)) return '0';
  return Number.isInteger(value) ? value.toString() : value.toFixed(1);
}

function formatRawPercent(value: number): string {
  if (!Number.isFinite(value)) return '0%';
  return `${Number.isInteger(value) ? value : value.toFixed(1)}%`;
}

function copilotSnapshotDetail(snapshot: CopilotQuotaSnapshot): string {
  const parts: string[] = [];

  if (snapshot.isUnlimitedEntitlement) {
    parts.push(colorize('Unlimited', oneDark.cyan));
  } else {
    parts.push(
      colorize(
        `${formatCount(snapshot.usedRequests)} / ${formatCount(snapshot.entitlementRequests)} used`,
        oneDark.text,
      ),
    );
    parts.push(colorize(`raw ${formatRawPercent(snapshot.remainingPercentage)}`, semantic.muted));
  }

  if (snapshot.overage > 0) {
    parts.push(colorize(`${formatCount(snapshot.overage)} overage`, oneDark.orange));
  }

  if (snapshot.usageAllowedWithExhaustedQuota || snapshot.overageAllowedWithExhaustedQuota) {
    parts.push(colorize('usage allowed', oneDark.cyan));
  }

  return parts.join(colorize('  |  ', semantic.muted));
}

function buildCopilot(p: ProviderQuota): string[] {
  const lines: string[] = [];
  const vc = oneDark.brightBlue;
  const extra = p.provider === 'copilot' ? (p.extra as CopilotQuotaExtra | undefined) : undefined;
  const snapshots = extra?.quotaSnapshots ?? {};

  lines.push(`${colorize(B.tl + B.h, vc)} ${colorize('Copilot', vc, true)} ${colorize(B.h.repeat(49), vc)}`);
  lines.push(v(vc));

  if (p.error) {
    lines.push(`${v(vc)}  ${colorize(`⚠️ ${p.error}`, oneDark.red)}`);
  } else if (Object.keys(snapshots).length === 0) {
    lines.push(`${v(vc)}  ${colorize('No usage data', semantic.muted)}`);
  } else {
    const orderedBuckets = [
      ...['premium_interactions', 'chat', 'completions'].filter((bucket) => snapshots[bucket]),
      ...Object.keys(snapshots).filter((bucket) => !['premium_interactions', 'chat', 'completions'].includes(bucket)),
    ];
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
      const nameS = colorize(name.padEnd(maxLen), oneDark.textBright);
      const barS = bar(rem);
      const pctS = colorize(formatPercent(rem).padStart(4), getQuotaColor(rem));
      const etaS = window?.resetsAt
        ? colorize(`→ ${formatEta(window.resetsAt, rem)} ${formatResetTime(window.resetsAt, rem)}`, oneDark.cyan)
        : colorize('→ N/A', oneDark.cyan);

      lines.push(`${v(vc)}  ${indicator(rem)} ${nameS} ${barS} ${pctS} ${etaS}`);
      lines.push(`${v(vc)}  ${colorize(B.dotO, semantic.muted)} ${copilotSnapshotDetail(snapshot)}`);
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

export async function showListAll(): Promise<void> {
  const s = p.spinner();
  s.start('Loading quotas...');

  const quotas = await getAllQuotas();

  s.stop('Quotas loaded');

  // Build output
  const sections: string[][] = [];

  for (const provider of quotas.providers) {
    if (!provider.available && !provider.error) continue;

    switch (provider.provider) {
      case 'claude':
        sections.push(buildClaude(provider));
        break;
      case 'codex':
        sections.push(buildCodex(provider));
        break;
      case 'copilot':
        sections.push(buildCopilot(provider));
        break;
      case 'amp':
        sections.push(buildAmp(provider));
        break;
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

  // Wait for enter — always restore raw mode even if an error occurs
  process.stdin.setRawMode?.(true);
  process.stdin.resume();
  try {
    await new Promise<void>((resolve) => {
      process.stdin.once('data', () => resolve());
    });
  } finally {
    process.stdin.setRawMode?.(false);
    process.stdin.pause();
  }
}
