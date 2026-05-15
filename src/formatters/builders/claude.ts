import { getClaudeExtra } from '../../providers/extras';
import type { ProviderQuota } from '../../providers/types';
import { BOX } from '../../theme';
import { barSegments, colorForDisplay, indicatorSegments, type Line } from '../segments';
import { formatPercent, toDisplay } from '../shared';
import { buildFooterLine, labelLine, modelLine, raw, vLine } from './shared';
import type { BuildOptions } from './types';

// ---------------------------------------------------------------------------
// Segment helpers — Claude provider color is 'orange'
// ---------------------------------------------------------------------------

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
  lines.push(vLine('orange'));

  if (p.error) {
    lines.push([{ text: BOX.v, color: 'orange' }, raw('  '), { text: `⚠️ ${p.error}`, color: 'red' }]);
  } else {
    const maxLen = 20;

    if (p.primary) {
      lines.push(labelLine('5-hour limit (shared)', labelColor, 'orange'));
      lines.push(modelLine('All Models', p.primary, maxLen, mode, 'orange'));
    }

    // Per-model weekly quotas (when API provides them)
    const claudeExtra = getClaudeExtra(p);
    const weeklyModels = claudeExtra?.weeklyModels;
    if (weeklyModels && Object.keys(weeklyModels).length > 0) {
      lines.push(vLine('orange'));
      lines.push(labelLine('Weekly per model', labelColor, 'orange'));
      const entries = Object.entries(weeklyModels);
      const wMaxLen = Math.max(...entries.map(([name]) => name.length), maxLen);
      for (const [name, window] of entries) {
        lines.push(modelLine(name, window, wMaxLen, mode, 'orange'));
      }
    }

    // Generic weekly (shared)
    if (p.secondary) {
      lines.push(vLine('orange'));
      lines.push(labelLine('Weekly limit (shared)', labelColor, 'orange'));
      lines.push(modelLine('All Models', p.secondary, maxLen, mode, 'orange'));
    }

    if (claudeExtra?.extraUsage?.enabled && claudeExtra.extraUsage.limit > 0) {
      const { remaining, used, limit } = claudeExtra.extraUsage;
      const disp = toDisplay(remaining, mode);
      lines.push(vLine('orange'));
      lines.push(labelLine('Extra Usage', labelColor, 'orange'));
      lines.push(
        extraUsageLine('Budget', maxLen, disp, mode, `$${(used / 100).toFixed(2)}/$${(limit / 100).toFixed(2)}`),
      );
    }
  }

  lines.push(vLine('orange'));
  lines.push(buildFooterLine(footer, 'orange'));

  return lines;
}
