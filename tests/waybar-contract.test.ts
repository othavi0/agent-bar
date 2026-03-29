import { describe, expect, it } from 'bun:test';
import {
  exportWaybarCss,
  exportWaybarModules,
  normalizeProviderSelection,
  type WaybarCssExportOptions,
} from '../src/waybar-contract';

// ---------------------------------------------------------------------------
// exportWaybarModules
// ---------------------------------------------------------------------------

describe('exportWaybarModules', () => {
  it('wires left and right click handlers through the terminal helper', () => {
    const result = exportWaybarModules(
      {
        appBin: '$HOME/.local/bin/agent-bar-omarchy',
        terminalScript: '$HOME/.config/waybar/scripts/agent-bar-omarchy-open-terminal',
      },
      ['claude', 'codex', 'amp'],
    );

    expect(result.modules['custom/agent-bar-omarchy-claude']['on-click']).toBe(
      '$HOME/.config/waybar/scripts/agent-bar-omarchy-open-terminal $HOME/.local/bin/agent-bar-omarchy menu',
    );
    expect(result.modules['custom/agent-bar-omarchy-codex']['exec-on-event']).toBe(true);
    expect(result.modules['custom/agent-bar-omarchy-amp']['on-click-right']).toBe(
      '$HOME/.config/waybar/scripts/agent-bar-omarchy-open-terminal $HOME/.local/bin/agent-bar-omarchy action-right amp',
    );
  });

  it('generates modules only for requested providers', () => {
    const result = exportWaybarModules(
      {
        appBin: '/usr/bin/agent-bar-omarchy',
        terminalScript: '/usr/bin/open-terminal',
      },
      ['claude'],
    );

    expect(Object.keys(result.modules)).toHaveLength(1);
    expect(result.modules['custom/agent-bar-omarchy-claude']).toBeDefined();
    expect(result.modules['custom/agent-bar-omarchy-codex']).toBeUndefined();
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
    iconsDir: '/home/user/.config/waybar/agent-bar-omarchy/icons',
    providerOrder: ['claude', 'codex', 'amp'],
    separators: 'gap',
  };

  function cssFor(separators: WaybarCssExportOptions['separators']): string {
    return exportWaybarCss({ ...defaultOpts, separators }).css;
  }

  it('includes base provider styles', () => {
    const css = cssFor('gap');
    expect(css).toContain('#custom-agent-bar-omarchy-claude');
    expect(css).toContain('#custom-agent-bar-omarchy-codex');
    expect(css).toContain('#custom-agent-bar-omarchy-amp');
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
