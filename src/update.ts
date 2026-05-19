#!/usr/bin/env bun

/**
 * agent-bar update - Update the managed ~/.agent-bar installation.
 */

import { existsSync } from 'node:fs';
import { homedir } from 'node:os';
import { join, resolve } from 'node:path';
import * as p from '@clack/prompts';
import { colorize, semantic } from './tui/colors';
import { printCommandHeader, printKeyValues, printWarning } from './tui/terminal-ui';

const REPO_ROOT = join(import.meta.dir, '..');
const DEPENDENCY_FILES = ['package.json', 'bun.lock', 'bun.lockb'];

export interface CommandResult {
  ok: boolean;
  output: string;
}

export type CommandRunner = (cmd: string, args: string[], cwd: string) => Promise<CommandResult>;

export interface UpdateSummary {
  repoRoot: string;
  installRoot: string;
  currentCommit: string;
  currentBranch: string;
  upstream: string;
  commits: string[];
  localChanges: string[];
  hasUpdates: boolean;
  hasLocalChanges: boolean;
  dependencyFilesChanged: boolean;
  needsDependencyInstall: boolean;
}

export type ManagedUpdateStatus = 'wrong-root' | 'up-to-date' | 'cancelled' | 'updated';

export type InstallKind = 'managed-git' | 'dev-git' | 'npm';

export interface ManagedUpdateResult {
  status: ManagedUpdateStatus;
  repoRoot: string;
  installRoot: string;
  summary?: UpdateSummary;
  installedDependencies: boolean;
}

export interface NpmUpdateSummary {
  packageName: string;
  currentVersion: string;
}

export type NpmUpdateStatus = 'cancelled' | 'updated';

export interface NpmUpdateResult {
  status: NpmUpdateStatus;
  summary: NpmUpdateSummary;
}

export interface NpmUpdateOptions {
  repoRoot?: string;
  runCommand?: CommandRunner;
  runSetup: () => Promise<void>;
  confirmNpm: (summary: NpmUpdateSummary) => Promise<boolean>;
  onEvent?: (event: UpdateEvent) => void;
}

export type UpdateEvent =
  | { type: 'step'; message: string }
  | { type: 'info'; message: string }
  | { type: 'success'; message: string };

export interface ManagedUpdateOptions {
  repoRoot?: string;
  installRoot?: string;
  runCommand?: CommandRunner;
  runSetup: () => Promise<void>;
  confirm: (summary: UpdateSummary) => Promise<boolean>;
  onEvent?: (event: UpdateEvent) => void;
}

class UpdateCommandError extends Error {
  constructor(
    readonly step: string,
    readonly output: string,
  ) {
    super(`${step} failed${output.trim() ? `: ${output.trim()}` : ''}`);
  }
}

export async function runCmd(cmd: string, args: string[], cwd: string): Promise<CommandResult> {
  try {
    const proc = Bun.spawn([cmd, ...args], {
      cwd,
      stdout: 'pipe',
      stderr: 'pipe',
    });

    const stdout = await new Response(proc.stdout).text();
    const stderr = await new Response(proc.stderr).text();
    const code = await proc.exited;

    return { ok: code === 0, output: stdout + stderr };
  } catch (error) {
    return { ok: false, output: String(error) };
  }
}

export function isManagedInstallRoot(repoRoot: string, installRoot: string = join(homedir(), '.agent-bar')): boolean {
  return resolve(repoRoot) === resolve(installRoot);
}

export function detectInstallKind(repoRoot: string, installRoot: string = join(homedir(), '.agent-bar')): InstallKind {
  if (!existsSync(join(repoRoot, '.git'))) {
    return 'npm';
  }
  return isManagedInstallRoot(repoRoot, installRoot) ? 'managed-git' : 'dev-git';
}

async function requireCommand(
  runCommand: CommandRunner,
  cwd: string,
  step: string,
  cmd: string,
  args: string[],
): Promise<string> {
  const result = await runCommand(cmd, args, cwd);
  if (!result.ok) {
    throw new UpdateCommandError(step, result.output);
  }
  return result.output.trim();
}

async function resolveUpstream(runCommand: CommandRunner, repoRoot: string): Promise<string> {
  const configured = await runCommand('git', ['rev-parse', '--abbrev-ref', '--symbolic-full-name', '@{u}'], repoRoot);
  if (configured.ok && configured.output.trim()) {
    return configured.output.trim();
  }

  return requireCommand(runCommand, repoRoot, 'Resolve origin/master', 'git', [
    'rev-parse',
    '--verify',
    'origin/master',
  ]);
}

function splitLines(output: string): string[] {
  return output
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean);
}

