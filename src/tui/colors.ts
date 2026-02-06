/**
 * Catppuccin Mocha color palette
 * https://catppuccin.com/palette
 */
export const catppuccin = {
  // Accent colors
  rosewater: '\x1b[38;2;245;224;220m',
  flamingo: '\x1b[38;2;242;205;205m',
  pink: '\x1b[38;2;245;194;231m',
  mauve: '\x1b[38;2;203;166;247m',
  red: '\x1b[38;2;243;139;168m',
  maroon: '\x1b[38;2;235;160;172m',
  peach: '\x1b[38;2;250;179;135m',
  yellow: '\x1b[38;2;249;226;175m',
  green: '\x1b[38;2;166;227;161m',
  teal: '\x1b[38;2;148;226;213m',
  sky: '\x1b[38;2;137;220;235m',
  sapphire: '\x1b[38;2;116;199;236m',
  blue: '\x1b[38;2;137;180;250m',
  lavender: '\x1b[38;2;180;190;254m',
  
  // Text colors
  text: '\x1b[38;2;205;214;244m',
  subtext1: '\x1b[38;2;186;194;222m',
  subtext0: '\x1b[38;2;166;173;200m',
  
  // Overlay colors
  overlay2: '\x1b[38;2;147;153;178m',
  overlay1: '\x1b[38;2;127;132;156m',
  overlay0: '\x1b[38;2;108;112;134m',
  
  // Surface colors
  surface2: '\x1b[38;2;88;91;112m',
  surface1: '\x1b[38;2;69;71;90m',
  surface0: '\x1b[38;2;49;50;68m',
  
  // Base colors
  base: '\x1b[38;2;30;30;46m',
  mantle: '\x1b[38;2;24;24;37m',
  crust: '\x1b[38;2;17;17;27m',
  
  // Formatting
  reset: '\x1b[0m',
  bold: '\x1b[1m',
  dim: '\x1b[2m',
} as const;

// Semantic colors for quota display
export const semantic = {
  good: catppuccin.green,      // >= 60%
  warning: catppuccin.yellow,   // >= 30%
  danger: catppuccin.peach,     // >= 10%
  critical: catppuccin.red,     // < 10%
  muted: catppuccin.overlay0,
  accent: catppuccin.mauve,
  title: catppuccin.lavender,
  subtitle: catppuccin.subtext0,
};

export function getQuotaColor(percent: number | null): string {
  if (percent === null) return semantic.muted;
  if (percent >= 60) return semantic.good;
  if (percent >= 30) return semantic.warning;
  if (percent >= 10) return semantic.danger;
  return semantic.critical;
}

export function colorize(text: string, color: string): string {
  return `${color}${text}${catppuccin.reset}`;
}

export function bold(text: string): string {
  return `${catppuccin.bold}${text}${catppuccin.reset}`;
}

export function dim(text: string): string {
  return `${catppuccin.dim}${text}${catppuccin.reset}`;
}
