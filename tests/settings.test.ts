import { afterEach, beforeEach, describe, expect, it } from 'bun:test';
import { existsSync } from 'node:fs';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { getSettingsPath, loadSettings, loadSettingsSync, saveSettings } from '../src/settings';

describe('settings', () => {
  let testRoot = '';
  let previousXdgConfigHome: string | undefined;

  beforeEach(async () => {
    testRoot = await mkdtemp(join(tmpdir(), 'agent-bar-settings-test-'));
    previousXdgConfigHome = process.env.XDG_CONFIG_HOME;
    process.env.XDG_CONFIG_HOME = testRoot;
  });

  afterEach(async () => {
    if (previousXdgConfigHome === undefined) {
      delete process.env.XDG_CONFIG_HOME;
    } else {
      process.env.XDG_CONFIG_HOME = previousXdgConfigHome;
    }

    await rm(testRoot, { recursive: true, force: true });
  });

  it('returns normalized defaults when no file exists', () => {
    const settings = loadSettingsSync();

    expect(settings.version).toBe(2);
    expect(settings.waybar.providers).toEqual(['claude', 'codex', 'amp']);
    expect(settings.waybar.providerOrder).toEqual(['claude', 'codex', 'amp']);
    expect(settings.waybar.separators).toBe('gap');
    expect(settings.notify?.enabled).toBe(true);
    expect(getSettingsPath()).toBe(join(testRoot, 'agent-bar', 'settings.json'));
  });

  it('preserves notify.enabled=false and defaults it to true otherwise', async () => {
    const settingsDir = join(testRoot, 'agent-bar');
    await mkdir(settingsDir, { recursive: true });
    await writeFile(
      join(settingsDir, 'settings.json'),
      JSON.stringify({ version: 2, waybar: { providers: ['claude'] }, notify: { enabled: false } }),
    );

    const off = await loadSettings();
    expect(off.notify?.enabled).toBe(false);

    await writeFile(
      join(settingsDir, 'settings.json'),
      JSON.stringify({ version: 2, waybar: { providers: ['claude'] } }),
    );
    const on = await loadSettings();
    expect(on.notify?.enabled).toBe(true);
  });

  it('saves and loads settings from the new namespace', async () => {
    await saveSettings({
      version: 1,
      waybar: {
        providers: ['codex', 'claude'],
        showPercentage: false,
        separators: 'pill',
        providerOrder: ['codex', 'claude'],
      },
      tooltip: {},
      models: {},
      windowPolicy: { codex: 'both' },
    });

    const settingsPath = getSettingsPath();
    expect(settingsPath).toBe(join(testRoot, 'agent-bar', 'settings.json'));
    expect(existsSync(settingsPath)).toBe(true);

    const loaded = await loadSettings();
    expect(loaded.waybar.providers).toEqual(['codex', 'claude']);
    expect(loaded.waybar.providerOrder).toEqual(['codex', 'claude']);
    expect(loaded.waybar.separators).toBe('pill');

    const onDisk = JSON.parse(await readFile(settingsPath, 'utf8'));
    expect(onDisk.waybar.providers).toEqual(['codex', 'claude']);
  });

  it('drops unknown providers and preserves a customized provider list', async () => {
    const settingsDir = join(testRoot, 'agent-bar');
    const settingsFile = join(settingsDir, 'settings.json');
    await mkdir(settingsDir, { recursive: true });
    await writeFile(
      settingsFile,
      JSON.stringify({
        version: 1,
        waybar: {
          // 'copilot' is no longer a known provider and must be dropped on load.
          providers: ['claude', 'copilot', 'amp'],
          showPercentage: true,
          separators: 'gap',
          providerOrder: ['amp', 'copilot', 'claude'],
        },
        tooltip: {},
        models: {},
        windowPolicy: {},
      }),
    );

    const settings = await loadSettings();

    expect(settings.waybar.providers).toEqual(['claude', 'amp']);
    expect(settings.waybar.providerOrder).toEqual(['amp', 'claude']);
  });

  describe('Settings displayMode', () => {
    it('default is "remaining" when not set', async () => {
      const { loadSettings } = await import('../src/settings');
      const s = await loadSettings();
      expect(s.waybar.displayMode).toBe('remaining');
    });

    it('rejects invalid value, falls back to "remaining"', async () => {
      const { loadSettings, saveSettings } = await import('../src/settings');
      const s = await loadSettings();
      // @ts-expect-error testando valor inválido
      s.waybar.displayMode = 'bogus';
      await saveSettings(s);
      const reloaded = await loadSettings();
      expect(reloaded.waybar.displayMode).toBe('remaining');
    });

    it('persists "used" value round-trip', async () => {
      const { loadSettings, saveSettings } = await import('../src/settings');
      const s = await loadSettings();
      s.waybar.displayMode = 'used';
      await saveSettings(s);
      const reloaded = await loadSettings();
      expect(reloaded.waybar.displayMode).toBe('used');
    });
  });

  describe('Settings waybar.signal', () => {
    it('is undefined by default', () => {
      const s = loadSettingsSync();
      expect(s.waybar.signal).toBeUndefined();
    });

    it('preserves a valid signal (1..30)', async () => {
      const settingsDir = join(testRoot, 'agent-bar');
      await mkdir(settingsDir, { recursive: true });
      await writeFile(
        join(settingsDir, 'settings.json'),
        JSON.stringify({ version: 2, waybar: { providers: ['claude'], signal: 8 } }),
      );
      const s = await loadSettings();
      expect(s.waybar.signal).toBe(8);
    });

    it('drops an invalid signal (0, negative, non-integer, out of range, wrong type)', async () => {
      const settingsDir = join(testRoot, 'agent-bar');
      await mkdir(settingsDir, { recursive: true });
      const settingsFile = join(settingsDir, 'settings.json');
      for (const bad of [0, -1, 3.5, 99, 'x']) {
        await writeFile(
          settingsFile,
          JSON.stringify({ version: 2, waybar: { providers: ['claude'], signal: bad } }),
        );
        const s = await loadSettings();
        expect(s.waybar.signal).toBeUndefined();
      }
    });
  });
});