export async function runManagedUpdate(options: ManagedUpdateOptions): Promise<ManagedUpdateResult> {
  const repoRoot = options.repoRoot ?? REPO_ROOT;
  const installRoot = options.installRoot ?? join(homedir(), '.agent-bar');
  const runCommand = options.runCommand ?? runCmd;
  const onEvent = options.onEvent ?? (() => {});

  if (!isManagedInstallRoot(repoRoot, installRoot)) {
    return { status: 'wrong-root', repoRoot, installRoot, installedDependencies: false };
  }

  onEvent({ type: 'step', message: 'Checking repository...' });
  await requireCommand(runCommand, repoRoot, 'Check git repository', 'git', ['rev-parse', '--git-dir']);
  const currentCommit = await requireCommand(runCommand, repoRoot, 'Read current commit', 'git', [
    'rev-parse',
    '--short',
    'HEAD',
  ]);
  const currentBranch = await requireCommand(runCommand, repoRoot, 'Read current branch', 'git', [
    'branch',
    '--show-current',
  ]);

  onEvent({ type: 'step', message: 'Fetching upstream...' });
  await requireCommand(runCommand, repoRoot, 'Fetch origin', 'git', ['fetch', '--prune', 'origin']);
  const upstream = await resolveUpstream(runCommand, repoRoot);

  onEvent({ type: 'step', message: 'Inspecting changes...' });
  const commits = splitLines(
    await requireCommand(runCommand, repoRoot, 'List incoming commits', 'git', [
      'log',
      '--oneline',
      `HEAD..${upstream}`,
      '-10',
    ]),
  );
  const localChanges = splitLines(
    await requireCommand(runCommand, repoRoot, 'Read local changes', 'git', ['status', '--short']),
  );
  const dependencyFilesChanged = Boolean(
    await requireCommand(runCommand, repoRoot, 'Check dependency changes', 'git', [
      'diff',
      '--name-only',
      'HEAD',
      upstream,
      '--',
      ...DEPENDENCY_FILES,
    ]),
  );
  const needsDependencyInstall = dependencyFilesChanged || !existsSync(join(repoRoot, 'node_modules'));

  const summary: UpdateSummary = {
    repoRoot,
    installRoot,
    currentCommit,
    currentBranch,
    upstream,
    commits,
    localChanges,
    hasUpdates: commits.length > 0,
    hasLocalChanges: localChanges.length > 0,
    dependencyFilesChanged,
    needsDependencyInstall,
  };

  if (!summary.hasUpdates && !summary.hasLocalChanges) {
    return { status: 'up-to-date', repoRoot, installRoot, summary, installedDependencies: false };
  }

  const approved = await options.confirm(summary);
  if (!approved) {
    return { status: 'cancelled', repoRoot, installRoot, summary, installedDependencies: false };
  }

  onEvent({ type: 'step', message: 'Discarding local install changes...' });
  await requireCommand(runCommand, repoRoot, 'Reset install checkout', 'git', ['reset', '--hard', upstream]);
  await requireCommand(runCommand, repoRoot, 'Clean install checkout', 'git', ['clean', '-fd']);

  let installedDependencies = false;
  if (needsDependencyInstall) {
    onEvent({ type: 'step', message: 'Installing dependencies...' });
    await requireCommand(runCommand, repoRoot, 'Install dependencies', 'bun', ['install']);
    installedDependencies = true;
  } else {
    onEvent({ type: 'info', message: 'Dependencies unchanged; skipping bun install.' });
  }

  onEvent({ type: 'step', message: 'Re-applying Waybar integration...' });
  await options.runSetup();
  onEvent({ type: 'success', message: 'Managed install updated.' });

  return { status: 'updated', repoRoot, installRoot, summary, installedDependencies };
}

async function readPackageInfo(repoRoot: string): Promise<NpmUpdateSummary> {
  const pkg = (await Bun.file(join(repoRoot, 'package.json')).json()) as {
    name?: string;
    version?: string;
  };
  if (!pkg.name || !pkg.version) {
    throw new Error('package.json is missing name or version');
  }
  return { packageName: pkg.name, currentVersion: pkg.version };
}

export async function runNpmUpdate(options: NpmUpdateOptions): Promise<NpmUpdateResult> {
  const repoRoot = options.repoRoot ?? REPO_ROOT;
  const runCommand = options.runCommand ?? runCmd;
  const onEvent = options.onEvent ?? (() => {});

  onEvent({ type: 'step', message: 'Reading package info...' });
  const summary = await readPackageInfo(repoRoot);

  const approved = await options.confirmNpm(summary);
  if (!approved) {
    return { status: 'cancelled', summary };
  }

  onEvent({ type: 'step', message: 'Updating package with Bun...' });
  await requireCommand(runCommand, repoRoot, 'Update package', 'bun', ['add', '-g', summary.packageName]);

  onEvent({ type: 'step', message: 'Re-applying Waybar integration...' });
  await options.runSetup();
  onEvent({ type: 'success', message: 'Package updated.' });

  return { status: 'updated', summary };
}

