import { afterEach, beforeEach, describe, expect, it } from 'bun:test';
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { runDoctor, scan } from '../src/doctor';

function makeHome(): string {
  return mkdtempSync(join(tmpdir(), 'agent-bar-doctor-'));
}

describe('doctor.scan', () => {
  let home: string;

  beforeEach(() => {
    home = makeHome();
  });

  afterEach(() => {
    rmSync(home, { recursive: true, force: true });
  });

  it('returns clean findings when $HOME has nothing relevant', async () => {
    const findings = await scan(home);
    expect(findings.packageJsonOrphan).toBe(false);
    expect(findings.packageJsonMixed).toBe(false);
    expect(findings.nodeModulesDir).toBe(null);
    expect(findings.lockfiles).toEqual([]);
  });

  it('detects orphan package.json with only @noctuacore/agent-bar', async () => {
    writeFileSync(join(home, 'package.json'), JSON.stringify({ dependencies: { '@noctuacore/agent-bar': '^4.0.0' } }));
    const findings = await scan(home);
    expect(findings.packageJsonOrphan).toBe(true);
    expect(findings.packageJsonMixed).toBe(false);
  });

  it('flags mixed package.json (agent-bar + other deps)', async () => {
    writeFileSync(
      join(home, 'package.json'),
      JSON.stringify({
        dependencies: { '@noctuacore/agent-bar': '^4.0.0', other: '1.0.0' },
      }),
    );
    const findings = await scan(home);
    expect(findings.packageJsonOrphan).toBe(false);
    expect(findings.packageJsonMixed).toBe(true);
  });

  it('ignores package.json that does not mention agent-bar', async () => {
    writeFileSync(join(home, 'package.json'), JSON.stringify({ dependencies: { other: '1.0.0' } }));
    const findings = await scan(home);
    expect(findings.packageJsonOrphan).toBe(false);
    expect(findings.packageJsonMixed).toBe(false);
  });

  it('detects node_modules/@noctuacore/agent-bar', async () => {
    mkdirSync(join(home, 'node_modules', '@noctuacore', 'agent-bar'), { recursive: true });
    const findings = await scan(home);
    expect(findings.nodeModulesDir).toBe(join(home, 'node_modules', '@noctuacore', 'agent-bar'));
  });

  it('lists lockfiles only when package.json is orphan or missing', async () => {
    writeFileSync(join(home, 'bun.lock'), '');
    writeFileSync(join(home, 'package-lock.json'), '{}');
    const orphanLess = await scan(home);
    expect(orphanLess.lockfiles).toEqual([join(home, 'bun.lock'), join(home, 'package-lock.json')]);

    writeFileSync(join(home, 'package.json'), JSON.stringify({ dependencies: { other: '1.0.0' } }));
    const legit = await scan(home);
    expect(legit.lockfiles).toEqual([]);
  });

  it('considers devDependencies too when classifying package.json', async () => {
    writeFileSync(
      join(home, 'package.json'),
      JSON.stringify({ devDependencies: { '@noctuacore/agent-bar': '^4.0.0' } }),
    );
    const findings = await scan(home);
    expect(findings.packageJsonOrphan).toBe(true);
  });
});

describe('doctor.runDoctor', () => {
  let home: string;

  beforeEach(() => {
    home = makeHome();
  });

  afterEach(() => {
    rmSync(home, { recursive: true, force: true });
  });

  it('returns clean when $HOME has nothing', async () => {
    const result = await runDoctor({ home, confirm: async () => true });
    expect(result.status).toBe('clean');
    expect(result.removed).toEqual([]);
  });

  it('removes orphan package.json + lockfile + node_modules when confirmed', async () => {
    writeFileSync(join(home, 'package.json'), JSON.stringify({ dependencies: { '@noctuacore/agent-bar': '^4.0.0' } }));
    writeFileSync(join(home, 'bun.lock'), '');
    mkdirSync(join(home, 'node_modules', '@noctuacore', 'agent-bar'), { recursive: true });

    const result = await runDoctor({ home, confirm: async () => true });

    expect(result.status).toBe('cleaned');
    expect(result.removed.sort()).toEqual(
      [
        join(home, 'package.json'),
        join(home, 'bun.lock'),
        join(home, 'node_modules', '@noctuacore', 'agent-bar'),
      ].sort(),
    );
    expect(existsSync(join(home, 'package.json'))).toBe(false);
    expect(existsSync(join(home, 'bun.lock'))).toBe(false);
    expect(existsSync(join(home, 'node_modules', '@noctuacore', 'agent-bar'))).toBe(false);
  });

  it('on mixed package.json: removes node_modules but keeps package.json + lockfile', async () => {
    writeFileSync(
      join(home, 'package.json'),
      JSON.stringify({
        dependencies: { '@noctuacore/agent-bar': '^4.0.0', other: '1.0.0' },
      }),
    );
    writeFileSync(join(home, 'bun.lock'), '');
    mkdirSync(join(home, 'node_modules', '@noctuacore', 'agent-bar'), { recursive: true });

    const result = await runDoctor({ home, confirm: async () => true });

    expect(result.status).toBe('mixed-only');
    expect(result.removed).toEqual([join(home, 'node_modules', '@noctuacore', 'agent-bar')]);
    expect(existsSync(join(home, 'package.json'))).toBe(true);
    expect(existsSync(join(home, 'bun.lock'))).toBe(true);
  });

  it('returns cancelled when confirm rejects', async () => {
    writeFileSync(join(home, 'package.json'), JSON.stringify({ dependencies: { '@noctuacore/agent-bar': '^4.0.0' } }));
    const result = await runDoctor({ home, confirm: async () => false });
    expect(result.status).toBe('cancelled');
    expect(result.removed).toEqual([]);
    expect(existsSync(join(home, 'package.json'))).toBe(true);
  });

  it('--dry-run: reports without removing', async () => {
    writeFileSync(join(home, 'package.json'), JSON.stringify({ dependencies: { '@noctuacore/agent-bar': '^4.0.0' } }));
    writeFileSync(join(home, 'bun.lock'), '');
    mkdirSync(join(home, 'node_modules', '@noctuacore', 'agent-bar'), { recursive: true });

    const result = await runDoctor({ home, dryRun: true, confirm: async () => true });

    expect(result.status).toBe('cleaned');
    expect(result.removed.sort()).toEqual(
      [
        join(home, 'package.json'),
        join(home, 'bun.lock'),
        join(home, 'node_modules', '@noctuacore', 'agent-bar'),
      ].sort(),
    );
    expect(existsSync(join(home, 'package.json'))).toBe(true);
    expect(existsSync(join(home, 'bun.lock'))).toBe(true);
    expect(existsSync(join(home, 'node_modules', '@noctuacore', 'agent-bar'))).toBe(true);
  });

  it('--yes: skips confirm callback', async () => {
    writeFileSync(join(home, 'package.json'), JSON.stringify({ dependencies: { '@noctuacore/agent-bar': '^4.0.0' } }));
    let confirmCalled = false;
    const result = await runDoctor({
      home,
      yes: true,
      confirm: async () => {
        confirmCalled = true;
        return false;
      },
    });
    expect(confirmCalled).toBe(false);
    expect(result.status).toBe('cleaned');
    expect(existsSync(join(home, 'package.json'))).toBe(false);
  });
});
