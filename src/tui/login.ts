import * as p from '@clack/prompts';
import { providers } from '../providers';
import { semantic, colorize } from './colors';

async function runInteractive(cmd: string, args: string[] = []): Promise<number> {
  const proc = Bun.spawn([cmd, ...args], {
    stdin: 'inherit',
    stdout: 'inherit',
    stderr: 'inherit',
  });
  return await proc.exited;
}

export async function loginProviderFlow(): Promise<void> {
  console.log('');
  console.log(colorize('  Tip: This does NOT modify Waybar config — it only helps you log in at the provider CLI.', semantic.subtitle));
  console.log(colorize('  Tip: Press q to go back.', semantic.subtitle));
  console.log('');

  const options = await Promise.all(
    providers.map(async (prov) => {
      const available = await prov.isAvailable();
      return {
        value: prov.id,
        label: available ? prov.name : `${prov.name} (not logged in)`,
        hint: available ? 'already logged in' : 'run login flow',
      };
    })
  );

  const choice = await p.select({
    message: 'Choose provider to log in',
    options: [
      ...options,
      { value: 'back' as const, label: 'Back' },
    ],
  });

  if (p.isCancel(choice) || choice === 'back') return;

  // Provider-specific flows
  switch (choice) {
    case 'claude': {
      p.log.info('Claude login: open the Claude CLI, then type /login inside it.');
      await p.text({ message: 'Press Enter to launch `claude`...', validate: () => undefined });
      await runInteractive('claude');
      break;
    }

    case 'codex': {
      p.log.info('Codex login: will run `codex auth login` (OAuth flow).');
      await p.text({ message: 'Press Enter to launch `codex auth login`...', validate: () => undefined });
      await runInteractive('codex', ['auth', 'login']);
      break;
    }

    case 'antigravity': {
      p.log.info('Antigravity is extension-managed (Codeium).');
      p.log.info('If quotas are not detected, ensure the Codeium language server is running.');
      await p.text({ message: 'Press Enter to go back...', validate: () => undefined });
      break;
    }

    default: {
      p.log.warn(`No login flow implemented for: ${String(choice)}`);
      await p.text({ message: 'Press Enter to go back...', validate: () => undefined });
      break;
    }
  }

  // Refresh Waybar after login attempt (best effort)
  try {
    await runInteractive('pkill', ['-USR2', 'waybar']);
  } catch {
    // ignore
  }
}
