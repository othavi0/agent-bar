import { getCodexExtra } from '../../providers/extras';
import type { ProviderQuota } from '../../providers/types';
import { BOX } from '../../theme';
import { barSegments, colorForDisplay, indicatorSegments, type Line } from '../segments';
import { formatPercent, toDisplay } from '../shared';
import type { CodexViewModel } from '../view-model';
import { buildFooterLine, labelLine, modelLine, raw } from './shared';
import type { BuildOptions } from './types';

// ---------------------------------------------------------------------------
// Segment helpers — Codex provider color is 'green'
// ---------------------------------------------------------------------------

/** Vertical bar line (Codex green border). */
const vLine = (): Line => [{ text: BOX.v, color: 'green' }];

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
      lines.push(labelLine('Available Models', labelColor, 'green'));
      lines.push([{ text: BOX.v, color: 'green' }, raw('  '), { text: 'No models selected', color: 'comment' }]);
    } else {
      const modelLen = Math.max(...models.map((m) => m.name.length), maxLen);

      if (policy !== 'seven_day') {
        lines.push(vLine());
        lines.push(labelLine('5-hour limit', labelColor, 'green'));
        for (const model of models) {
          lines.push(modelLine(model.name, model.windows.fiveHour, modelLen, mode, 'green', 'N/A'));
        }
      }

      if (policy !== 'five_hour') {
        lines.push(vLine());
        lines.push(labelLine('7-day limit', labelColor, 'green'));
        for (const model of models) {
          lines.push(modelLine(model.name, model.windows.sevenDay, modelLen, mode, 'green', 'N/A'));
        }
      }
    }

    const codexExtra = getCodexExtra(p);
    if (codexExtra?.extraUsage?.enabled) {
      const { remaining, limit } = codexExtra.extraUsage;
      const disp = toDisplay(remaining, mode);
      lines.push(vLine());
      lines.push(labelLine('Credits', labelColor, 'green'));
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
  lines.push(buildFooterLine(footer, 'green'));

  return lines;
}
