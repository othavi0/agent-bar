import * as p from '@clack/prompts';
import { ensureAmpCli, findAmpBin } from '../amp-cli';
import { ensureCommand } from '../install';
import { loadSettings, saveSettings } from '../settings';
import { colorize, semantic } from './colors';

async function runInteractive(cmd: string, args: string[] = []): Promise<number> {
  const proc = Bun.spawn([cmd, ...args], {
    stdin: 'inherit',
    stdout: 'inherit',
    stderr: 'inherit',
  });
  return await proc.exited;
}

async function ensureClaudeCli(): Promise<boolean> {
  return ensureCommand('claude', 'Install Claude Code CLI first (binary: claude).');
}

async function ensureCodexCli(): Promise<boolean> {
  return ensureCommand('codex', 'Install OpenAI Codex CLI first (binary: codex).');
}

async function ensureCopilotCli(): Promise<boolean> {
  return ensureCommand('copilot', 'Install GitHub Copilot CLI first (binary: copilot).');
}

async function activateProvider(providerId: string): Promise<void> {
  const settings = await loadSettings();

  if (settings.waybar.providers.includes(providerId)) {
    return;
  }

  const providerName = providerId.charAt(0).toUpperCase() + providerId.slice(1);
  const enable = await p.confirm({
    message: colorize(`Enable ${providerName} in Waybar?`, semantic.title),
    initialValue: true,
  });

  if (p.isCancel(enable) || !enable) return;

  settings.waybar.providers.push(providerId);
  await saveSettings(settings);

  try {
    const { applyWaybarIntegration } = await import('../waybar-integration');
    applyWaybarIntegration();
    Bun.spawn(['pkill', '-SIGUSR2', 'waybar'], { stdout: 'ignore', stderr: 'ignore' });
  } catch {
    /* best effort */
  }
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
        ['1) Run /login inside the Claude CLI', '2) Finish the browser/auth steps'].join('\n'),
        colorize('Claude Login', semantic.title),
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
      p.note(`Will run ${colorize('codex auth login', semantic.accent)}`, colorize('Codex Login', semantic.title));

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

    case 'copilot': {
      p.note(`Will run ${colorize('copilot login', semantic.accent)}`, colorize('Copilot Login', semantic.title));

      const ok = await ensureCopilotCli();
      if (!ok) {
        await waitEnter();
        return;
      }

      const code = await runInteractive('copilot', ['login']);
      if (code === 0) {
        await activateProvider('copilot');
      }
      await waitEnter();
      return;
    }

    case 'amp': {
      p.note('Will open Amp login in browser.', colorize('Amp Login', semantic.title));

      let ampBin = findAmpBin();
      if (!ampBin) {
        const ok = await ensureAmpCli();
        if (!ok) {
          await waitEnter();
          return;
        }
        ampBin = findAmpBin();
      }

      if (!ampBin) {
        p.log.error(colorize('Amp CLI is still unavailable after install.', semantic.danger));
        await waitEnter();
        return;
      }

      const code = await runInteractive(ampBin, ['login']);
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
