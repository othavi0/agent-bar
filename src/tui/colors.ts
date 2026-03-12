import { ANSI } from "../theme";

export const oneDark = {
  green: ANSI.green,
  yellow: ANSI.yellow,
  orange: ANSI.orange,
  red: ANSI.red,
  cyan: ANSI.cyan,
  blue: ANSI.blue,
  magenta: ANSI.magenta,
  text: ANSI.text,
  textBright: ANSI.textBright,
  comment: ANSI.comment,
  muted: ANSI.muted,
  borderSoft: ANSI.borderSoft,
  borderStrong: ANSI.borderStrong,
  brightBlue: ANSI.brightBlue,
  brightMagenta: ANSI.brightMagenta,
  reset: ANSI.reset,
  bold: ANSI.bold,
  dim: ANSI.dim,
} as const;

// Semantic colors for quota display
export const semantic = {
  good: oneDark.green,
  warning: oneDark.yellow,
  danger: oneDark.orange,
  critical: oneDark.red,
  muted: oneDark.comment,
  accent: oneDark.blue,
  title: oneDark.textBright,
  subtitle: oneDark.muted,
  highlight: oneDark.cyan,
};

export function getQuotaColor(percent: number | null): string {
  if (percent === null) return semantic.muted;
  if (percent >= 60) return semantic.good;
  if (percent >= 30) return semantic.warning;
  if (percent >= 10) return semantic.danger;
  return semantic.critical;
}

export function colorize(text: string, color: string, isBold: boolean = false): string {
  const boldPrefix = isBold ? oneDark.bold : '';
  return `${boldPrefix}${color}${text}${oneDark.reset}`;
}

export function bold(text: string): string {
  return `${oneDark.bold}${text}${oneDark.reset}`;
}

export function dim(text: string): string {
  return `${oneDark.dim}${text}${oneDark.reset}`;
}
