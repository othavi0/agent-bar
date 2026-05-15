import { getAmpExtra } from '../../providers/extras';
import type { ProviderQuota } from '../../providers/types';
import { BOX } from '../../theme';
import { barSegments, type ColorToken, colorForDisplay, indicatorSegments, type Line } from '../segments';
import { etaLabel, formatEta, formatPercent, formatResetTime, toDisplay } from '../shared';
import { buildFooterLine, labelLine, raw, vLine } from './shared';
import type { BuildOptions } from './types';

// ---------------------------------------------------------------------------
// Per-surface `labelColor` (verified against the old builders):
//   Terminal: 'magenta'  — old `label()` helper hardcoded ANSI.magenta for the ◆
//   Waybar:   'magenta'  — old label used PROVIDER_HEX.amp (ONE_DARK.magenta)
//   TUI:      'blue'     — old `label()` helper hardcoded oneDark.blue for the ◆
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Internal line helpers
// ---------------------------------------------------------------------------

/**
 * Free Tier bar line shared between sublines and inline layouts.
 * Inline layout appends the ETA directly; sublines layout omits it.
 */
function freeTierBarLine(disp: number | null, mode: BuildOptions['mode'], etaSegments: Line): Line {
  return [
    { text: BOX.v, color: 'magenta' },
    raw('  '),
    ...indicatorSegments(disp, mode),
    raw(' '),
    ...barSegments(disp, mode),
    raw(' '),
    { text: formatPercent(disp).padStart(4), color: colorForDisplay(disp, mode) },
    ...etaSegments,
  ];
}

// ---------------------------------------------------------------------------
// Main builder
// ---------------------------------------------------------------------------

/**
 * Pure builder for the Amp provider card.
 *
 * Emits Line[] with no I/O, no settings reads, no markup.
 * Surface-specific differences are driven by `options`:
 *
 * ampFreeTierLayout:
 *   'sublines' — Terminal: bar line (no ETA) + tree-connector sub-lines (├─/└─)
 *                for dollar info and ETA.
 *   'inline'   — Waybar:  bar line with inline ETA + a dot-○ line for dollar info.
 *   'generic'  — TUI:     simplified generic model loop (no Free Tier special
 *                handling; iterates all models with name+bar+pct).
 *
 * labelColor (per surface):
 *   Terminal → 'magenta', Waybar → 'magenta', TUI → 'blue'
 */
