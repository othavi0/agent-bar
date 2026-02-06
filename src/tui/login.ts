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

export async function loginProviderFlow(): Promise<void> {
  // Box with tips (OpenClaw-style)
  p.note(
    [
      'This does NOT modify Waybar config.',
      'It only helps you log in at the provider CLI.',
      '',
      colorize('Space', semantic.accent) + ' to select  ' + colorize('Enter', semantic.accent) + ' to confirm  ' + colorize('q', semantic.accent) + ' to go back',
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
          'Antigravity uses Google OAuth via OpenClaw.',
          '',
          'Will run: ' + colorize('openclaw models auth login google-antigravity', semantic.accent),
        ].join('\n'),
        colorize('Antigravity Login', semantic.title)
      );
      
      const cont = await p.confirm({
        message: 'Launch OpenClaw auth?',
        initialValue: true,
      });
      
      if (p.isCancel(cont) || !cont) return;
      
      await runInteractive('openclaw', ['models', 'auth', 'login', 'google-antigravity']);
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