function printSummary(summary: UpdateSummary): void {
  printKeyValues('Install', [
    ['Path', summary.repoRoot],
    ['Branch', summary.currentBranch || '(detached)'],
    ['Current', summary.currentCommit],
    ['Upstream', summary.upstream],
  ]);

  if (summary.commits.length > 0) {
    p.note(
      summary.commits.map((line) => colorize(line, semantic.subtitle)).join('\n'),
      colorize('Incoming', semantic.title),
    );
  }

  if (summary.localChanges.length > 0) {
    p.note(
      summary.localChanges.map((line) => colorize(line, semantic.warning)).join('\n'),
      colorize('Local changes to discard', semantic.warning),
    );
  }
}

async function runNpmUpdateInteractive(): Promise<void> {
  try {
    const result = await runNpmUpdate({
      runSetup: async () => {
        const { runSetup } = await import('./setup');
        await runSetup({ confirm: false, clearScreen: false });
      },
      confirmNpm: async (summary) => {
        printKeyValues('Package', [
          ['Name', summary.packageName],
          ['Installed', summary.currentVersion],
        ]);
        printWarning('npm update', [`This runs \`bun add -g ${summary.packageName}\` and re-applies setup.`]);

        const proceed = await p.confirm({
          message: 'Update the package with Bun and re-apply setup?',
          initialValue: true,
        });

        return !p.isCancel(proceed) && proceed;
      },
    });

    if (result.status === 'cancelled') {
      p.outro(colorize('Update cancelled', semantic.muted));
      return;
    }

    p.outro(colorize('Package updated. Restart Waybar if modules look stale.', semantic.good));
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    p.log.error(colorize(message, semantic.danger));
    p.outro(colorize('Update failed', semantic.danger));
    process.exit(1);
  }
}

export async function main() {
  console.clear();
  printCommandHeader('update', 'Updater for agent-bar');

  const installKind = detectInstallKind(REPO_ROOT);

  if (installKind === 'dev-git') {
    p.log.error(colorize('This is a development checkout, not a managed install.', semantic.danger));
    p.log.info(colorize('Update it with git directly, e.g. `git pull`.', semantic.subtitle));
    p.outro(colorize('Update aborted', semantic.muted));
    return;
  }

  if (installKind === 'npm') {
    await runNpmUpdateInteractive();
    return;
  }

  const spinner = p.spinner();
  let spinnerActive = false;

  const stopSpinner = (message: string) => {
    if (spinnerActive) {
      spinner.stop(message);
      spinnerActive = false;
    }
  };

  try {
    const result = await runManagedUpdate({
      onEvent: (event) => {
        if (event.type === 'step') {
          stopSpinner(event.message);
          spinner.start(event.message);
          spinnerActive = true;
        } else if (event.type === 'info') {
          stopSpinner(event.message);
          p.log.info(colorize(event.message, semantic.muted));
        } else if (event.type === 'success') {
          stopSpinner(event.message);
        }
      },
      runSetup: async () => {
        const { runSetup } = await import('./setup');
        await runSetup({ confirm: false, clearScreen: false });
      },
      confirm: async (summary) => {
        stopSpinner('Checks complete');
        printSummary(summary);
        printWarning('Managed update', [
          `This will discard local changes in ${summary.installRoot}.`,
          'Use a separate checkout for development work.',
        ]);

        const proceed = await p.confirm({
          message: 'Reset the managed install, pull upstream, and re-apply setup?',
          initialValue: true,
        });

        return !p.isCancel(proceed) && proceed;
      },
    });

    stopSpinner('Update checks complete');

    if (result.status === 'wrong-root') {
      p.log.error(colorize(`Managed updates only run from ${result.installRoot}`, semantic.danger));
      p.log.info(colorize(`Current checkout: ${result.repoRoot}`, semantic.subtitle));
      p.outro(colorize('Update aborted', semantic.muted));
      return;
    }

    if (result.status === 'up-to-date') {
      p.outro(colorize('Already up to date', semantic.good));
      return;
    }

    if (result.status === 'cancelled') {
      p.outro(colorize('Update cancelled', semantic.muted));
      return;
    }

    const suffix = result.installedDependencies ? ' Dependencies updated.' : ' Dependencies unchanged.';
    p.outro(colorize(`Update complete.${suffix}`, semantic.good));
  } catch (error) {
    stopSpinner('Update failed');
    const message = error instanceof Error ? error.message : String(error);
    p.log.error(colorize(message, semantic.danger));
    p.outro(colorize('Update failed', semantic.danger));
    process.exit(1);
  }
}

if (import.meta.main) {
  main().catch((error) => {
    console.error('Update failed:', error);
    process.exit(1);
  });
}
