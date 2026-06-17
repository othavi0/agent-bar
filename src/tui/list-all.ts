import * as p from '@clack/prompts';
import { buildAmp as buildAmpLines } from '../formatters/builders/amp';
import { buildClaude } from '../formatters/builders/claude';
import { buildCodex as buildCodexLines } from '../formatters/builders/codex';
import { normalizePlanLabel } from '../formatters/shared';
import { resolveCodexViewModel } from '../formatters/view-model';
import { getAllQuotas } from '../providers';
import type { ProviderQuota } from '../providers/types';
import { colorize, semantic } from './colors';
import { renderColorize } from './render-colorize';

function buildClaudeTui(provider: ProviderQuota): string[] {
  const rendered = renderColorize(
    buildClaude(provider, {
      mode: 'remaining',
      headerTitle: 'Claude',
      headerWidth: 56,
      labelColor: 'blue',
      footer: undefined,
    }),
  );
  return rendered.split('\n');
}

function buildCodexTui(provider: ProviderQuota): string[] {
  const viewModel = resolveCodexViewModel(provider);
  const planLabel = normalizePlanLabel(provider);
  const rendered = renderColorize(
    buildCodexLines(provider, viewModel, {
      mode: 'remaining',
      headerTitle: 'Codex',
      headerWidth: 56,
      labelColor: 'blue',
      planLabel,
      footer: undefined,
    }),
  );
  return rendered.split('\n');
}

function buildAmpTui(provider: ProviderQuota): string[] {
  const rendered = renderColorize(
    buildAmpLines(provider, {
      mode: 'remaining',
      headerTitle: 'Amp',
      headerWidth: 56,
      labelColor: 'blue',
      ampFreeTierLayout: 'generic',
      footer: undefined,
    }),
  );
  return rendered.split('\n');
}

export async function showListAll(): Promise<void> {
  const s = p.spinner();
  s.start('Loading quotas...');

  const quotas = await getAllQuotas();

  s.stop('Quotas loaded');

  // Build output
  const sections: string[][] = [];

  for (const provider of quotas.providers) {
    if (!provider.available && !provider.error) continue;

    switch (provider.provider) {
      case 'claude':
        sections.push(buildClaudeTui(provider));
        break;
      case 'codex':
        sections.push(buildCodexTui(provider));
        break;
      case 'amp':
        sections.push(buildAmpTui(provider));
        break;
    }
  }

  // Print
  console.log('');
  for (const section of sections) {
    for (const line of section) {
      console.log(line);
    }
    console.log('');
  }

  console.log(colorize('Press Enter to continue...', semantic.subtitle));

  // Wait for enter — always restore raw mode even if an error occurs
  process.stdin.setRawMode?.(true);
  process.stdin.resume();
  try {
    await new Promise<void>((resolve) => {
      process.stdin.once('data', () => resolve());
    });
  } finally {
    process.stdin.setRawMode?.(false);
    process.stdin.pause();
  }
}
