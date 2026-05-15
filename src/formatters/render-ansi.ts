import { ANSI } from '../theme';
import type { ColorToken, Line, Segment } from './segments';

const ANSI_BY_TOKEN: Record<ColorToken, string> = {
  green: ANSI.green,
  yellow: ANSI.yellow,
  orange: ANSI.orange,
  red: ANSI.red,
  comment: ANSI.comment,
  text: ANSI.text,
  textBright: ANSI.textBright,
  muted: ANSI.muted,
  magenta: ANSI.magenta,
  cyan: ANSI.cyan,
  blue: ANSI.blue,
  brightBlue: ANSI.brightBlue,
};

function renderSegment(seg: Segment): string {
  if (seg.raw) return seg.text;
  return `${ANSI_BY_TOKEN[seg.color]}${seg.bold ? ANSI.bold : ''}${seg.text}`;
}

function renderLine(line: Line): string {
  if (line.length === 0) return '';
  const body = line.map(renderSegment).join('');
  // Append reset only if at least one non-raw segment is present
  const hasColored = line.some((s) => !s.raw);
  return hasColored ? `${body}${ANSI.reset}` : body;
}

/** Render a list of Lines to a multi-line ANSI string. */
export function renderAnsi(lines: Line[]): string {
  return lines.map(renderLine).join('\n');
}
