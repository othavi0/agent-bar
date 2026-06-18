import { afterEach, describe, expect, it, spyOn } from 'bun:test';
import * as fs from 'node:fs';
import * as doctor from '../src/doctor';
import { runSetup } from '../src/setup';
import * as wc from '../src/waybar-contract';
import * as wi from '../src/waybar-integration';

// Uses spyOn (restored after each test) rather than mock.module so it never
// replaces whole shared modules — that would strip their other exports and
// break unrelated test files when Bun loads them in the same process.
describe('runSetup — system (compiled) install', () => {
  const spies: Array<ReturnType<typeof spyOn>> = [];

  afterEach(() => {
    for (const s of spies) s.mockRestore();
    spies.length = 0;
    delete process.env.AGENT_BAR_FORCE_COMPILED;
  });

  it('completes, installs via resolveAssetSourceRoot (no repoRoot), uses PATH appBin, and skips the symlink', async () => {
    let installOpts: { repoRoot?: string } | undefined;
    let integrationOpts: { appBin?: string } | undefined;

    spies.push(
      spyOn(wc, 'installWaybarAssets').mockImplementation(((o: { repoRoot?: string }) => {
        installOpts = o;
        return { iconsDir: '/x/i', terminalScript: '/x/t' };
      }) as never),
    );
    spies.push(
      spyOn(wi, 'applyWaybarIntegration').mockImplementation(((o: { appBin?: string }) => {
        integrationOpts = o;
        return { configChanged: true, styleChanged: true };
      }) as never),
    );
    spies.push(
      spyOn(doctor, 'scan').mockImplementation((async () => ({
        packageJsonOrphan: false,
        packageJsonMixed: false,
        nodeModulesDir: null,
        lockfiles: [],
      })) as never),
    );
    // Belt-and-suspenders: never touch the real fs / Waybar even if the guard regressed.
    const symlinkSpy = spyOn(fs, 'symlinkSync').mockImplementation((() => {}) as never);
    spies.push(symlinkSpy);
    spies.push(spyOn(Bun, 'spawn').mockImplementation((() => ({})) as never));

    process.env.AGENT_BAR_FORCE_COMPILED = '1';

    const ok = await runSetup({ confirm: false, clearScreen: false });

    // No crash → the /$bunfs REPO_ROOT blocker is fixed.
    expect(ok).toBe(true);
    // Assets resolve via the default resolveAssetSourceRoot, not a hardwired repoRoot.
    expect(installOpts?.repoRoot).toBeUndefined();
    // Real getDefaultWaybarAssetPaths under a compiled binary → PATH-resolved appBin.
    expect(integrationOpts?.appBin).toBe('agent-bar');
    // The symlink is skipped on a system install.
    expect(symlinkSpy).not.toHaveBeenCalled();
  });
});
