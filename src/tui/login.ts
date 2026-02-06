import * as p from '@clack/prompts';
import { providers } from '../providers';
import { catppuccin, semantic, colorize } from './colors';

async function runInteractive(cmd: string, args: string[] = []): Promise<number> {
  const proc = Bun.spawn([cmd, ...args], {
    stdin: 'inherit',
    stdout: 'inherit',
    stderr: 'inherit',
  });
  return await proc.exited;
}

async function commandExists(cmd: string): Promise<boolean> {
  try {
    // Prefer Bun.which (doesn't depend on shell/which availability)
    if (typeof Bun.which === 'function') {
      return Bun.which(cmd) !== null;
    }

    const proc = Bun.spawn(['which', cmd], { stdout: 'ignore', stderr: 'ignore' });
    return await proc.exited === 0;
  } catch {
    return false;
  }
}

function resolveAntigravityUsagePath(): string {
  // In Waybar/uwsm environments, PATH can miss Bun's global bin.
  // Try Bun.which first, then fallback to Bun's default global bin location.
  const found = typeof Bun.which === 'function' ? Bun.which('antigravity-usage') : null;
  if (found) return found;

  const home = process.env.HOME ?? '';
  return `${home}/.cache/.bun/bin/antigravity-usage`;
}

async function ensureAntigravityUsage(): Promise<boolean> {
  // Check if already installed
  if (await commandExists('antigravity-usage')) {
    return true;
  }

  // Install silently with bun
  const spinner = p.spinner();
  spinner.start('Installing dependencies...');
  
  try {
    const proc = Bun.spawn(['bun', 'add', '-g', 'antigravity-usage'], {
      stdout: 'ignore',
      stderr: 'ignore',
    });
    const code = await proc.exited;
    
    if (code === 0) {
      spinner.stop(colorize('Dependencies ready', semantic.good));
      return true;
    } else {
      spinner.stop(colorize('Failed to install dependencies', semantic.danger));
      return false;
    }
  } catch (error) {
    spinner.stop(colorize('Failed to install dependencies', semantic.danger));
    return false;
  }
}

export async function loginProviderFlow(): Promise<void> {
  // Box with tips (OpenClaw-style)
  p.note(
    [
      'This helps you log in to provider CLIs.',
      '',
      colorize('Space', semantic.highlight) + ' to select  ' + colorize('Enter', semantic.highlight) + ' to confirm  ' + colorize('q', semantic.highlight) + ' to go back',
    ].join('\n'),
    colorize('Provider Login', semantic.title)
  );

  const options = await Promise.all(
    providers.map(async (prov) => {
      const available = await prov.isAvailable();
      return {
        value: prov.id,
        label: available 
          ? colorize(prov.name, catppuccin.green) 
          : colorize(`${prov.name}`, catppuccin.text) + colorize(' (not logged in)', semantic.muted),
        hint: available ? 'already logged in' : 'run login flow',
      };
    })
  );

  const choice = await p.select({
    message: colorize('Choose provider', semantic.title),
    options: [
      ...options,
      { value: 'back' as const, label: colorize('Back', semantic.muted) },
    ],
  });

  if (p.isCancel(choice) || choice === 'back') return;

  // Provider-specific flows
  switch (choice) {
    case 'claude': {
      p.note(
        [
          '1. Confirm the folder (trust prompt)',
          '2. Type ' + colorize('/login', semantic.accent),
          '3. Choose your login method',
        ].join('\n'),
        colorize('Claude Login Steps', semantic.title)
      );
      
      const cont = await p.confirm({
        message: 'Launch Claude CLI?',
        initialValue: true,
      });
      
      if (p.isCancel(cont) || !cont) return;
      
      await runInteractive('claude');
      break;
    }

    case 'codex': {
      p.note(
        'Will run ' + colorize('codex auth login', semantic.accent) + ' (OAuth flow)',
        colorize('Codex Login', semantic.title)
      );
      
      const cont = await p.confirm({
        message: 'Launch Codex auth?',
        initialValue: true,
      });
      
      if (p.isCancel(cont) || !cont) return;
      
      await runInteractive('codex', ['auth', 'login']);
      break;
    }

    case 'antigravity': {
      p.note(
        [
          'Will open browser for Google OAuth.',
          '',
          'If the CLI returns immediately after opening the browser,',
          'qbar will keep this terminal open and wait for tokens to appear.',
        ].join('\n'),
        colorize('Antigravity Login', semantic.title)
      );
      
      const cont = await p.confirm({
        message: 'Open browser for Google OAuth?',
        initialValue: true,
      });
      
      if (p.isCancel(cont) || !cont) return;

      // Ensure antigravity-usage is installed
      const installed = await ensureAntigravityUsage();
      if (!installed) {
        p.log.error('Could not setup Antigravity login. Please try again.');
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

      // Run the login flow (may open browser and exit quickly)
      const antigravityCmd = resolveAntigravityUsagePath();
      await runInteractive(antigravityCmd, ['login']);

      // Wait for tokens to appear/update so the terminal doesn't close too early.
      p.log.info(colorize('Waiting for OAuth completion in your browser...', semantic.subtitle));
      const timeoutMs = 3 * 60_000;
      const start = Date.now();

      let ok = false;
      while (Date.now() - start < timeoutMs) {
        const now = latestTokenMtimeMs();
        if (now > before) {
          ok = true;
          p.log.success(colorize('Antigravity tokens detected. Login complete.', semantic.good));
          break;
        }
        await Bun.sleep(500);
      }

      if (!ok) {
        p.log.warn(colorize('Timed out waiting for tokens. If you finished login in the browser, try again.', semantic.warning));
        p.log.warn(colorize(`Looking in: ${accountsDir}`, semantic.muted));
      }

      // Keep terminal open so the user can read what happened
      p.log.info(colorize('Press Enter to continue...', semantic.subtitle));
      await new Promise<void>((resolve) => {
        process.stdin.setRawMode?.(true);
        process.stdin.resume();
        process.stdin.once('data', () => {
          process.stdin.setRawMode?.(false);
          resolve();
        });
      });

      break;
    }

    default: {
      p.log.warn(`No login flow for: ${String(choice)}`);
      break;
    }
  }

  // Refresh Waybar after login attempt (best effort)
  try {
    Bun.spawn(['pkill', '-USR2', 'waybar']);
  } catch {
    // ignore
  }
}
