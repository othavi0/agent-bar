import * as p from '@clack/prompts';
import { colorize, semantic, catppuccin } from './colors';
import { ensureBunGlobalPackage, ensureYayPackage, hasCmd } from '../install';

async function runInteractive(cmd: string, args: string[] = []): Promise<number> {
  const proc = Bun.spawn([cmd, ...args], {
    stdin: 'inherit',
    stdout: 'inherit',
    stderr: 'inherit',
  });
  return await proc.exited;
}

function resolveAntigravityUsagePath(): string {
  const found = typeof Bun.which === 'function' ? Bun.which('antigravity-usage') : null;
  if (found) return found;
  const home = process.env.HOME ?? '';
  return `${home}/.cache/.bun/bin/antigravity-usage`;
}

async function ensureAntigravityUsage(): Promise<boolean> {
  return await ensureBunGlobalPackage('antigravity-usage', 'antigravity-usage');
}

async function ensureClaudeCli(): Promise<boolean> {
  return await ensureYayPackage('aur/claude-code', 'aur/claude-code', 'claude');
}

async function ensureCodexCli(): Promise<boolean> {
  return await ensureYayPackage('aur/openai-codex-bin', 'aur/openai-codex-bin', 'codex');
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
  // Quick sanity
  if (!await hasCmd('yay')) {
    p.log.error(colorize('yay not found. This flow is Omarchy-only.', semantic.danger));
    await waitEnter();
    return;
  }

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

      await runInteractive('claude');
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

      await runInteractive('codex', ['auth', 'login']);
      await waitEnter();
      return;
    }

    case 'antigravity': {
      p.note(
        [
          'Will open browser for Google OAuth.',
          'If the CLI returns quickly, qbar will wait for tokens.',
        ].join('\n'),
        colorize('Antigravity Login', semantic.title)
      );

      const ok = await ensureAntigravityUsage();
      if (!ok) {
        await waitEnter();
        return;
      }

      const { readdirSync, statSync } = await import('node:fs');
      const { join } = await import('node:path');

      const accountsDir = join(process.env.HOME ?? '', '.config', 'antigravity-usage', 'accounts');

      const latestTokenMtimeMs = (): number => {
        try {
          const accounts = readdirSync(accountsDir, { withFileTypes: true })
            .filter((d) => d.isDirectory())
            .map((d) => d.name);

          let latest = 0;
          for (const acc of accounts) {
            const tokenPath = join(accountsDir, acc, 'tokens.json');
            try {
              const st = statSync(tokenPath);
              latest = Math.max(latest, st.mtimeMs);
            } catch {
              // ignore
            }
          }
          return latest;
        } catch {
          return 0;
        }
      };

      const before = latestTokenMtimeMs();

      const antigravityCmd = resolveAntigravityUsagePath();
      await runInteractive(antigravityCmd, ['login']);

      p.log.info(colorize('Waiting for OAuth completion in your browser...', semantic.subtitle));
      const timeoutMs = 3 * 60_000;
      const start = Date.now();

      let okTokens = false;
      while (Date.now() - start < timeoutMs) {
        const now = latestTokenMtimeMs();
        if (now > before) {
          okTokens = true;
          p.log.success(colorize('Antigravity tokens detected. Login complete.', semantic.good));
          break;
        }
        await Bun.sleep(500);
      }

      if (!okTokens) {
        p.log.warn(colorize('Timed out waiting for tokens. If you finished login in the browser, try again.', semantic.warning));
        p.log.warn(colorize(`Looking in: ${accountsDir}`, semantic.muted));
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
