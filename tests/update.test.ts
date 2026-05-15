import { afterEach, describe, expect, it } from 'bun:test';
import { mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { type CommandRunner, runManagedUpdate } from '../src/update';

const tempDirs: string[] = [];

function tempInstallRoot(): string {
  const dir = mkdtempSync(join(tmpdir(), 'agent-bar-update-'));
  tempDirs.push(dir);
  return dir;
}

afterEach(() => {
  for (const dir of tempDirs.splice(0)) {
    rmSync(dir, { recursive: true, force: true });
  }
});

function commandKey(cmd: string, args: string[]): string {
  return `${cmd} ${args.join(' ')}`;
}

function fakeRunner(outputs: Record<string, string>): { commands: Array<[string, string[]]>; run: CommandRunner } {
  const commands: Array<[string, string[]]> = [];

  return {
    commands,
    run: async (cmd, args) => {
      commands.push([cmd, args]);
      const key = commandKey(cmd, args);
      return { ok: true, output: outputs[key] ?? '' };
    },
  };
}

describe('runManagedUpdate', () => {
  it('aborts before running commands outside the managed install root', async () => {
    const { commands, run } = fakeRunner({});

    const result = await runManagedUpdate({
      repoRoot: '/tmp/dev/agent-bar',
      installRoot: '/home/test/.agent-bar',
      runCommand: run,
      runSetup: async () => {},
      confirm: async () => true,
    });

    expect(result.status).toBe('wrong-root');
    expect(commands).toEqual([]);
  });

  it('discards local install changes, resets to upstream, installs changed deps, and runs setup', async () => {
    const installRoot = tempInstallRoot();
    let setupCount = 0;
    const { commands, run } = fakeRunner({
      'git rev-parse --git-dir': '.git\n',
      'git rev-parse --short HEAD': 'abc123\n',
      'git branch --show-current': 'master\n',
      'git rev-parse --abbrev-ref --symbolic-full-name @{u}': 'origin/master\n',
      'git log --oneline HEAD..origin/master -10': 'def456 update cli\n',
      'git status --short': ' M README.md\n?? scratch.txt\n',
      'git diff --name-only HEAD origin/master -- package.json bun.lock bun.lockb': 'package.json\n',
    });

    const result = await runManagedUpdate({
      repoRoot: installRoot,
      installRoot,
      runCommand: run,
      runSetup: async () => {
        setupCount += 1;
      },
      confirm: async (summary) => {
        expect(summary.hasLocalChanges).toBe(true);
        expect(summary.hasUpdates).toBe(true);
        expect(summary.dependencyFilesChanged).toBe(true);
        return true;
      },
    });

    expect(result.status).toBe('updated');
    expect(result.installedDependencies).toBe(true);
    expect(setupCount).toBe(1);
    expect(commands).toEqual([
      ['git', ['rev-parse', '--git-dir']],
      ['git', ['rev-parse', '--short', 'HEAD']],
      ['git', ['branch', '--show-current']],
      ['git', ['fetch', '--prune', 'origin']],
      ['git', ['rev-parse', '--abbrev-ref', '--symbolic-full-name', '@{u}']],
      ['git', ['log', '--oneline', 'HEAD..origin/master', '-10']],
      ['git', ['status', '--short']],
      ['git', ['diff', '--name-only', 'HEAD', 'origin/master', '--', 'package.json', 'bun.lock', 'bun.lockb']],
      ['git', ['reset', '--hard', 'origin/master']],
      ['git', ['clean', '-fd']],
      ['bun', ['install']],
    ]);
  });

  it('skips bun install when dependency files did not change and node_modules exists', async () => {
    const installRoot = tempInstallRoot();
    mkdirSync(join(installRoot, 'node_modules'));
    let setupCount = 0;
    const { commands, run } = fakeRunner({
      'git rev-parse --git-dir': '.git\n',
      'git rev-parse --short HEAD': 'abc123\n',
      'git branch --show-current': 'master\n',
      'git rev-parse --abbrev-ref --symbolic-full-name @{u}': 'origin/master\n',
      'git log --oneline HEAD..origin/master -10': 'def456 update docs\n',
      'git status --short': '',
      'git diff --name-only HEAD origin/master -- package.json bun.lock bun.lockb': '',
    });

    const result = await runManagedUpdate({
      repoRoot: installRoot,
      installRoot,
      runCommand: run,
      runSetup: async () => {
        setupCount += 1;
      },
      confirm: async () => true,
    });

    expect(result.status).toBe('updated');
    expect(result.installedDependencies).toBe(false);
    expect(setupCount).toBe(1);
    expect(commands.some(([cmd, args]) => cmd === 'bun' && args[0] === 'install')).toBe(false);
  });

  it('does not reset or setup when the destructive confirmation is declined', async () => {
    const installRoot = tempInstallRoot();
    let setupCount = 0;
    const { commands, run } = fakeRunner({
      'git rev-parse --git-dir': '.git\n',
      'git rev-parse --short HEAD': 'abc123\n',
      'git branch --show-current': 'master\n',
      'git rev-parse --abbrev-ref --symbolic-full-name @{u}': 'origin/master\n',
      'git log --oneline HEAD..origin/master -10': 'def456 update cli\n',
      'git status --short': ' M README.md\n',
      'git diff --name-only HEAD origin/master -- package.json bun.lock bun.lockb': '',
    });

    const result = await runManagedUpdate({
      repoRoot: installRoot,
      installRoot,
      runCommand: run,
      runSetup: async () => {
        setupCount += 1;
      },
      confirm: async () => false,
    });

    expect(result.status).toBe('cancelled');
    expect(setupCount).toBe(0);
    expect(commands.some(([cmd, args]) => cmd === 'git' && args[0] === 'reset')).toBe(false);
    expect(commands.some(([cmd, args]) => cmd === 'git' && args[0] === 'clean')).toBe(false);
  });

  it('does not reset, install, or setup when already current and clean', async () => {
    const installRoot = tempInstallRoot();
    mkdirSync(join(installRoot, 'node_modules'));
    let setupCount = 0;
    const { commands, run } = fakeRunner({
      'git rev-parse --git-dir': '.git\n',
      'git rev-parse --short HEAD': 'abc123\n',
      'git branch --show-current': 'master\n',
      'git rev-parse --abbrev-ref --symbolic-full-name @{u}': 'origin/master\n',
      'git log --oneline HEAD..origin/master -10': '',
      'git status --short': '',
      'git diff --name-only HEAD origin/master -- package.json bun.lock bun.lockb': '',
    });

    const result = await runManagedUpdate({
      repoRoot: installRoot,
      installRoot,
      runCommand: run,
      runSetup: async () => {
        setupCount += 1;
      },
      confirm: async () => {
        throw new Error('confirm should not be called for no-op updates');
      },
    });

    expect(result.status).toBe('up-to-date');
    expect(setupCount).toBe(0);
    expect(commands.some(([cmd, args]) => cmd === 'git' && args[0] === 'reset')).toBe(false);
    expect(commands.some(([cmd, args]) => cmd === 'bun' && args[0] === 'install')).toBe(false);
  });
});
