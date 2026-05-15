import { getCopilotExtra } from '../../providers/extras';
import type { CopilotQuotaSnapshot, ProviderQuota } from '../../providers/types';
import { BOX } from '../../theme';
import { barSegments, colorForDisplay, indicatorSegments, type Line } from '../segments';
import { formatEta, formatPercent, formatResetTime, toDisplay } from '../shared';
import { buildFooterLine, labelLine, raw, vLine } from './shared';
import type { BuildOptions } from './types';

// ---------------------------------------------------------------------------
// Per-surface `labelColor` (verified against the old builders):
//   Terminal: 'magenta'    — old `label()` hardcoded ANSI.magenta for the ◆
//   Waybar:   'brightBlue' — old label used PROVIDER_HEX.copilot = ONE_DARK.brightBlue
//   TUI:      'blue'       — old `label()` hardcoded oneDark.blue for the ◆
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Pure data helpers (Copilot-specific; no markup)
// ---------------------------------------------------------------------------

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
  mode: BuildOptions['mode'],
): number | null {
  if (mode === 'used') {
    const used = copilotUsedPercent(snapshot);
    if (used !== null) return used;
  }
  return toDisplay(remaining, mode);
}

// ---------------------------------------------------------------------------
// Snapshot detail line (produces a Line of Segments, no markup strings)
// ---------------------------------------------------------------------------

function copilotSnapshotDetailSegments(snapshot: CopilotQuotaSnapshot): Line {
  const parts: Line[] = [];

  if (snapshot.isUnlimitedEntitlement) {
    parts.push([{ text: 'Unlimited', color: 'cyan' }]);
  } else {
    parts.push([
      {
        text: `${formatCount(snapshot.usedRequests)} / ${formatCount(snapshot.entitlementRequests)} used`,
        color: 'text',
      },
    ]);
    parts.push([{ text: `raw ${formatRawPercent(snapshot.remainingPercentage)}`, color: 'comment' }]);
  }

  if (snapshot.overage > 0) {
    parts.push([{ text: `${formatCount(snapshot.overage)} overage`, color: 'orange' }]);
  }

  if (snapshot.usageAllowedWithExhaustedQuota || snapshot.overageAllowedWithExhaustedQuota) {
    parts.push([{ text: 'usage allowed', color: 'cyan' }]);
  }

  // Join parts with '  |  ' separator
  const joined: Line = [];
  for (let i = 0; i < parts.length; i++) {
    if (i > 0) joined.push({ text: '  |  ', color: 'comment' });
    joined.push(...parts[i]);
  }
  return joined;
}

// ---------------------------------------------------------------------------
// Bucket label normalisation
// ---------------------------------------------------------------------------

function bucketLabel(bucket: string): string {
  if (bucket === 'premium_interactions') return 'Premium requests';
  if (bucket === 'chat') return 'Chat';
  if (bucket === 'completions') return 'Completions';
  return bucket.replace(/[_-]+/g, ' ').replace(/\b\w/g, (char) => char.toUpperCase());
}

// ---------------------------------------------------------------------------
// Main builder
// ---------------------------------------------------------------------------

/**
 * Pure builder for the Copilot provider card.
 *
 * Emits Line[] with no I/O, no settings reads, no markup.
 * Surface-specific differences (header title/fill, labelColor, footer stamp) are driven by `options`.
 *
 * labelColor per surface:
 *   Terminal → 'magenta', Waybar → 'brightBlue', TUI → 'blue'
 */
export function buildCopilot(p: ProviderQuota, options: BuildOptions): Line[] {
  const { mode, headerTitle, headerWidth, labelColor, footer, accountInHeader } = options;
  const headerFill = Math.max(1, headerWidth - headerTitle.length);
  const lines: Line[] = [];

  const extra = getCopilotExtra(p);
  const snapshots = extra?.quotaSnapshots ?? {};

  // Header
  lines.push([
    { text: BOX.tl + BOX.h, color: 'brightBlue' },
    raw(' '),
    { text: headerTitle, color: 'brightBlue', bold: true },
    raw(' '),
    { text: BOX.h.repeat(headerFill), color: 'brightBlue' },
  ]);
  lines.push(vLine('brightBlue'));

  if (p.error) {
    lines.push([{ text: BOX.v, color: 'brightBlue' }, raw('  '), { text: `⚠️ ${p.error}`, color: 'red' }]);
  } else {
    const orderedBuckets = [
      ...['premium_interactions', 'chat', 'completions'].filter((bucket) => snapshots[bucket]),
      ...Object.keys(snapshots).filter((bucket) => !['premium_interactions', 'chat', 'completions'].includes(bucket)),
    ];

    if (orderedBuckets.length === 0) {
      lines.push([{ text: BOX.v, color: 'brightBlue' }, raw('  '), { text: 'No usage data', color: 'comment' }]);
    } else {
      const labels = orderedBuckets.map(bucketLabel);
      const maxLen = Math.max(...labels.map((name) => name.length), 20);

      lines.push(labelLine('Usage', labelColor, 'brightBlue'));

      for (let i = 0; i < orderedBuckets.length; i++) {
        const bucket = orderedBuckets[i];
        const name = labels[i];
        const snapshot = snapshots[bucket];
        const window = p.models?.[name];
        const rem = window?.remaining ?? null;
        const disp = copilotDisplayValue(snapshot, rem, mode);
        const boundedDisp = boundedPercent(disp);

        // ETA text: '→ ETA (time)' when resetsAt is known, '→ N/A' otherwise
        const etaText = window?.resetsAt
          ? `→ ${formatEta(window.resetsAt, rem)} ${formatResetTime(window.resetsAt, rem)}`
          : '→ N/A';

        // Usage bar line: ┃  ● Name             ████░░  X% → ETA
        lines.push([
          { text: BOX.v, color: 'brightBlue' },
          raw('  '),
          ...indicatorSegments(boundedDisp, mode),
          raw(' '),
          { text: name.padEnd(maxLen), color: 'textBright' },
          raw(' '),
          ...barSegments(boundedDisp, mode),
          raw(' '),
          { text: formatPercent(disp).padStart(4), color: colorForDisplay(disp, mode) },
          raw(' '),
          { text: etaText, color: 'cyan' },
        ]);

        // Snapshot detail line: ┃  ○ <detail>
        lines.push([
          { text: BOX.v, color: 'brightBlue' },
          raw('  '),
          { text: BOX.dotO, color: 'comment' },
          raw(' '),
          ...copilotSnapshotDetailSegments(snapshot),
        ]);
      }
    }
  }

  // Account line — omitted when the surface already shows the account in the header
  if (p.account && !accountInHeader) {
    lines.push(vLine('brightBlue'));
    lines.push([{ text: BOX.v, color: 'brightBlue' }, raw('  '), { text: `Account: ${p.account}`, color: 'comment' }]);
  }

  lines.push(vLine('brightBlue'));
  lines.push(buildFooterLine(footer, 'brightBlue'));

  return lines;
}
