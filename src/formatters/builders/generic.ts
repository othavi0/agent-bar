import type { ProviderQuota } from '../../providers/types';
import { BOX } from '../../theme';
import { barSegments, colorForDisplay, indicatorSegments, type Line } from '../segments';
import { formatPercent, toDisplay } from '../shared';
import { buildFooterLine, raw } from './shared';
import type { BuildOptions } from './types';

/**
 * Pure builder for the generic provider card.
 *
 * Used as the fallback when a provider has no dedicated builder.
 * Emits Line[] with no I/O, no markup — only composes Segments using shared primitives.
 *
 * Header width is intentionally narrower than dedicated builders (headerWidth=52 → 56
 * total visual chars) to match the original generic fallback layout.
 */
export function buildGeneric(p: ProviderQuota, options: BuildOptions): Line[] {
  const { mode, headerTitle, headerWidth, footer } = options;
  const headerFill = Math.max(1, headerWidth - headerTitle.length);
  const lines: Line[] = [];

  // Header
  lines.push([
    { text: BOX.tl + BOX.h, color: 'text' },
    raw(' '),
    { text: headerTitle, color: 'text', bold: true },
    raw(' '),
    { text: BOX.h.repeat(headerFill), color: 'text' },
  ]);

  if (p.error) {
    lines.push([{ text: BOX.v, color: 'text' }, raw('  '), { text: p.error, color: 'red' }]);
  } else if (p.primary) {
    const rem = p.primary.remaining;
    const disp = toDisplay(rem, mode);
    lines.push([
      { text: BOX.v, color: 'text' },
      raw('  '),
      ...indicatorSegments(disp, mode),
      raw(' '),
      ...barSegments(disp, mode),
      raw(' '),
      { text: formatPercent(disp).padStart(4), color: colorForDisplay(disp, mode) },
    ]);
  }

  lines.push(buildFooterLine(footer, 'text'));

  return lines;
}
