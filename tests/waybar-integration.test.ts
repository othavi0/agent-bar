import { afterEach, beforeEach, describe, expect, it } from 'bun:test';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import {
  applyWaybarIntegration,
  getAppModuleIDs,
  removeWaybarIntegration,
  type WaybarIntegrationPaths,
} from '../src/waybar-integration';

/** Strip // and /* *\/ comments so a JSONC string can go through JSON.parse. */
function stripJsonc(text: string): string {
  return text.replace(/\/\*[\s\S]*?\*\//g, '').replace(/^\s*\/\/.*$/gm, '');
}

describe('waybar config patcher', () => {
  let dir: string;
  let paths: WaybarIntegrationPaths;
  const assetOpts = { appBin: '/bin/agent-bar', terminalScript: '/bin/term', iconsDir: '/icons' };

  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'agent-bar-wb-'));
    paths = {
      waybarConfigPath: join(dir, 'config.jsonc'),
      waybarStylePath: join(dir, 'style.css'),
      modulesIncludePath: join(dir, 'agent-bar', 'modules.jsonc'),
      styleIncludePath: join(dir, 'agent-bar', 'style.css'),
    };
  });

  afterEach(() => {
    rmSync(dir, { recursive: true, force: true });
  });

  it('adds managed modules while preserving existing ones and valid JSON', () => {
    writeFileSync(
      paths.waybarConfigPath,
      `{
  "modules-right": ["clock", "battery"],
  "include": ["/existing/include.jsonc"]
}`,
    );

    const result = applyWaybarIntegration({ paths, ...assetOpts });
    expect(result.configChanged).toBe(true);

    const patched = readFileSync(paths.waybarConfigPath, 'utf8');
    const parsed = JSON.parse(stripJsonc(patched));

    // user modules preserved, managed appended
    expect(parsed['modules-right']).toContain('clock');
    expect(parsed['modules-right']).toContain('battery');
    for (const id of getAppModuleIDs(['claude', 'codex', 'amp'])) {
      expect(parsed['modules-right']).toContain(id);
    }
    // include preserved + managed include added
    expect(parsed.include).toContain('/existing/include.jsonc');
    expect(parsed.include).toContain(paths.modulesIncludePath);
  });

  it('does NOT corrupt the config when modules-right holds a nested array (stays valid JSON)', () => {
    // The old non-greedy regex truncated at the first nested `]`, producing
    // invalid JSON. The bracket-aware scanner must keep the file parseable.
    writeFileSync(
      paths.waybarConfigPath,
      `{
  "modules-right": ["clock", {"name": "x", "items": ["a", "b"]}],
  "include": []
}`,
    );

    applyWaybarIntegration({ paths, ...assetOpts });
    const patched = readFileSync(paths.waybarConfigPath, 'utf8');

    // The critical guarantee: the result is still parseable JSON (no dangling
    // `]}` suffix from a truncated match).
    expect(() => JSON.parse(stripJsonc(patched))).not.toThrow();
  });

  it('leaves a commented-out modules-right line untouched', () => {
    writeFileSync(
      paths.waybarConfigPath,
      `{
  // "modules-right": ["old-module"],
  "modules-right": ["clock"],
  "include": []
}`,
    );

    applyWaybarIntegration({ paths, ...assetOpts });
    const patched = readFileSync(paths.waybarConfigPath, 'utf8');

    // The comment must still read exactly as authored.
    expect(patched).toContain('// "modules-right": ["old-module"],');
    // The live array got the managed modules.
    const parsed = JSON.parse(stripJsonc(patched));
    expect(parsed['modules-right']).toContain('clock');
    expect(parsed['modules-right']).toContain('custom/agent-bar-claude');
  });

  it('round-trips: remove reverses apply and backs up before mutating', () => {
    writeFileSync(paths.waybarConfigPath, `{\n  "modules-right": ["clock"],\n  "include": []\n}`);
    writeFileSync(paths.waybarStylePath, `window { color: red; }\n`);

    applyWaybarIntegration({ paths, ...assetOpts });
    const removeResult = removeWaybarIntegration({ paths });
    expect(removeResult.configChanged).toBe(true);

    const finalConfig = JSON.parse(stripJsonc(readFileSync(paths.waybarConfigPath, 'utf8')));
    for (const id of getAppModuleIDs(['claude', 'codex', 'amp'])) {
      expect(finalConfig['modules-right']).not.toContain(id);
    }
    expect(finalConfig['modules-right']).toContain('clock');

    // backup was created before removal mutated the files
    expect(readFileSync(`${paths.waybarStylePath}.agent-bar-backup`, 'utf8')).toContain('window { color: red; }');
  });
});
