import * as p from '@clack/prompts';
import { buildAmp as buildAmpLines } from '../formatters/builders/amp';
import { buildClaude } from '../formatters/builders/claude';
import { buildCodex as buildCodexLines } from '../formatters/builders/codex';
import { buildCopilot as buildCopilotLines } from '../formatters/builders/copilot';
import { formatEta, formatPercent, formatResetTime, normalizePlanLabel } from '../formatters/shared';
import { resolveCodexViewModel } from '../formatters/view-model';
import { getAllQuotas } from '../providers';
import { getCopilotExtra } from '../providers/extras';
import type { CopilotQuotaSnapshot, ProviderQuota, QuotaWindow } from '../providers/types';
import { BOX as B } from '../theme';
import { colorize, getQuotaColor, oneDark, semantic } from './colors';
import { renderColorize } from './render-colorize';

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

// Model line (kept for reference; no longer called after Claude migration — Task 8 removes it)
function _modelLine(name: string, window: QuotaWindow | undefined, maxLen: number, vColor: string): string {
  const rem = window?.remaining ?? null;
  const reset = window?.resetsAt ?? null;
  const nameS = colorize(name.padEnd(maxLen), oneDark.textBright);
  const barS = bar(rem);
  const pctS = colorize(formatPercent(rem).padStart(4), getQuotaColor(rem));
  const etaS = colorize(`→ ${formatEta(reset, rem)} ${formatResetTime(reset, rem)}`, oneDark.cyan);
  return `${v(vColor)}  ${indicator(rem)} ${nameS} ${barS} ${pctS} ${etaS}`;
}

function _codexModelLine(name: string, window: QuotaWindow | undefined, maxLen: number, vColor: string): string {
  const rem = window?.remaining ?? null;
  const nameS = colorize(name.padEnd(maxLen), oneDark.textBright);
  const barS = bar(rem);
  const pctS = colorize(formatPercent(rem).padStart(4), getQuotaColor(rem));
  const etaS = window?.resetsAt
    ? colorize(`→ ${formatEta(window.resetsAt, rem)} ${formatResetTime(window.resetsAt, rem)}`, oneDark.cyan)
    : colorize('→ N/A', oneDark.cyan);
  return `${v(vColor)}  ${indicator(rem)} ${nameS} ${barS} ${pctS} ${etaS}`;
}

function buildClaudeTui(provider: ProviderQuota): string[] {
  const rendered = renderColorize(
    buildClaude(provider, {
      mode: 'remaining',
      headerTitle: 'Claude',
      headerWidth: 56,
      labelColor: 'blue',
      footer: undefined,
    }),
  );
  return rendered.split('\n');
}

function buildCodexTui(provider: ProviderQuota): string[] {
  const viewModel = resolveCodexViewModel(provider);
  const planLabel = normalizePlanLabel(provider);
  const rendered = renderColorize(
    buildCodexLines(provider, viewModel, {
      mode: 'remaining',
      headerTitle: 'Codex',
      headerWidth: 56,
      labelColor: 'blue',
      planLabel,
      footer: undefined,
    }),
  );
  return rendered.split('\n');
}

function buildAmpTui(provider: ProviderQuota): string[] {
  const rendered = renderColorize(
    buildAmpLines(provider, {
      mode: 'remaining',
      headerTitle: 'Amp',
      headerWidth: 56,
      labelColor: 'blue',
      ampFreeTierLayout: 'generic',
      footer: undefined,
    }),
  );
  return rendered.split('\n');
}

function buildCopilotTui(provider: ProviderQuota): string[] {
  const rendered = renderColorize(
    buildCopilotLines(provider, {
      mode: 'remaining',
      headerTitle: 'Copilot',
      headerWidth: 56,
      labelColor: 'blue',
      footer: undefined,
    }),
  );
  return rendered.split('\n');
}

function _buildAmp(p: ProviderQuota): string[] {
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
  const extra = getCopilotExtra(p);
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
        sections.push(buildClaudeTui(provider));
        break;
      case 'codex':
        sections.push(buildCodexTui(provider));
        break;
      case 'copilot':
        sections.push(buildCopilotTui(provider));
        break;
      case 'amp':
        sections.push(buildAmpTui(provider));
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
