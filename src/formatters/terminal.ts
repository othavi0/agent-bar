import type { AllQuotas, ProviderQuota } from '../providers/types';
import { ANSI } from '../theme';
import { buildAmp as buildAmpLines } from './builders/amp';
import { buildClaude } from './builders/claude';
import { buildCodex as buildCodexLines } from './builders/codex';
import { buildGeneric } from './builders/generic';
import { renderAnsi } from './render-ansi';
import { type DisplayMode, normalizePlanLabel } from './shared';
import { resolveCodexViewModel } from './view-model';

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

// ---------------------------------------------------------------------------
// Terminal builder registry
// ---------------------------------------------------------------------------

type TerminalBuilder = (p: ProviderQuota, mode: DisplayMode) => string[];

const TERMINAL_BUILDERS: Record<string, TerminalBuilder> = {
  claude: buildClaudeTerminal,
  codex: buildCodexTerminal,
  amp: buildAmpTerminal,
};

export function formatForTerminal(quotas: AllQuotas, mode: DisplayMode = 'remaining'): string {
  const sections: string[][] = [];

  for (const p of quotas.providers) {
    if (!p.available && !p.error) continue;
    const builder = TERMINAL_BUILDERS[p.provider];
    if (builder) {
      sections.push(builder(p, mode));
    } else {
      const name = p.displayName ?? p.provider;
      sections.push(
        renderAnsi(
          buildGeneric(p, { mode, headerTitle: name, headerWidth: 52, labelColor: 'text', footer: undefined }),
        ).split('\n'),
      );
    }
  }

  if (sections.length === 0) {
    return `${ANSI.comment}No providers connected${ANSI.reset}`;
  }

  return sections.map((s) => s.join('\n')).join('\n\n');
}

export function outputTerminal(quotas: AllQuotas, mode: DisplayMode = 'remaining'): void {
  console.log(formatForTerminal(quotas, mode));
}
