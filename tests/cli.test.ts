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

function expectExit1(args: string[], needle: string) {
  const origExit = process.exit;
  const origErr = console.error;
  const codes: number[] = [];
  const errs: string[] = [];
  process.exit = ((c?: number) => {
    codes.push(c ?? 0);
    throw new Error('__exit__');
  }) as typeof process.exit;
  console.error = (...a: unknown[]) => {
    errs.push(a.join(' '));
  };
  try {
    expect(() => parseArgs(args)).toThrow('__exit__');
    expect(codes).toEqual([1]);
    expect(errs.join('\n')).toContain(needle);
  } finally {
    process.exit = origExit;
    console.error = origErr;
  }
}

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

describe('output format flags', () => {
  it('defaults format to waybar, watch false, interval 60', () => {
    const o = parseArgs([]);
    expect(o.format).toBe('waybar');
    expect(o.watch).toBe(false);
    expect(o.intervalSeconds).toBe(60);
  });

  it('parses --format json', () => {
    expect(parseArgs(['--format', 'json']).format).toBe('json');
  });

  it('--watch implies json', () => {
    const o = parseArgs(['--watch']);
    expect(o.watch).toBe(true);
    expect(o.format).toBe('json');
  });

  it('parses --interval', () => {
    expect(parseArgs(['--watch', '--interval', '30']).intervalSeconds).toBe(30);
  });

  it('exits 1 on invalid --format', () => {
    expectExit1(['--format', 'xml'], '--format must be');
  });

  it('exits 1 on invalid --interval', () => {
    expectExit1(['--watch', '--interval', 'abc'], '--interval must be');
  });

  it('exits 1 on --watch with explicit --format waybar', () => {
    expectExit1(['--watch', '--format', 'waybar'], '--watch requires --format json');
  });

  it('exits 1 on --interval 1.5 (non-integer)', () => {
    expectExit1(['--watch', '--interval', '1.5'], '--interval must be');
  });

  it('exits 1 on --interval 0', () => {
    expectExit1(['--watch', '--interval', '0'], '--interval must be');
  });

  it('warns (no exit) on --interval without --watch', () => {
    const origErr = console.error;
    const errs: string[] = [];
    console.error = (...a: unknown[]) => {
      errs.push(a.join(' '));
    };
    try {
      const o = parseArgs(['--interval', '30']);
      expect(o.watch).toBe(false);
      expect(errs.join('\n')).toContain('--interval has no effect without --watch');
    } finally {
      console.error = origErr;
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
