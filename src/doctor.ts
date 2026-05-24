import { existsSync, readFileSync, rmSync, statSync } from 'node:fs';
import { join } from 'node:path';
import * as p from '@clack/prompts';
import { colorize, semantic } from './tui/colors';
import { printCommandHeader, printKeyValues, printWarning } from './tui/terminal-ui';

const TARGET_PACKAGE = '@noctuacore/agent-bar';
const LOCKFILE_NAMES = ['bun.lock', 'bun.lockb', 'package-lock.json'] as const;

export interface DoctorFindings {
  packageJsonPath: string | null;
  packageJsonOrphan: boolean;
  packageJsonMixed: boolean;
  nodeModulesDir: string | null;
  lockfiles: string[];
}

interface PackageJsonShape {
  dependencies?: Record<string, string>;
  devDependencies?: Record<string, string>;
}

function readJson(path: string): PackageJsonShape | null {
  try {
    return JSON.parse(readFileSync(path, 'utf8')) as PackageJsonShape;
  } catch {
    return null;
  }
}

function classifyPackageJson(pkg: PackageJsonShape | null): { orphan: boolean; mixed: boolean } {
  if (!pkg) return { orphan: false, mixed: false };
  const deps = {
    ...(pkg.dependencies ?? {}),
    ...(pkg.devDependencies ?? {}),
  };
  const names = Object.keys(deps);
  if (!names.includes(TARGET_PACKAGE)) return { orphan: false, mixed: false };
  if (names.length === 1) return { orphan: true, mixed: false };
  return { orphan: false, mixed: true };
}

function findNodeModulesDir(home: string): string | null {
  const dir = join(home, 'node_modules', '@noctuacore', 'agent-bar');
  try {
    if (statSync(dir).isDirectory()) return dir;
  } catch {
    // directory does not exist — expected
  }
  return null;
}

function findLockfiles(home: string, classification: 'orphan' | 'mixed' | 'legit' | 'none'): string[] {
  // Only surface lockfiles when there is no legitimate package.json anchoring them.
  // 'legit' = package.json exists but does not mention agent-bar (real project, leave it alone).
  // 'mixed' = agent-bar is one of several deps (same: real project).
  if (classification === 'mixed' || classification === 'legit') return [];
  return LOCKFILE_NAMES.map((name) => join(home, name)).filter((p) => existsSync(p));
}

export type DoctorStatus = 'clean' | 'cancelled' | 'cleaned' | 'mixed-only';

export interface DoctorResult {
  status: DoctorStatus;
  removed: string[];
  findings: DoctorFindings;
}

export interface DoctorOptions {
  home: string;
  dryRun?: boolean;
  yes?: boolean;
  confirm: (findings: DoctorFindings) => Promise<boolean>;
}

function plannedRemovals(findings: DoctorFindings): string[] {
  const items: string[] = [];
  if (findings.packageJsonOrphan && findings.packageJsonPath) items.push(findings.packageJsonPath);
  if (findings.nodeModulesDir) items.push(findings.nodeModulesDir);
  if (!findings.packageJsonMixed) items.push(...findings.lockfiles);
  return items;
}

function performRemoval(path: string): void {
  rmSync(path, { recursive: true, force: true });
}

export async function runDoctor(options: DoctorOptions): Promise<DoctorResult> {
  const findings = await scan(options.home);

  const nothingToDo =
    !findings.packageJsonOrphan &&
    !findings.packageJsonMixed &&
    !findings.nodeModulesDir &&
    findings.lockfiles.length === 0;

  if (nothingToDo) {
    return { status: 'clean', removed: [], findings };
  }

  const approved = options.yes ? true : await options.confirm(findings);
  if (!approved) {
    return { status: 'cancelled', removed: [], findings };
  }

  const removals = plannedRemovals(findings);

  if (!options.dryRun) {
    for (const path of removals) {
      performRemoval(path);
    }
  }

  const status: DoctorStatus = findings.packageJsonMixed && !findings.packageJsonOrphan ? 'mixed-only' : 'cleaned';
  return { status, removed: removals, findings };
}

