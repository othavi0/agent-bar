import * as p from '@clack/prompts';
import { providers } from '../providers';
import { catppuccin, semantic, colorize } from './colors';
import { runAntigravityOAuth } from '../auth/antigravity-oauth';
import { addAntigravityAccount } from '../auth/storage';

async function runInteractive(cmd: string, args: string[] = []): Promise<number> {
  const proc = Bun.spawn([cmd, ...args], {
    stdin: 'inherit',
    stdout: 'inherit',
    stderr: 'inherit',
  });
  return await proc.exited;
}

async function openBrowser(url: string): Promise<void> {
  // Try common browser openers
  const openers = ['xdg-open', 'open', 'start'];
  
  for (const opener of openers) {
    try {
      const proc = Bun.spawn([opener, url], {
        stdout: 'ignore',
        stderr: 'ignore',
      });
      const code = await proc.exited;
      if (code === 0) return;
    } catch {
      continue;
    }
  }
  
  throw new Error('Could not open browser');
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
          'After login, you\'ll be redirected back',
          'and the token will be saved locally.',
        ].join('\n'),
        colorize('Antigravity Login', semantic.title)
      );
      
      const cont = await p.confirm({
        message: 'Open browser for Google OAuth?',
        initialValue: true,
      });
      
      if (p.isCancel(cont) || !cont) return;

      const spinner = p.spinner();
      spinner.start('Waiting for OAuth callback...');

      try {
        const result = await runAntigravityOAuth(openBrowser);
        
        // Save to qbar's own storage
        await addAntigravityAccount({
          email: result.email,
          name: result.name,
          accessToken: result.accessToken,
          refreshToken: result.refreshToken,
          expiresAt: result.expiresAt,
        });

        spinner.stop(colorize(`Logged in as ${result.email}`, semantic.good));
      } catch (error) {
        spinner.stop(colorize(`OAuth failed: ${error instanceof Error ? error.message : 'Unknown error'}`, semantic.danger));
      }
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
