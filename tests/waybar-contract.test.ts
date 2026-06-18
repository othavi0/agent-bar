import { describe, expect, it } from 'bun:test';
import { mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { APP_NAME } from '../src/app-identity';
import {
  exportWaybarCss,
  exportWaybarModules,
  getDefaultWaybarAssetPaths,
  normalizeProviderSelection,
  resolveAssetSourceRoot,
  type WaybarCssExportOptions,
} from '../src/waybar-contract';

// ---------------------------------------------------------------------------
// exportWaybarModules
// ---------------------------------------------------------------------------

describe('exportWaybarModules', () => {
  it('wires left and right click handlers through the terminal helper', () => {
    const result = exportWaybarModules(
      {
        appBin: '$HOME/.local/bin/agent-bar',
        terminalScript: '$HOME/.config/waybar/scripts/agent-bar-open-terminal',
      },
      ['claude', 'codex', 'amp'],
    );

    expect(result.modules['custom/agent-bar-claude']['on-click']).toBe(
      '$HOME/.config/waybar/scripts/agent-bar-open-terminal $HOME/.local/bin/agent-bar menu',
    );
    expect(result.modules['custom/agent-bar-codex']['exec-on-event']).toBe(true);
    expect(result.modules['custom/agent-bar-codex'].exec).toBe('$HOME/.local/bin/agent-bar --provider codex');
    expect(result.modules['custom/agent-bar-amp']['on-click-right']).toBe(
      '$HOME/.config/waybar/scripts/agent-bar-open-terminal $HOME/.local/bin/agent-bar action-right amp',
    );
  });

  it('generates modules only for requested providers', () => {
    const result = exportWaybarModules(
      {
        appBin: '/usr/bin/agent-bar',
        terminalScript: '/usr/bin/open-terminal',
      },
      ['claude'],
    );

    expect(Object.keys(result.modules)).toHaveLength(1);
    expect(result.modules['custom/agent-bar-claude']).toBeDefined();
    expect(result.modules['custom/agent-bar-codex']).toBeUndefined();
  });

  it('includes signal in each module when provided', () => {
    const result = exportWaybarModules({ appBin: 'bin', terminalScript: 'term', signal: 8 }, ['claude', 'codex']);

    expect(result.modules['custom/agent-bar-claude'].signal).toBe(8);
    expect(result.modules['custom/agent-bar-codex'].signal).toBe(8);
  });

  it('omits signal when not provided', () => {
    const result = exportWaybarModules({ appBin: 'bin', terminalScript: 'term' }, ['claude']);

    expect('signal' in result.modules['custom/agent-bar-claude']).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// normalizeProviderSelection
// ---------------------------------------------------------------------------

describe('normalizeProviderSelection', () => {
  it('filters out unknown providers', () => {
    const result = normalizeProviderSelection(['claude', 'unknown', 'amp'], []);

    expect(result.providers).toEqual(['claude', 'amp']);
    expect(result.providerOrder).toEqual(['claude', 'amp']);
  });

  it('deduplicates providers', () => {
    const result = normalizeProviderSelection(['claude', 'claude', 'codex'], []);

    expect(result.providers).toEqual(['claude', 'codex']);
  });

  it('respects providerOrder for ordering', () => {
    const result = normalizeProviderSelection(['claude', 'codex', 'amp'], ['amp', 'claude', 'codex']);

    expect(result.providerOrder).toEqual(['amp', 'claude', 'codex']);
  });

  it('adds providers missing from providerOrder to the end', () => {
    const result = normalizeProviderSelection(['claude', 'codex', 'amp'], ['codex']);

    expect(result.providerOrder).toEqual(['codex', 'claude', 'amp']);
  });

  it('filters providerOrder entries not in enabled providers', () => {
    const result = normalizeProviderSelection(['claude'], ['amp', 'claude', 'codex']);

    expect(result.providerOrder).toEqual(['claude']);
  });

  it('handles empty input gracefully', () => {
    const result = normalizeProviderSelection([], []);

    expect(result.providers).toEqual([]);
    expect(result.providerOrder).toEqual([]);
  });
});

// ---------------------------------------------------------------------------
// exportWaybarCss
// ---------------------------------------------------------------------------

describe('exportWaybarCss', () => {
  const defaultOpts: WaybarCssExportOptions = {
    iconsDir: '/home/user/.config/waybar/agent-bar/icons',
    providerOrder: ['claude', 'codex', 'amp'],
    separators: 'gap',
  };

  function cssFor(separators: WaybarCssExportOptions['separators']): string {
    return exportWaybarCss({ ...defaultOpts, separators }).css;
  }

  it('includes base provider styles', () => {
    const css = cssFor('gap');
    expect(css).toContain('#custom-agent-bar-claude');
    expect(css).toContain('#custom-agent-bar-codex');
    expect(css).toContain('#custom-agent-bar-amp');
  });

  it('includes provider icon references', () => {
    const css = cssFor('gap');
    expect(css).toContain('claude-code-icon.png');
    expect(css).toContain('codex-icon.png');
    expect(css).toContain('amp-icon.svg');
  });

  it('includes color state selectors', () => {
    const css = cssFor('gap');
    expect(css).toContain('.ok');
    expect(css).toContain('.low');
    expect(css).toContain('.warn');
    expect(css).toContain('.critical');
    expect(css).toContain('.disconnected');
  });

  describe('separator styles', () => {
    const styles: WaybarCssExportOptions['separators'][] = ['pill', 'gap', 'bare', 'glass', 'shadow', 'none'];

    for (const style of styles) {
      it(`generates ${style} separator CSS`, () => {
        const css = cssFor(style);
        expect(css).toContain(`separators: ${style}`);
        expect(css.length).toBeGreaterThan(100);
      });
    }

    it('pill style includes border-radius', () => {
      expect(cssFor('pill')).toContain('border-radius');
    });

    it('bare style makes borders transparent', () => {
      expect(cssFor('bare')).toContain('border-color: transparent');
    });

    it('glass style includes rgba background', () => {
      expect(cssFor('glass')).toContain('rgba(');
    });

    it('shadow style includes box-shadow', () => {
      expect(cssFor('shadow')).toContain('box-shadow');
    });

    it('none style minimizes visual separators', () => {
      const css = cssFor('none');
      expect(css).toContain('border-color: transparent');
      expect(css).toContain('margin: 0');
    });
  });
});

// ---------------------------------------------------------------------------
// resolveAssetSourceRoot
// ---------------------------------------------------------------------------

describe('resolveAssetSourceRoot', () => {
  it('honors an absolute AGENT_BAR_ASSET_DIR that contains icons/', () => {
    const dir = mkdtempSync(join(tmpdir(), 'ab-assets-'));
    mkdirSync(join(dir, 'icons'), { recursive: true });
    process.env.AGENT_BAR_ASSET_DIR = dir;
    try {
      expect(resolveAssetSourceRoot()).toBe(dir);
    } finally {
      delete process.env.AGENT_BAR_ASSET_DIR;
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it('throws a clear error under a compiled binary with no assets', () => {
    process.env.AGENT_BAR_FORCE_COMPILED = '1';
    process.env.AGENT_BAR_ASSET_DIR = '/nonexistent-xyz';
    try {
      expect(() => resolveAssetSourceRoot()).toThrow(/Asset directory not found/);
    } finally {
      delete process.env.AGENT_BAR_FORCE_COMPILED;
      delete process.env.AGENT_BAR_ASSET_DIR;
    }
  });
});

// ---------------------------------------------------------------------------
// getDefaultWaybarAssetPaths().appBin
// ---------------------------------------------------------------------------

describe('getDefaultWaybarAssetPaths appBin', () => {
  it('uses a PATH-resolved appBin under a compiled (system) binary', () => {
    process.env.AGENT_BAR_FORCE_COMPILED = '1';
    try {
      expect(getDefaultWaybarAssetPaths().appBin).toBe('agent-bar');
    } finally {
      delete process.env.AGENT_BAR_FORCE_COMPILED;
    }
  });

  it('uses the ~/.local/bin appBin otherwise', () => {
    expect(getDefaultWaybarAssetPaths().appBin).toBe(`$HOME/.local/bin/${APP_NAME}`);
  });
});