export async function scan(home: string): Promise<DoctorFindings> {
  const packageJsonPath = join(home, 'package.json');
  const pkgExists = existsSync(packageJsonPath);
  const pkg = pkgExists ? readJson(packageJsonPath) : null;
  const { orphan, mixed } = classifyPackageJson(pkg);
  const classification: 'orphan' | 'mixed' | 'legit' | 'none' = orphan
    ? 'orphan'
    : mixed
      ? 'mixed'
      : pkgExists
        ? 'legit'
        : 'none';

  return {
    packageJsonPath: pkg !== null ? packageJsonPath : null,
    packageJsonOrphan: orphan,
    packageJsonMixed: mixed,
    nodeModulesDir: findNodeModulesDir(home),
    lockfiles: findLockfiles(home, classification),
  };
}

function describeFindings(findings: DoctorFindings): Array<[string, string]> {
  const rows: Array<[string, string]> = [];
  if (findings.packageJsonOrphan && findings.packageJsonPath) {
    rows.push(['Orphan package.json', findings.packageJsonPath]);
  }
  if (findings.packageJsonMixed && findings.packageJsonPath) {
    rows.push(['Mixed package.json (kept)', findings.packageJsonPath]);
  }
  if (findings.nodeModulesDir) {
    rows.push(['node_modules', findings.nodeModulesDir]);
  }
  for (const lock of findings.lockfiles) {
    rows.push(['Lockfile', lock]);
  }
  return rows;
}

export interface DoctorMainOptions {
  dryRun?: boolean;
  yes?: boolean;
}

export async function main(opts: DoctorMainOptions = {}): Promise<void> {
  console.clear();
  printCommandHeader('doctor', `Detect & clean ${TARGET_PACKAGE} leftovers in $HOME`);

  const dryRun = opts.dryRun ?? false;
  const yes = opts.yes ?? false;
  const home = process.env.HOME ?? process.env.USERPROFILE ?? '';

  if (!home) {
    p.log.error(colorize('Could not resolve $HOME', semantic.danger));
    process.exit(1);
  }

  try {
    const result = await runDoctor({
      home,
      dryRun,
      yes,
      confirm: async (findings) => {
        const rows = describeFindings(findings);
        if (rows.length === 0) return false;
        printKeyValues('Found', rows);

        if (findings.packageJsonMixed) {
          printWarning('package.json kept', [
            'It contains other dependencies — likely a real project.',
            'Only node_modules/@noctuacore will be removed.',
          ]);
        }

        const proceed = await p.confirm({
          message: dryRun ? 'Show what would be removed?' : 'Remove the leftovers above?',
          initialValue: true,
        });
        return !p.isCancel(proceed) && proceed;
      },
    });

    if (result.status === 'clean') {
      p.outro(colorize('Nothing to clean — $HOME is tidy.', semantic.good));
      return;
    }

    if (result.status === 'cancelled') {
      p.outro(colorize('Doctor cancelled', semantic.muted));
      return;
    }

    if (dryRun) {
      p.outro(colorize(`Dry run — would remove ${result.removed.length} item(s).`, semantic.good));
      return;
    }

    if (result.status === 'mixed-only') {
      p.outro(colorize(`Removed ${result.removed.length} item(s). package.json kept.`, semantic.good));
      return;
    }

    p.outro(colorize(`Cleaned ${result.removed.length} item(s) from $HOME.`, semantic.good));
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    p.log.error(colorize(message, semantic.danger));
    p.outro(colorize('Doctor failed', semantic.danger));
    process.exit(1);
  }
}

if (import.meta.main) {
  main().catch((e) => {
    console.error('Doctor failed:', e);
    process.exit(1);
  });
}