export function buildAmp(p: ProviderQuota, options: BuildOptions): Line[] {
  const { mode, headerTitle, headerWidth, labelColor, footer, ampFreeTierLayout = 'inline', accountInHeader } = options;
  const headerFill = Math.max(1, headerWidth - headerTitle.length);
  const lines: Line[] = [];

  const m: Record<string, string> = getAmpExtra(p)?.meta ?? {};

  // Header
  lines.push([
    { text: BOX.tl + BOX.h, color: 'magenta' },
    raw(' '),
    { text: headerTitle, color: 'magenta', bold: true },
    raw(' '),
    { text: BOX.h.repeat(headerFill), color: 'magenta' },
  ]);
  lines.push(vLine('magenta'));

  if (p.error) {
    lines.push([{ text: BOX.v, color: 'magenta' }, raw('  '), { text: `⚠️ ${p.error}`, color: 'red' }]);
  } else if (ampFreeTierLayout === 'generic') {
    // -----------------------------------------------------------------------
    // TUI layout: generic model loop (no Free Tier special handling)
    // -----------------------------------------------------------------------
    if (!p.models || Object.keys(p.models).length === 0) {
      lines.push([{ text: BOX.v, color: 'magenta' }, raw('  '), { text: 'No usage data', color: 'muted' }]);
    } else {
      const entries = Object.entries(p.models);
      const maxLen = Math.max(...entries.map(([name]) => name.length), 20);

      lines.push(labelLine('Usage', labelColor, 'magenta'));
      for (const [name, window] of entries) {
        const disp = toDisplay(window.remaining, mode);
        lines.push([
          { text: BOX.v, color: 'magenta' },
          raw('  '),
          ...indicatorSegments(disp, mode),
          raw(' '),
          { text: name.padEnd(maxLen), color: 'textBright' },
          raw(' '),
          ...barSegments(disp, mode),
          raw(' '),
          { text: formatPercent(disp).padStart(4), color: colorForDisplay(disp, mode) },
        ]);
      }
    }
  } else {
    // -----------------------------------------------------------------------
    // Terminal ('sublines') and Waybar ('inline') layouts
    // -----------------------------------------------------------------------
    const free = p.models?.['Free Tier'];

    if (free) {
      const rem = free.remaining;
      const disp = toDisplay(rem, mode);

      lines.push(labelLine('Free Tier', labelColor, 'magenta'));

      if (ampFreeTierLayout === 'sublines') {
        // Terminal: bar line without ETA; sub-lines with tree connectors
        lines.push(freeTierBarLine(disp, mode, []));

        // Build sub-details
        const subs: Line[] = [];

        // Dollar info sub-line: replenishRate  ( freeRemaining / freeTotal )  bonus
        const dollarParts: Line = [];
        if (m.replenishRate) {
          dollarParts.push({ text: m.replenishRate, color: 'cyan' });
        }
        const dollars = [m.freeRemaining, m.freeTotal].filter(Boolean).join(' / ');
        if (dollars) {
          if (dollarParts.length > 0) dollarParts.push(raw('  '));
          dollarParts.push({ text: `( ${dollars} )`, color: 'text' });
        }
        if (m.bonus) {
          if (dollarParts.length > 0) dollarParts.push(raw('  '));
          dollarParts.push({ text: m.bonus, color: 'cyan' });
        }
        if (dollarParts.length > 0) subs.push(dollarParts);

        // ETA sub-line (only when resetsAt is present and not full)
        if (free.resetsAt && rem !== 100) {
          const etaText = `${etaLabel(mode)} ${formatEta(free.resetsAt, rem)}  ${formatResetTime(free.resetsAt, rem)}`;
          subs.push([{ text: etaText, color: 'cyan' }]);
        }

        for (let i = 0; i < subs.length; i++) {
          const conn = i === subs.length - 1 ? '└─' : '├─';
          lines.push([
            { text: BOX.v, color: 'magenta' },
            raw('  '),
            { text: conn, color: 'comment' },
            raw(' '),
            ...subs[i],
          ]);
        }
      } else {
        // Waybar inline: bar line with ETA appended; dollar info on ○ line
        const etaSegs: Line =
          free.resetsAt && rem !== 100
            ? [
                raw('  '),
                {
                  text: `→ ${etaLabel(mode)} ${formatEta(free.resetsAt, rem)} ${formatResetTime(free.resetsAt, rem)}`,
                  color: 'cyan',
                },
              ]
            : [];

        lines.push(freeTierBarLine(disp, mode, etaSegs));

        // Dollar info on ○ line
        const infoParts: Line = [];
        if (m.replenishRate) {
          infoParts.push({ text: m.replenishRate, color: 'cyan' });
        }
        const dollars = [m.freeRemaining, m.freeTotal].filter(Boolean).join(' / ');
        if (dollars) {
          if (infoParts.length > 0) infoParts.push({ text: '  |  ', color: 'comment' });
          infoParts.push({ text: dollars, color: 'text' });
        }
        if (m.bonus) {
          if (infoParts.length > 0) infoParts.push({ text: '  |  ', color: 'comment' });
          infoParts.push({ text: m.bonus, color: 'cyan' });
        }
        if (infoParts.length > 0) {
          lines.push([
            { text: BOX.v, color: 'magenta' },
            raw('  '),
            { text: BOX.dotO, color: 'comment' },
            raw(' '),
            ...infoParts,
          ]);
        }
      }
    }

    // Credits section (terminal + waybar; not in TUI generic layout)
    const credits = p.models?.Credits;
    if (credits) {
      lines.push(vLine('magenta'));
      const balance = m.creditsBalance ?? '$0';
      const creditColor: ColorToken = credits.remaining > 0 ? 'green' : 'comment';
      lines.push(labelLine('Credits', labelColor, 'magenta'));
      const creditDisp = toDisplay(credits.remaining, mode);
      lines.push([
        { text: BOX.v, color: 'magenta' },
        raw('  '),
        ...indicatorSegments(creditDisp, mode),
        raw(' '),
        {
          text: ampFreeTierLayout === 'inline' ? `${balance} remaining` : balance,
          color: creditColor,
        },
      ]);
    }

    // Fallback for unknown models (when neither Free Tier nor Credits)
    if (!free && !credits && p.models && Object.keys(p.models).length > 0) {
      const entries = Object.entries(p.models);
      const maxLen = Math.max(...entries.map(([name]) => name.length), 20);
      lines.push(labelLine('Usage', labelColor, 'magenta'));
      for (const [name, window] of entries) {
        const disp = toDisplay(window.remaining, mode);
        lines.push([
          { text: BOX.v, color: 'magenta' },
          raw('  '),
          ...indicatorSegments(disp, mode),
          raw(' '),
          { text: name.padEnd(maxLen), color: 'textBright' },
          raw(' '),
          ...barSegments(disp, mode),
          raw(' '),
          { text: formatPercent(disp).padStart(4), color: colorForDisplay(disp, mode) },
        ]);
      }
    }
  }

  // Account line — omitted when the surface already shows the account in the header
  if (p.account && !accountInHeader) {
    lines.push(vLine('magenta'));
    lines.push([{ text: BOX.v, color: 'magenta' }, raw('  '), { text: `Account: ${p.account}`, color: 'comment' }]);
  }

  lines.push(vLine('magenta'));
  lines.push(buildFooterLine(footer, 'magenta'));

  return lines;
}
