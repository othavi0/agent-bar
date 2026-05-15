import type { AllQuotas, ProviderQuota } from '../providers/types';
import { ANSI, BOX } from '../theme';
import { buildAmp as buildAmpLines } from './builders/amp';
import { buildClaude } from './builders/claude';
import { buildCodex as buildCodexLines } from './builders/codex';
import { buildCopilot as buildCopilotLines } from './builders/copilot';
import { renderAnsi } from './render-ansi';
import { type ColorToken, colorForDisplay } from './segments';
import { type DisplayMode, formatPercent, normalizePlanLabel, toDisplay } from './shared';
import { resolveCodexViewModel } from './view-model';

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

function getColor(display: number | null, mode: DisplayMode): string {
  return ANSI_BY_TOKEN[colorForDisplay(display, mode)];
}

function buildClaudeTerminal(p: ProviderQuota, mode: DisplayMode): string[] {
  const rendered = renderAnsi(
    buildClaude(p, {
      mode,
      headerTitle: 'Claude',
      headerWidth: 56,
      labelColor: 'magenta',
      footer: undefined,
    }),
  );
  return rendered.split('\n');
}

function buildCodexTerminal(p: ProviderQuota, mode: DisplayMode): string[] {
  const viewModel = resolveCodexViewModel(p);
  const planLabel = normalizePlanLabel(p);
  const rendered = renderAnsi(
    buildCodexLines(p, viewModel, {
      mode,
      headerTitle: 'Codex',
      headerWidth: 56,
      labelColor: 'magenta',
      planLabel,
      footer: undefined,
    }),
  );
  return rendered.split('\n');
}

function buildAmpTerminal(p: ProviderQuota, mode: DisplayMode): string[] {
  const rendered = renderAnsi(
    buildAmpLines(p, {
      mode,
      headerTitle: 'Amp',
      headerWidth: 56,
      labelColor: 'magenta',
      ampFreeTierLayout: 'sublines',
      footer: undefined,
    }),
  );
  return rendered.split('\n');
}

function buildCopilotTerminal(p: ProviderQuota, mode: DisplayMode): string[] {
  const rendered = renderAnsi(
    buildCopilotLines(p, {
      mode,
      headerTitle: 'Copilot',
      headerWidth: 56,
      labelColor: 'magenta',
      footer: undefined,
    }),
  );
  return rendered.split('\n');
}

// ---------------------------------------------------------------------------
// Terminal builder registry
// ---------------------------------------------------------------------------

type TerminalBuilder = (p: ProviderQuota, mode: DisplayMode) => string[];

const TERMINAL_BUILDERS: Record<string, TerminalBuilder> = {
  claude: buildClaudeTerminal,
  codex: buildCodexTerminal,
  copilot: buildCopilotTerminal,
  amp: buildAmpTerminal,
};

function buildGenericTerminal(p: ProviderQuota, mode: DisplayMode): string[] {
  const vc = ANSI.text;
  const vi = (c: string) => `${c}${BOX.v}${ANSI.reset}`;
  const lines: string[] = [];
  const name = p.displayName ?? p.provider;

  lines.push(
    `${vc}${BOX.tl}${BOX.h}${ANSI.reset} ${vc}${name}${ANSI.reset} ${vc}${BOX.h.repeat(Math.max(1, 55 - name.length - 3))}${ANSI.reset}`,
  );

  if (p.error) {
    lines.push(`${vi(vc)}  ${ANSI.red}${p.error}${ANSI.reset}`);
  } else if (p.primary) {
    const rem = p.primary.remaining;
    const disp = toDisplay(rem, mode);
    const color = getColor(disp, mode);
    const suffix = mode === 'used' ? 'used' : 'remaining';
    lines.push(`${vi(vc)}  ${color}${formatPercent(disp)} ${suffix}${ANSI.reset}`);
  }

  lines.push(`${vc}${BOX.bl}${BOX.h.repeat(55)}${ANSI.reset}`);
  return lines;
}

export function formatForTerminal(quotas: AllQuotas, mode: DisplayMode = 'remaining'): string {
  const sections: string[][] = [];

  for (const p of quotas.providers) {
    if (!p.available && !p.error) continue;
    const builder = TERMINAL_BUILDERS[p.provider];
    sections.push(builder ? builder(p, mode) : buildGenericTerminal(p, mode));
  }

  if (sections.length === 0) {
    return `${ANSI.comment}No providers connected${ANSI.reset}`;
  }

  return sections.map((s) => s.join('\n')).join('\n\n');
}

export function outputTerminal(quotas: AllQuotas, mode: DisplayMode = 'remaining'): void {
  console.log(formatForTerminal(quotas, mode));
}
