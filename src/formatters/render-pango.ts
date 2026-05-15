import { ONE_DARK } from '../theme';
import type { ColorToken, Line, Segment } from './segments';

const HEX_BY_TOKEN: Record<ColorToken, string> = {
  green: ONE_DARK.green,
  yellow: ONE_DARK.yellow,
  orange: ONE_DARK.orange,
  red: ONE_DARK.red,
  comment: ONE_DARK.comment,
  text: ONE_DARK.text,
  textBright: ONE_DARK.textBright,
  muted: ONE_DARK.muted,
  magenta: ONE_DARK.magenta,
  cyan: ONE_DARK.cyan,
  blue: ONE_DARK.blue,
  brightBlue: ONE_DARK.brightBlue,
};

function escapeXml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/'/g, '&#39;')
    .replace(/"/g, '&quot;');
}

function span(color: string, text: string, bold = false): string {
  return `<span foreground='${color}'${bold ? " weight='bold'" : ''}>${escapeXml(text)}</span>`;
}

function renderSegment(seg: Segment): string {
  if (seg.raw) return seg.text;
  return span(HEX_BY_TOKEN[seg.color], seg.text, seg.bold ?? false);
}

function renderLine(line: Line): string {
  return line.map(renderSegment).join('');
}

/** Render a list of Lines to a multi-line Pango markup string. */
export function renderPango(lines: Line[]): string {
  return lines.map(renderLine).join('\n');
}
