import { getStatusForPercent, type HealthStatus } from '../config';
import { type DisplayMode, toHealth } from './shared';

/** Theme-neutral color token. Renderers map this to ANSI or Pango hex. */
export type ColorToken =
  | 'green'
  | 'yellow'
  | 'orange'
  | 'red'
  | 'comment'
  | 'text'
  | 'textBright'
  | 'muted'
  | 'magenta'
  | 'cyan'
  | 'blue'
  | 'brightBlue';

export interface Segment {
  text: string;
  color: ColorToken;
  bold?: boolean;
}

const STATUS_TO_COLOR: Record<HealthStatus, ColorToken> = {
  ok: 'green',
  low: 'yellow',
  warn: 'orange',
  critical: 'red',
};

export function colorForDisplay(display: number | null, mode: DisplayMode): ColorToken {
  const health = toHealth(display, mode);
  if (health === null) return 'text';
  return STATUS_TO_COLOR[getStatusForPercent(health)];
}

/** Build 20-wide quota bar segments. Empty when value is null. */
export function barSegments(display: number | null, mode: DisplayMode): Segment[] {
  if (display === null) return [{ text: '░'.repeat(20), color: 'comment' }];
  const filled = Math.floor(display / 5);
  return [
    { text: '█'.repeat(filled), color: colorForDisplay(display, mode) },
    { text: '░'.repeat(20 - filled), color: 'comment' },
  ];
}

/** Build single-dot indicator segments. Open dot when value is null. */
export function indicatorSegments(display: number | null, mode: DisplayMode): Segment[] {
  if (display === null) return [{ text: '○', color: 'comment' }];
  return [{ text: '●', color: colorForDisplay(display, mode) }];
}

/** A single rendered line: an ordered list of colored text segments. */
export type Line = Segment[];
