/** Respect the NO_COLOR convention (https://no-color.org/) */
const NO_COLOR = !!process.env.NO_COLOR;

const ansiFromHex = (hex: string): string => {
  if (NO_COLOR) return "";
  const clean = hex.replace("#", "");
  const r = Number.parseInt(clean.slice(0, 2), 16);
  const g = Number.parseInt(clean.slice(2, 4), 16);
  const b = Number.parseInt(clean.slice(4, 6), 16);
  return `\x1b[38;2;${r};${g};${b}m`;
};

export const ONE_DARK = {
  background: "#1f2329",
  surface: "#161a20",
  overlay: "#242a33",
  selection: "#2d3541",
  selectionAlt: "#323b48",
  text: "#c0c9d4",
  textBright: "#e2e8f0",
  comment: "#6a7485",
  muted: "#97a1ae",
  borderSoft: "#434d5d",
  borderStrong: "#3c4656",
  red: "#e06c75",
  green: "#98c379",
  yellow: "#e5c07b",
  blue: "#61afef",
  magenta: "#c678dd",
  cyan: "#56b6c2",
  orange: "#d19a66",
  brightBlue: "#528bff",
  brightMagenta: "#d070ff",
} as const;

export const PROVIDER_HEX = {
  claude: ONE_DARK.orange,
  codex: ONE_DARK.green,
  amp: ONE_DARK.magenta,
} as const;

export const ANSI = {
  reset: NO_COLOR ? "" : "\x1b[0m",
  bold: NO_COLOR ? "" : "\x1b[1m",
  dim: NO_COLOR ? "" : "\x1b[2m",
  green: ansiFromHex(ONE_DARK.green),
  yellow: ansiFromHex(ONE_DARK.yellow),
  orange: ansiFromHex(ONE_DARK.orange),
  red: ansiFromHex(ONE_DARK.red),
  blue: ansiFromHex(ONE_DARK.blue),
  cyan: ansiFromHex(ONE_DARK.cyan),
  magenta: ansiFromHex(ONE_DARK.magenta),
  text: ansiFromHex(ONE_DARK.text),
  textBright: ansiFromHex(ONE_DARK.textBright),
  comment: ansiFromHex(ONE_DARK.comment),
  muted: ansiFromHex(ONE_DARK.muted),
  borderSoft: ansiFromHex(ONE_DARK.borderSoft),
  borderStrong: ansiFromHex(ONE_DARK.borderStrong),
  brightBlue: ansiFromHex(ONE_DARK.brightBlue),
  brightMagenta: ansiFromHex(ONE_DARK.brightMagenta),
} as const;

export const PROVIDER_ANSI = {
  claude: ANSI.orange,
  codex: ANSI.green,
  amp: ANSI.magenta,
} as const;

/** Box-drawing characters (bold variant) — single source of truth */
export const BOX = {
  tl: "┏",
  bl: "┗",
  lt: "┣",
  h: "━",
  v: "┃",
  dot: "●",
  dotO: "○",
  diamond: "◆",
} as const;

