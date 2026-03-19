import { existsSync } from 'node:fs';
import * as p from '@clack/prompts';
import { colorize, semantic } from './colors';
import { ensureBunGlobalPackage, ensureCommand } from '../install';
import { loadSettings, saveSettings } from '../settings';

async function runInteractive(cmd: string, args: string[] = []): Promise<number> {
  const proc = Bun.spawn([cmd, ...args], {
    stdin: 'inherit',
    stdout: 'inherit',
    stderr: 'inherit',
  });
  return await proc.exited;
}

function findAmpBin(): string | null {
  if (typeof Bun.which === 'function') {
    const found = Bun.which('amp');
    if (found) return found;
  }

  const home = process.env.HOME ?? '';
  const paths = [
    `${home}/.local/bin/amp`,
    `${home}/.amp/bin/amp`,
    `${home}/.cache/.bun/bin/amp`,
    `${home}/.bun/bin/amp`,
  ];

  for (const p of paths) {
    if (existsSync(p)) return p;
  }

  return null;
}

async function ensureClaudeCli(): Promise<boolean> {
  return ensureCommand('claude', 'Install Claude Code CLI first (binary: claude).');
}

async function ensureCodexCli(): Promise<boolean> {
  return ensureCommand('codex', 'Install OpenAI Codex CLI first (binary: codex).');
}

async function ensureAmpCli(): Promise<boolean> {
  return ensureBunGlobalPackage('@anthropic-ai/amp', 'amp', 'amp');
}

async function activateProvider(providerId: string): Promise<void> {
  const settings = await loadSettings();

  if (!settings.waybar.providers.includes(providerId)) {
    settings.waybar.providers.push(providerId);
  }
  await saveSettings(settings);
}

async function waitEnter(): Promise<void> {
  const { createInterface } = await import('node:readline');
  p.log.info(colorize('Press Enter to continue...', semantic.subtitle));
  return new Promise<void>((resolve) => {
    const rl = createInterface({ input: process.stdin });
    rl.once('line', () => {
      rl.close();
      resolve();
    });
  });
}

export async function loginSingleProvider(providerId: string): Promise<void> {
  switch (providerId) {
    case 'claude': {
      p.note(
        [
          '1) Run /login inside the Claude CLI',
          '2) Finish the browser/auth steps',
        ].join('\n'),
        colorize('Claude Login', semantic.title)
      );

      const ok = await ensureClaudeCli();
      if (!ok) {
        await waitEnter();
        return;
      }

      const code = await runInteractive('claude');
      if (code === 0) {
        await activateProvider('claude');
      }
      await waitEnter();
      return;
    }

    case 'codex': {
      p.note(
        'Will run ' + colorize('codex auth login', semantic.accent),
        colorize('Codex Login', semantic.title)
      );

      const ok = await ensureCodexCli();
      if (!ok) {
        await waitEnter();
        return;
      }

      const code = await runInteractive('codex', ['auth', 'login']);
      if (code === 0) {
        await activateProvider('codex');
      }
      await waitEnter();
      return;
    }

    case 'amp': {
      p.note(
        'Will open Amp login in browser.',
        colorize('Amp Login', semantic.title)
      );

      const ampBin = findAmpBin();
      if (!ampBin) {
        const ok = await ensureAmpCli();
        if (!ok) {
          await waitEnter();
          return;
        }
      }

      const code = await runInteractive(ampBin || 'amp', ['login']);
      if (code === 0) {
        await activateProvider('amp');
      }
      await waitEnter();
      return;
    }

    default: {
      p.log.error(colorize(`Unknown provider: ${providerId}`, semantic.danger));
      await waitEnter();
      return;
    }
  }
}
