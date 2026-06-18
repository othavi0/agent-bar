import { afterAll, afterEach, beforeEach, describe, expect, it, mock, spyOn } from 'bun:test';
import { existsSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

// setup.ts caches homedir() at module load, so HOME must be a temp dir BEFORE
// the dynamic import below — guarantees nothing touches the real ~/.local/bin.
const TMP_HOME = mkdtempSync(join(tmpdir(), 'ab-setup-home-'));
process.env.HOME = TMP_HOME;

// Capture the args the heavy collaborators receive.
const installArgs: Array<{ repoRoot?: string }> = [];
const integrationArgs: Array<{ appBin?: string }> = [];

mock.module('../src/waybar-contract', () => ({
  installWaybarAssets: (opts: { repoRoot?: string }) => {
    installArgs.push(opts);
    return { iconsDir: '/x/icons', terminalScript: '/x/helper' };
  },
  getDefaultWaybarAssetPaths: () => ({
    waybarDir: '/x/wd',
    scriptsDir: '/x/sd',
    iconsDir: '/x/id',
    terminalScript: '/x/ts',
    // simulate a compiled/system install (real logic is covered in waybar-contract.test.ts)
    appBin: 'agent-bar',
  }),
}));
mock.module('../src/waybar-integration', () => ({
  applyWaybarIntegration: (opts: { appBin?: string }) => {
    integrationArgs.push(opts);
    return { configChanged: true, styleChanged: true };
  },
  getDefaultWaybarIntegrationPaths: () => ({ waybarConfigPath: '/x/config.jsonc', waybarStylePath: '/x/style.css' }),
}));
mock.module('../src/doctor', () => ({
  scan: async () => ({ packageJsonOrphan: false, packageJsonMixed: false, nodeModulesDir: null, lockfiles: [] }),
}));

describe('runSetup — system (compiled) install', () => {
  let spawnSpy: ReturnType<typeof spyOn>;

  beforeEach(() => {
    installArgs.length = 0;
    integrationArgs.length = 0;
    // Stub Bun.spawn so reloadWaybar() never signals the real Waybar.
    spawnSpy = spyOn(Bun, 'spawn').mockImplementation((() => ({})) as never);
    process.env.AGENT_BAR_FORCE_COMPILED = '1';
  });

  afterEach(() => {
    spawnSpy.mockRestore();
    delete process.env.AGENT_BAR_FORCE_COMPILED;
  });

  afterAll(() => {
    rmSync(TMP_HOME, { recursive: true, force: true });
  });

  it('completes, installs assets via resolveAssetSourceRoot (no repoRoot), and skips the symlink', async () => {
    const { runSetup } = await import('../src/setup');
    const ok = await runSetup({ confirm: false, clearScreen: false });

    // No crash → the /$bunfs REPO_ROOT blocker is fixed.
    expect(ok).toBe(true);
    // Assets resolve via the default resolveAssetSourceRoot, not a hardwired repoRoot.
    expect(installArgs[0]?.repoRoot).toBeUndefined();
    // The generated module gets the PATH-resolved appBin.
    expect(integrationArgs[0]?.appBin).toBe('agent-bar');
    // The symlink is skipped on a system install (binary already lives in PATH).
    expect(existsSync(join(TMP_HOME, '.local', 'bin', 'agent-bar'))).toBe(false);
  });
});
