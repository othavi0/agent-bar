import * as p from '@clack/prompts';
import { colorize, semantic } from './tui/colors';

/**
 * Omarchy-only installer helpers.
 * We use yay so we can pull Omarchy/AUR packages (latest).
 */

export async function hasCmd(cmd: string): Promise<boolean> {
  if (typeof Bun.which === 'function') {
    return Bun.which(cmd) !== null;
  }

  try {
    const proc = Bun.spawn(['which', cmd], { stdout: 'ignore', stderr: 'ignore' });
    return await proc.exited === 0;
  } catch {
    return false;
  }
}

async function runInteractive(cmd: string, args: string[] = []): Promise<number> {
  const proc = Bun.spawn([cmd, ...args], {
    stdin: 'inherit',
    stdout: 'inherit',
    stderr: 'inherit',
  });
  return await proc.exited;
}

export async function ensureYay(): Promise<boolean> {
  if (await hasCmd('yay')) return true;
  p.log.error(colorize('yay not found. This project is Omarchy-only; install yay first.', semantic.danger));
  return false;
}

export async function ensureYayPackage(pkg: string, label?: string): Promise<boolean> {
  const ok = await ensureYay();
  if (!ok) return false;

  const spinner = p.spinner();
  spinner.start(`Installing ${label ?? pkg}...`);

  try {
    // --needed: skip if already installed
    // --noconfirm: non-interactive (Omarchy usage)
    const code = await runInteractive('yay', ['-S', '--needed', '--noconfirm', pkg]);
    if (code === 0) {
      spinner.stop(colorize(`${label ?? pkg} ready`, semantic.good));
      return true;
    }

    spinner.stop(colorize(`Failed to install ${label ?? pkg}`, semantic.danger));
    return false;
  } catch {
    spinner.stop(colorize(`Failed to install ${label ?? pkg}`, semantic.danger));
    return false;
  }
}

export async function ensureBun(): Promise<boolean> {
  if (await hasCmd('bun')) return true;

  // On Omarchy, bun is commonly available, but we still try to install.
  // Package name may vary; bun is in official repos in many setups.
  return await ensureYayPackage('bun', 'Bun');
}

export async function ensureBunGlobalPackage(pkg: string, label?: string): Promise<boolean> {
  const ok = await ensureBun();
  if (!ok) return false;

  // If command exists already, skip. (Best-effort: pkg name usually matches bin)
  const bin = pkg;
  if (await hasCmd(bin)) return true;

  const spinner = p.spinner();
  spinner.start(`Installing ${label ?? pkg}...`);

  try {
    const code = await runInteractive('bun', ['add', '-g', pkg]);
    if (code === 0) {
      spinner.stop(colorize(`${label ?? pkg} ready`, semantic.good));
      return true;
    }

    spinner.stop(colorize(`Failed to install ${label ?? pkg}`, semantic.danger));
    return false;
  } catch {
    spinner.stop(colorize(`Failed to install ${label ?? pkg}`, semantic.danger));
    return false;
  }
}
