import { existsSync } from 'node:fs';
import { join } from 'node:path';
import * as p from '@clack/prompts';
import { APP_NAME } from './app-identity';
import { ensureCommand } from './install';
import { colorize, semantic } from './tui/colors';

export const AMP_INSTALL_COMMAND = 'curl -fsSL https://ampcode.com/install.sh | bash';
export const AMP_MISSING_ERROR = 'Amp CLI not installed. Right-click to install and log in.';

interface FindAmpBinOptions {
  home?: string;
  exists?: (path: string) => boolean;
  which?: (cmd: string) => string | null | undefined;
}

function runInteractiveShell(command: string): Promise<number> {
  const proc = Bun.spawn(['bash', '-lc', command], {
    stdin: 'inherit',
    stdout: 'inherit',
    stderr: 'inherit',
  });
  return proc.exited;
}

export function getAmpCandidatePaths(home = process.env.HOME ?? ''): string[] {
  if (!home) {
    return [];
  }

  return [
    join(home, '.local', 'bin', 'amp'),
    join(home, '.amp', 'bin', 'amp'),
    join(home, '.cache', '.bun', 'bin', 'amp'),
    join(home, '.bun', 'bin', 'amp'),
  ];
}

export function findAmpBin(options: FindAmpBinOptions = {}): string | null {
  const which = options.which ?? (typeof Bun.which === 'function' ? Bun.which.bind(Bun) : undefined);
  const foundFromPath = which?.('amp');
  if (foundFromPath) {
    return foundFromPath;
  }

  const exists = options.exists ?? existsSync;
  const paths = getAmpCandidatePaths(options.home);
  if (paths.length === 0) {
    paths.push(...getAmpCandidatePaths());
  }

  for (const path of paths) {
    if (exists(path)) {
      return path;
    }
  }

  return null;
}

export async function ensureAmpCli(): Promise<boolean> {
  if (findAmpBin()) {
    return true;
  }

  p.note(
    [
      `Amp CLI is required before ${APP_NAME} can fetch usage or open Amp login.`,
      '',
      `Installer: ${colorize(AMP_INSTALL_COMMAND, semantic.accent)}`,
    ].join('\n'),
    colorize('Amp Install', semantic.title),
  );

  const confirm = await p.confirm({
    message: 'Install Amp CLI now?',
    initialValue: true,
  });

  if (p.isCancel(confirm) || !confirm) {
    return false;
  }

  const hasCurl = await ensureCommand('curl', 'Install curl first (required for the Amp installer).');
  if (!hasCurl) {
    return false;
  }

  p.log.info(colorize('Running the official Amp installer...', semantic.subtitle));

  try {
    const code = await runInteractiveShell(AMP_INSTALL_COMMAND);
    if (code !== 0) {
      p.log.error(colorize('Failed to install Amp CLI.', semantic.danger));
      return false;
    }
  } catch {
    p.log.error(colorize('Failed to install Amp CLI.', semantic.danger));
    return false;
  }

  const ampBin = findAmpBin();
  if (!ampBin) {
    p.log.error(
      colorize(
        `Amp installer finished, but ${APP_NAME} could not find the amp binary. Check that ~/.local/bin is available and try again.`,
        semantic.danger,
      ),
    );
    return false;
  }

  p.log.success(colorize(`Amp ready: ${ampBin}`, semantic.good));
  return true;
}
