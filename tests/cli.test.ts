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

import { parseArgs, showHelp } from '../src/cli';

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

    it('parses the doctor command', () => {
      expect(parseArgs(['doctor']).command).toBe('doctor');
    });

    it('parses --dry-run and --yes flags for doctor', () => {
      const opts = parseArgs(['doctor', '--dry-run', '--yes']);
      expect(opts.command).toBe('doctor');
      expect(opts.dryRun).toBe(true);
      expect(opts.yes).toBe(true);
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

    it('parses --version and -V', () => {
      expect(parseArgs(['--version']).command).toBe('version');
      expect(parseArgs(['-V']).command).toBe('version');
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

describe('unknown commands', () => {
  it('exits 1 on unknown command with suggestion (typo)', () => {
    const originalExit = process.exit;
    const originalError = console.error;
    const exitCalls: number[] = [];
    const errors: string[] = [];

    process.exit = ((code?: number) => {
      exitCalls.push(code ?? 0);
      throw new Error('__exit__');
    }) as typeof process.exit;
    console.error = (...args: unknown[]) => {
      errors.push(args.join(' '));
    };

    try {
      expect(() => parseArgs(['setip'])).toThrow('__exit__');
      expect(exitCalls).toEqual([1]);
      expect(errors.join('\n')).toContain("Did you mean 'setup'");
    } finally {
      process.exit = originalExit;
      console.error = originalError;
    }
  });

  it('exits 1 on unknown command without suggestion', () => {
    const originalExit = process.exit;
    const originalError = console.error;
    const exitCalls: number[] = [];
    const errors: string[] = [];

    process.exit = ((code?: number) => {
      exitCalls.push(code ?? 0);
      throw new Error('__exit__');
    }) as typeof process.exit;
    console.error = (...args: unknown[]) => {
      errors.push(args.join(' '));
    };

    try {
      expect(() => parseArgs(['xyzzy'])).toThrow('__exit__');
      expect(exitCalls).toEqual([1]);
      expect(errors.join('\n')).toContain('Unknown command: xyzzy');
      expect(errors.join('\n')).toContain('help');
    } finally {
      process.exit = originalExit;
      console.error = originalError;
    }
  });

  it('exits 1 on a two-word subcommand missing its second word', () => {
    const originalExit = process.exit;
    const originalError = console.error;
    const exitCalls: number[] = [];
    const errors: string[] = [];

    process.exit = ((code?: number) => {
      exitCalls.push(code ?? 0);
      throw new Error('__exit__');
    }) as typeof process.exit;
    console.error = (...args: unknown[]) => {
      errors.push(args.join(' '));
    };

    try {
      expect(() => parseArgs(['assets'])).toThrow('__exit__');
      expect(() => parseArgs(['export'])).toThrow('__exit__');
      expect(exitCalls).toEqual([1, 1]);
      expect(errors.join('\n')).toContain('assets install');
      expect(errors.join('\n')).toContain('waybar-modules');
    } finally {
      process.exit = originalExit;
      console.error = originalError;
    }
  });

  it('does not exit on unknown flag (warn only)', () => {
    const originalExit = process.exit;
    let exited = false;
    process.exit = (() => {
      exited = true;
      throw new Error('__exit__');
    }) as typeof process.exit;

    try {
      const opts = parseArgs(['--unknown-flag']);
      expect(exited).toBe(false);
      expect(opts.command).toBe('waybar');
    } finally {
      process.exit = originalExit;
    }
  });
});

describe('showHelp', () => {
  it('describes npm-era entrypoints and managed checkout updates', () => {
    const lines: string[] = [];
    const originalLog = console.log;
    console.log = (...args: unknown[]) => {
      lines.push(args.join(' '));
    };

    try {
      showHelp();
    } finally {
      console.log = originalLog;
    }

    const output = lines.join('\n');
    expect(output).toContain('Update the install (npm or managed checkout)');
    expect(output).toContain('agent-bar  or  bun run start');
  });
});
