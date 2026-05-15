import { getCodexExtra } from '../../providers/extras';
import type { ProviderQuota, QuotaWindow } from '../../providers/types';
import { BOX } from '../../theme';
import { barSegments, colorForDisplay, indicatorSegments, type Line, type Segment } from '../segments';
import { formatEta, formatPercent, formatResetTime, toDisplay } from '../shared';
import type { CodexViewModel } from '../view-model';
import type { BuildOptions } from './types';

const TOOLTIP_BORDER = 56;

// ---------------------------------------------------------------------------
// Segment helpers — Codex provider color is 'green'
// ---------------------------------------------------------------------------

/** Literal (uncolored) connector text. */
const raw = (text: string): Segment => ({ text, color: 'text', raw: true });

/** Vertical bar line (Codex green border). */
const vLine = (): Line => [{ text: BOX.v, color: 'green' }];

/** Section label line: ┣━ ◆ Label (connector green, label uses labelColor from options). */
function labelLine(text: string, labelColor: BuildOptions['labelColor']): Line {
  return [
    { text: BOX.lt + BOX.h, color: 'green' },
    raw(' '),
    { text: `${BOX.diamond} ${text}`, color: labelColor, bold: true },
  ];
}

/** A single Codex model line: ┃  ● Name             ████░░░░  X% → ETA (time). */
function codexModelLine(
  name: string,
  window: QuotaWindow | undefined,
  maxLen: number,
  mode: BuildOptions['mode'],
): Line {
  const rem = window?.remaining ?? null;
  const disp = toDisplay(rem, mode);
  const etaText = window?.resetsAt
    ? `→ ${formatEta(window.resetsAt, rem)} ${formatResetTime(window.resetsAt, rem)}`
    : '→ N/A';
  return [
    { text: BOX.v, color: 'green' },
    raw('  '),
    ...indicatorSegments(disp, mode),
    raw(' '),
    { text: name.padEnd(maxLen), color: 'textBright' },
    raw(' '),
    ...barSegments(disp, mode),
    raw(' '),
    { text: formatPercent(disp).padStart(4), color: colorForDisplay(disp, mode) },
    raw(' '),
    { text: etaText, color: 'cyan' },
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
      { text: BOX.bl + BOX.h.repeat(left), color: 'green' },
      { text: stamp, color: 'comment' },
      { text: BOX.h.repeat(right), color: 'green' },
    ];
  }
  return [{ text: BOX.bl + BOX.h.repeat(55), color: 'green' }];
}

// ---------------------------------------------------------------------------
// Main builder
// ---------------------------------------------------------------------------

/**
 * Pure builder for the Codex provider card.
 *
 * Emits Line[] with no I/O, no settings reads, no markup.
 * Surface-specific differences (header title/fill, labelColor, planLabel, footer stamp)
 * are driven by `options` and the already-resolved `viewModel`.
 *
 * Per-surface labelColor:
 *   - Terminal: 'magenta'  (mirrors the old `label` helper using ANSI.magenta)
 *   - Waybar:   'green'    (provider color; old label used PROVIDER_HEX.codex = ONE_DARK.green)
 *   - TUI:      'blue'     (mirrors the old `label` helper using oneDark.blue)
 *
 * planLabel is used for the "Plan: X" row emitted in terminal/TUI but NOT in waybar
 * (waybar embeds the plan into headerTitle instead).
 */
export function buildCodex(p: ProviderQuota, viewModel: CodexViewModel, options: BuildOptions): Line[] {
  const { mode, headerTitle, headerWidth, labelColor, footer, planLabel } = options;
  const headerFill = Math.max(1, headerWidth - headerTitle.length);
  const lines: Line[] = [];

  // Header
  lines.push([
    { text: BOX.tl + BOX.h, color: 'green' },
    raw(' '),
    { text: headerTitle, color: 'green', bold: true },
    raw(' '),
    { text: BOX.h.repeat(headerFill), color: 'green' },
  ]);
  lines.push(vLine());

  if (p.error) {
    lines.push([{ text: BOX.v, color: 'green' }, raw('  '), { text: `⚠️ ${p.error}`, color: 'red' }]);
  } else {
    const { models, policy } = viewModel;
    const maxLen = 20;

    // Plan line — terminal/TUI only (waybar embeds plan in headerTitle)
    if (planLabel !== undefined) {
      lines.push([{ text: BOX.v, color: 'green' }, raw('  '), { text: `Plan: ${planLabel}`, color: 'muted' }]);
    }

    if (models.length === 0) {
      lines.push(vLine());
      lines.push(labelLine('Available Models', labelColor));
      lines.push([{ text: BOX.v, color: 'green' }, raw('  '), { text: 'No models selected', color: 'comment' }]);
    } else {
      const modelLen = Math.max(...models.map((m) => m.name.length), maxLen);

      if (policy !== 'seven_day') {
        lines.push(vLine());
        lines.push(labelLine('5-hour limit', labelColor));
        for (const model of models) {
          lines.push(codexModelLine(model.name, model.windows.fiveHour, modelLen, mode));
        }
      }

      if (policy !== 'five_hour') {
        lines.push(vLine());
        lines.push(labelLine('7-day limit', labelColor));
        for (const model of models) {
          lines.push(codexModelLine(model.name, model.windows.sevenDay, modelLen, mode));
        }
      }
    }

    const codexExtra = getCodexExtra(p);
    if (codexExtra?.extraUsage?.enabled) {
      const { remaining, limit } = codexExtra.extraUsage;
      const disp = toDisplay(remaining, mode);
      lines.push(vLine());
      lines.push(labelLine('Credits', labelColor));
      const limitText = limit === -1 ? 'Unlimited' : 'Balance';
      lines.push([
        { text: BOX.v, color: 'green' },
        raw('  '),
        ...indicatorSegments(disp, mode),
        raw(' '),
        { text: 'Balance'.padEnd(maxLen), color: 'textBright' },
        raw(' '),
        ...barSegments(disp, mode),
        raw(' '),
        { text: formatPercent(disp).padStart(4), color: colorForDisplay(disp, mode) },
        raw(' '),
        { text: limitText, color: 'cyan' },
      ]);
    }
  }

  lines.push(vLine());
  lines.push(buildFooterLine(footer));

  return lines;
}
