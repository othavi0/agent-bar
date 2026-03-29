import { describe, expect, it, mock } from 'bun:test';

// Suppress logger noise during tests
mock.module('../src/logger', () => ({
  logger: {
    debug: () => {},
    info: () => {},
    warn: () => {},
    error: () => {},
  },
}));

import { parseArgs } from '../src/cli';

describe('parseArgs', () => {
  // -----------------------------------------------------------------------
  // Default behavior
  // -----------------------------------------------------------------------

  it('defaults to waybar command with no args', () => {
    const opts = parseArgs([]);
    expect(opts.command).toBe('waybar');
    expect(opts.refresh).toBe(false);
    expect(opts.verbose).toBe(false);
  });

  // -----------------------------------------------------------------------
  // Commands
  // -----------------------------------------------------------------------

  describe('commands', () => {
    it('parses menu', () => {
      expect(parseArgs(['menu']).command).toBe('menu');
    });

    it('parses status', () => {
      expect(parseArgs(['status']).command).toBe('status');
    });

    it('parses setup', () => {
      expect(parseArgs(['setup']).command).toBe('setup');
    });

    it('parses assets install', () => {
      expect(parseArgs(['assets', 'install']).command).toBe('assets-install');
    });

    it('parses apply-local (hyphenated)', () => {
      expect(parseArgs(['apply-local']).command).toBe('apply-local');
    });

    it('parses apply local (space-separated)', () => {
      expect(parseArgs(['apply', 'local']).command).toBe('apply-local');
    });

    it('parses export waybar-modules', () => {
      expect(parseArgs(['export', 'waybar-modules']).command).toBe('export-waybar-modules');
    });

    it('parses export waybar-css', () => {
      expect(parseArgs(['export', 'waybar-css']).command).toBe('export-waybar-css');
    });

    it('parses update', () => {
      expect(parseArgs(['update']).command).toBe('update');
    });

    it('parses uninstall', () => {
      expect(parseArgs(['uninstall']).command).toBe('uninstall');
    });

    it('parses remove', () => {
      expect(parseArgs(['remove']).command).toBe('remove');
    });

    it('parses help', () => {
      expect(parseArgs(['help']).command).toBe('help');
    });

    it('parses --help flag', () => {
      expect(parseArgs(['--help']).command).toBe('help');
    });

    it('parses -h flag', () => {
      expect(parseArgs(['-h']).command).toBe('help');
    });

    it('parses action-right with provider', () => {
      const opts = parseArgs(['action-right', 'claude']);
      expect(opts.command).toBe('action-right');
      expect(opts.provider).toBe('claude');
    });
  });

  // -----------------------------------------------------------------------
  // Flags
  // -----------------------------------------------------------------------

  describe('flags', () => {
    it('parses --refresh', () => {
      expect(parseArgs(['--refresh']).refresh).toBe(true);
    });

    it('parses -r shorthand', () => {
      expect(parseArgs(['-r']).refresh).toBe(true);
    });

    it('parses --verbose', () => {
      expect(parseArgs(['--verbose']).verbose).toBe(true);
    });

    it('parses -v shorthand', () => {
      expect(parseArgs(['-v']).verbose).toBe(true);
    });

    it('parses --terminal / -t', () => {
      expect(parseArgs(['--terminal']).command).toBe('terminal');
      expect(parseArgs(['-t']).command).toBe('terminal');
    });

    it('parses --provider with value', () => {
      expect(parseArgs(['--provider', 'codex']).provider).toBe('codex');
    });

    it('parses -p shorthand', () => {
      expect(parseArgs(['-p', 'amp']).provider).toBe('amp');
    });

    it('parses --waybar-dir', () => {
      expect(parseArgs(['--waybar-dir', '/custom/path']).waybarDir).toBe('/custom/path');
    });

    it('parses --scripts-dir', () => {
      expect(parseArgs(['--scripts-dir', '/scripts']).scriptsDir).toBe('/scripts');
    });

    it('parses --icons-dir', () => {
      expect(parseArgs(['--icons-dir', '/icons']).iconsDir).toBe('/icons');
    });

    it('parses --app-bin', () => {
      expect(parseArgs(['--app-bin', '/usr/bin/app']).appBin).toBe('/usr/bin/app');
    });

    it('parses --terminal-script', () => {
      expect(parseArgs(['--terminal-script', '/bin/launch']).terminalScript).toBe('/bin/launch');
    });
  });

  // -----------------------------------------------------------------------
  // Combinations
  // -----------------------------------------------------------------------

  describe('flag combinations', () => {
    it('combines command with flags', () => {
      const opts = parseArgs(['status', '--refresh', '--verbose', '-p', 'claude']);
      expect(opts.command).toBe('status');
      expect(opts.refresh).toBe(true);
      expect(opts.verbose).toBe(true);
      expect(opts.provider).toBe('claude');
    });

    it('flags before command', () => {
      const opts = parseArgs(['-v', '-r', 'menu']);
      expect(opts.command).toBe('menu');
      expect(opts.verbose).toBe(true);
      expect(opts.refresh).toBe(true);
    });
  });
});
