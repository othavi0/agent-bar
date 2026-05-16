import { existsSync } from 'node:fs';
import { join } from 'node:path';
import * as p from '@clack/prompts';
import { colorize, semantic } from './tui/colors';

export async function hasCmd(cmd: string): Promise<boolean> {
  if (typeof Bun.which === 'function') {
    if (Bun.which(cmd) !== null) return true;
  }

  const home = process.env.HOME ?? '';

  const bunGlobalPaths = [join(home, '.cache', '.bun', 'bin', cmd), join(home, '.bun', 'bin', cmd)];

  for (const p of bunGlobalPaths) {
    if (existsSync(p)) return true;
  }

  try {
    const proc = Bun.spawn(['which', cmd], { stdout: 'ignore', stderr: 'ignore' });
    return (await proc.exited) === 0;
  } catch {
    return false;
  }
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

export async function ensureBunGlobalPackage(pkg: string, label?: string, binName?: string): Promise<boolean> {
  const bin = binName ?? pkg;
  if (await hasCmd(bin)) {
    return true;
  }

  const ok = await ensureBun();
  if (!ok) return false;

  const spinner = p.spinner();
  spinner.start(`Installing ${label ?? pkg}...`);

  try {
    const proc = Bun.spawn(['bun', 'add', '-g', pkg], {
      stdout: 'pipe',
      stderr: 'pipe',
    });
    const stdout = await new Response(proc.stdout).text();
    const stderr = await new Response(proc.stderr).text();
    const code = await proc.exited;

    if (code === 0 && (await hasCmd(bin))) {
      spinner.stop(`${label ?? pkg} ready`);
      return true;
    }

    const diagnostic = (stderr || stdout).trim();
    if (diagnostic) {
      p.log.error(diagnostic);
    }
    spinner.error(`Failed to install ${label ?? pkg}`);
    return false;
  } catch (err) {
    p.log.error(String(err));
    spinner.error(`Failed to install ${label ?? pkg}`);
    return false;
  }
}
