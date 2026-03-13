import * as p from '@clack/prompts';
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { createSpinner } from './spinner';
import { colorize, semantic } from './tui/colors';

export async function hasCmd(cmd: string): Promise<boolean> {
  if (typeof Bun.which === 'function') {
    if (Bun.which(cmd) !== null) return true;
  }

  const home = process.env.HOME ?? '';

  const bunGlobalPaths = [
    join(home, '.cache', '.bun', 'bin', cmd),
    join(home, '.bun', 'bin', cmd),
  ];

  for (const p of bunGlobalPaths) {
    if (existsSync(p)) return true;
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

export async function ensureCommand(cmd: string, installHint: string): Promise<boolean> {
  if (await hasCmd(cmd)) {
    return true;
  }

  p.log.warn(colorize(`${cmd} not found. ${installHint}`, semantic.warning));
  return false;
}

export async function ensureBun(): Promise<boolean> {
  return ensureCommand('bun', 'Install Bun first: https://bun.sh');
}

export async function ensureBunGlobalPackage(
  pkg: string,
  label?: string,
  binName?: string,
): Promise<boolean> {
  const bin = binName ?? pkg;
  if (await hasCmd(bin)) {
    return true;
  }

  const ok = await ensureBun();
  if (!ok) return false;

  const spinner = createSpinner(`Installing ${label ?? pkg}...`);
  spinner.start();

  try {
    const code = await runInteractive('bun', ['add', '-g', pkg]);
    if (code === 0 && (await hasCmd(bin))) {
      spinner.succeed(`${label ?? pkg} ready`);
      return true;
    }

    spinner.fail(`Failed to install ${label ?? pkg}`);
    return false;
  } catch {
    spinner.fail(`Failed to install ${label ?? pkg}`);
    return false;
  }
}
