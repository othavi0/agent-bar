import type { ColorToken, Line, Segment } from '../formatters/segments';
import { colorize, oneDark } from './colors';

const COLOR_BY_TOKEN: Record<ColorToken, string> = {
  green: oneDark.green,
  yellow: oneDark.yellow,
  orange: oneDark.orange,
  red: oneDark.red,
  comment: oneDark.comment,
  text: oneDark.text,
  textBright: oneDark.textBright,
  muted: oneDark.muted,
  magenta: oneDark.magenta,
  cyan: oneDark.cyan,
  blue: oneDark.blue,
  brightBlue: oneDark.brightBlue,
};

function renderSegment(seg: Segment): string {
  if (seg.raw) return seg.text;
  return colorize(seg.text, COLOR_BY_TOKEN[seg.color], seg.bold ?? false);
}

function renderLine(line: Line): string {
  return line.map(renderSegment).join('');
}

/** Render a list of Lines to a multi-line colorized string (TUI). */
export function renderColorize(lines: Line[]): string {
  return lines.map(renderLine).join('\n');
}
