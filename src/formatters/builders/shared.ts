import type { QuotaWindow } from '../../providers/types';
import { BOX } from '../../theme';
import { barSegments, type ColorToken, colorForDisplay, indicatorSegments, type Line, type Segment } from '../segments';
import { formatEta, formatPercent, formatResetTime, toDisplay } from '../shared';
import type { BuildOptions } from './types';

export const TOOLTIP_BORDER = 56;

// ---------------------------------------------------------------------------
// Segment helpers
// ---------------------------------------------------------------------------

/** Literal (uncolored) connector text — raw segment skips span/ANSI wrapping. */
export const raw = (text: string): Segment => ({ text, color: 'text', raw: true });

// ---------------------------------------------------------------------------
// Footer helpers
// ---------------------------------------------------------------------------

export function formatAgo(iso: string): string {
  const diffMs = Date.now() - new Date(iso).getTime();
  if (diffMs < 60000) return 'just now';
  const mins = Math.floor(diffMs / 60000);
  if (mins < 60) return `${mins}m ago`;
  return `${Math.floor(mins / 60)}h ago`;
}

/**
 * Footer line with optional cached stamp.
 * `color` is the provider accent color (e.g. 'orange' for Claude, 'green' for Codex).
 */
export function buildFooterLine(footer: BuildOptions['footer'], color: ColorToken): Line {
  const fetchedAt = footer?.fetchedAt;
  if (fetchedAt) {
    const ago = formatAgo(fetchedAt);
    const stamp = ` cached · ${ago} `;
    const totalDashes = TOOLTIP_BORDER - 1 - stamp.length;
    const left = Math.max(1, Math.floor(totalDashes / 2));
    const right = Math.max(1, totalDashes - left);
    return [
      { text: BOX.bl + BOX.h.repeat(left), color },
      { text: stamp, color: 'comment' },
      { text: BOX.h.repeat(right), color },
    ];
  }
  return [{ text: BOX.bl + BOX.h.repeat(55), color }];
}

// ---------------------------------------------------------------------------
// Section label
// ---------------------------------------------------------------------------

/**
 * Section label line: ┣━ ◆ Label
 * `connectorColor` is the provider accent color; `labelColor` comes from BuildOptions.
 */
export function labelLine(text: string, labelColor: BuildOptions['labelColor'], connectorColor: ColorToken): Line {
  return [
    { text: BOX.lt + BOX.h, color: connectorColor },
    raw(' '),
    { text: `${BOX.diamond} ${text}`, color: labelColor, bold: true },
  ];
}

// ---------------------------------------------------------------------------
// Model line
// ---------------------------------------------------------------------------

/**
 * A full model line: ┃  ● Name             ████░░░░  X% → ETA (time)
 *
 * `providerColor` — accent color for the vertical bar segment (e.g. 'orange', 'green').
 * `nullEtaText`   — when set and `window.resetsAt` is absent, use this as the ETA text
 *                   (e.g. 'N/A' for Codex → renders as '→ N/A').
 *                   When not set, falls through to `formatEta`/`formatResetTime` which
 *                   produce '?' / '(??:??)' (Claude behavior).
 */
export function modelLine(
  name: string,
  window: QuotaWindow | undefined,
  maxLen: number,
  mode: BuildOptions['mode'],
  providerColor: ColorToken,
  nullEtaText?: string,
): Line {
  const rem = window?.remaining ?? null;
  const reset = window?.resetsAt ?? null;
  const disp = toDisplay(rem, mode);

  let etaText: string;
  if (nullEtaText !== undefined && reset === null) {
    etaText = `→ ${nullEtaText}`;
  } else {
    etaText = `→ ${formatEta(reset, rem)} ${formatResetTime(reset, rem)}`;
  }

  return [
    { text: BOX.v, color: providerColor },
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
