import { getClaudeExtra } from '../../providers/extras';
import type { ProviderQuota, QuotaWindow } from '../../providers/types';
import { BOX } from '../../theme';
import { barSegments, colorForDisplay, indicatorSegments, type Line, type Segment } from '../segments';
import { formatEta, formatPercent, formatResetTime, toDisplay } from '../shared';
import type { BuildOptions } from './types';

const TOOLTIP_BORDER = 56;

// ---------------------------------------------------------------------------
// Segment helpers
// ---------------------------------------------------------------------------

/** Literal (uncolored) connector text — raw segment skips span/ANSI wrapping. */
const raw = (text: string): Segment => ({ text, color: 'text', raw: true });

/** A full model line (vertical bar + padding + indicator + name + bar + pct + eta). */
function modelLine(name: string, window: QuotaWindow | undefined, maxLen: number, mode: BuildOptions['mode']): Line {
  const rem = window?.remaining ?? null;
  const reset = window?.resetsAt ?? null;
  const disp = toDisplay(rem, mode);
  return [
    { text: BOX.v, color: 'orange' },
    raw('  '),
    ...indicatorSegments(disp, mode),
    raw(' '),
    { text: name.padEnd(maxLen), color: 'textBright' },
    raw(' '),
    ...barSegments(disp, mode),
    raw(' '),
    { text: formatPercent(disp).padStart(4), color: colorForDisplay(disp, mode) },
    raw(' '),
    { text: `→ ${formatEta(reset, rem)} ${formatResetTime(reset, rem)}`, color: 'cyan' },
  ];
}

/** Section label line: ┣━ ◆ Label */
function labelLine(text: string, labelColor: BuildOptions['labelColor']): Line {
  return [
    { text: BOX.lt + BOX.h, color: 'orange' },
    raw(' '),
    { text: `${BOX.diamond} ${text}`, color: labelColor, bold: true },
  ];
}

/** Vertical bar line (empty row with provider border). */
const vLine = (): Line => [{ text: BOX.v, color: 'orange' }];

/** Extra Usage row. */
function extraUsageLine(
  name: string,
  maxLen: number,
  disp: number | null,
  mode: BuildOptions['mode'],
  usedStr: string,
): Line {
  return [
    { text: BOX.v, color: 'orange' },
    raw('  '),
    ...indicatorSegments(disp, mode),
    raw(' '),
    { text: name.padEnd(maxLen), color: 'textBright' },
    raw(' '),
    ...barSegments(disp, mode),
    raw(' '),
    { text: formatPercent(disp).padStart(4), color: colorForDisplay(disp, mode) },
    raw(' '),
    { text: usedStr, color: 'cyan' },
  ];
}

// ---------------------------------------------------------------------------
// Footer helpers
// ---------------------------------------------------------------------------

function formatAgo(iso: string): string {
  const diffMs = Date.now() - new Date(iso).getTime();
  if (diffMs < 60000) return 'just now';
  const mins = Math.floor(diffMs / 60000);
  if (mins < 60) return `${mins}m ago`;
  return `${Math.floor(mins / 60)}h ago`;
}

function buildFooterLine(footer: BuildOptions['footer']): Line {
  const fetchedAt = footer?.fetchedAt;
  if (fetchedAt) {
    const ago = formatAgo(fetchedAt);
    const stamp = ` cached · ${ago} `;
    const totalDashes = TOOLTIP_BORDER - 1 - stamp.length;
    const left = Math.max(1, Math.floor(totalDashes / 2));
    const right = Math.max(1, totalDashes - left);
    return [
      { text: BOX.bl + BOX.h.repeat(left), color: 'orange' },
      { text: stamp, color: 'comment' },
      { text: BOX.h.repeat(right), color: 'orange' },
    ];
  }
  // Plain footer (terminal / TUI / waybar without fetchedAt)
  return [{ text: BOX.bl + BOX.h.repeat(55), color: 'orange' }];
}

// ---------------------------------------------------------------------------
// Main builder
// ---------------------------------------------------------------------------

/**
 * Pure builder for the Claude provider card.
 *
 * Emits Line[] with no I/O, no settings reads, no markup.
 * Surface-specific differences (header title/fill, footer stamp) are driven by `options`.
 */
export function buildClaude(p: ProviderQuota, options: BuildOptions): Line[] {
  const { mode, headerTitle, headerWidth, labelColor, footer } = options;
  const headerFill = Math.max(1, headerWidth - headerTitle.length);
  const lines: Line[] = [];

  // Header
  lines.push([
    { text: BOX.tl + BOX.h, color: 'orange' },
    raw(' '),
    { text: headerTitle, color: 'orange', bold: true },
    raw(' '),
    { text: BOX.h.repeat(headerFill), color: 'orange' },
  ]);
  lines.push(vLine());

  if (p.error) {
    lines.push([{ text: BOX.v, color: 'orange' }, raw('  '), { text: `⚠️ ${p.error}`, color: 'red' }]);
  } else {
    const maxLen = 20;

    if (p.primary) {
      lines.push(labelLine('5-hour limit (shared)', labelColor));
      lines.push(modelLine('All Models', p.primary, maxLen, mode));
    }

    // Per-model weekly quotas (when API provides them)
    const claudeExtra = getClaudeExtra(p);
    const weeklyModels = claudeExtra?.weeklyModels;
    if (weeklyModels && Object.keys(weeklyModels).length > 0) {
      lines.push(vLine());
      lines.push(labelLine('Weekly per model', labelColor));
      const entries = Object.entries(weeklyModels);
      const wMaxLen = Math.max(...entries.map(([name]) => name.length), maxLen);
      for (const [name, window] of entries) {
        lines.push(modelLine(name, window, wMaxLen, mode));
      }
    }

    // Generic weekly (shared)
    if (p.secondary) {
      lines.push(vLine());
      lines.push(labelLine('Weekly limit (shared)', labelColor));
      lines.push(modelLine('All Models', p.secondary, maxLen, mode));
    }

    if (claudeExtra?.extraUsage?.enabled && claudeExtra.extraUsage.limit > 0) {
      const { remaining, used, limit } = claudeExtra.extraUsage;
      const disp = toDisplay(remaining, mode);
      lines.push(vLine());
      lines.push(labelLine('Extra Usage', labelColor));
      lines.push(
        extraUsageLine('Budget', maxLen, disp, mode, `$${(used / 100).toFixed(2)}/$${(limit / 100).toFixed(2)}`),
      );
    }
  }

  lines.push(vLine());
  lines.push(buildFooterLine(footer));

  return lines;
}
