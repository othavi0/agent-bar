import { afterEach, beforeEach, describe, expect, it, mock, spyOn } from 'bun:test';
import { EventEmitter } from 'node:events';
import { Readable, Writable } from 'node:stream';
import type { ProviderQuota } from '../../src/providers/types';
import { fakeFile } from '../helpers/mocks';

class FakeCopilotProcess extends EventEmitter {
  stdout = new Readable({ read() {} });
  stderr = new Readable({ read() {} });
  written = '';
  killed = false;

  stdin = new Writable({
    write: (chunk, _encoding, callback) => {
      this.written += chunk.toString();
      callback();
    },
  });

  sendMessage(message: object) {
    const body = JSON.stringify(message);
    this.stdout.push(`Content-Length: ${Buffer.byteLength(body, 'utf8')}\r\n\r\n${body}`);
  }

  sendMalformedFrame() {
    this.stdout.push('Content-Length: 8\r\n\r\nnot-json');
  }

  kill() {
    this.killed = true;
  }

  getWrittenMessages(): unknown[] {
    const messages: unknown[] = [];
    let rest = this.written;

    while (rest.length > 0) {
      const headerEnd = rest.indexOf('\r\n\r\n');
      if (headerEnd === -1) break;

      const header = rest.slice(0, headerEnd);
      const match = header.match(/Content-Length:\s*(\d+)/i);
      if (!match) break;

      const length = Number.parseInt(match[1], 10);
      const bodyStart = headerEnd + 4;
      const body = rest.slice(bodyStart, bodyStart + length);
      messages.push(JSON.parse(body));
      rest = rest.slice(bodyStart + length);
    }

    return messages;
  }
}

const QUOTA_RESULT = {
  quotaSnapshots: {
    chat: {
      isUnlimitedEntitlement: true,
      entitlementRequests: 0,
      usedRequests: 0,
      usageAllowedWithExhaustedQuota: false,
      overage: 0,
      overageAllowedWithExhaustedQuota: false,
      remainingPercentage: 100,
      resetDate: '2026-05-11T20:00:37.668Z',
      hasQuota: false,
      tokenBasedBilling: false,
    },
    completions: {
      isUnlimitedEntitlement: true,
      entitlementRequests: 0,
      usedRequests: 0,
      usageAllowedWithExhaustedQuota: false,
      overage: 0,
      overageAllowedWithExhaustedQuota: false,
      remainingPercentage: 100,
      resetDate: '2026-05-11T20:00:37.668Z',
      hasQuota: false,
      tokenBasedBilling: false,
    },
    premium_interactions: {
      isUnlimitedEntitlement: false,
      entitlementRequests: 300,
      usedRequests: 698,
      usageAllowedWithExhaustedQuota: true,
      overage: 0,
      overageAllowedWithExhaustedQuota: true,
      remainingPercentage: -132.8,
      resetDate: '2026-05-11T20:00:37.668Z',
      hasQuota: false,
      tokenBasedBilling: false,
    },
  },
};

let mockFindCopilotBin: ReturnType<typeof mock>;
let mockCacheGetOrFetch: ReturnType<typeof mock>;
let fakeProc: FakeCopilotProcess;
const mockSpawn = mock(() => fakeProc);

mock.module('../../src/config', () => ({
  CONFIG: {
    paths: {
      copilot: {
        home: '/tmp/agent-bar-copilot-test',
        config: '/tmp/agent-bar-copilot-test/config.json',
        settings: '/tmp/agent-bar-copilot-test/settings.json',
      },
      codex: {
        auth: '/tmp/agent-bar-copilot-test/codex/auth.json',
        sessions: '/tmp/agent-bar-copilot-test/codex/sessions',
      },
      claude: {
        credentials: '/tmp/agent-bar-copilot-test/claude/.credentials.json',
      },
      amp: {
        bin: '/tmp/agent-bar-copilot-test/amp/bin',
        settings: '/tmp/agent-bar-copilot-test/amp/settings.json',
        threads: '/tmp/agent-bar-copilot-test/amp/threads',
      },
      cache: '/tmp/agent-bar-copilot-test/cache',
      config: '/tmp/agent-bar-copilot-test/config',
    },
    cache: {
      ttlMs: 300_000,
      lockTimeoutMs: 5_000,
    },
    api: {
      timeoutMs: 50,
      claude: {
        usageUrl: 'https://api.anthropic.com/api/oauth/usage',
        betaHeader: 'oauth-2025-04-20',
      },
    },
  },
}));

mock.module('../../src/copilot-cli', () => {
  mockFindCopilotBin = mock(() => '/usr/bin/copilot');
  return {
    COPILOT_MISSING_ERROR: 'GitHub Copilot CLI not installed. Install `copilot` and log in.',
    findCopilotBin: mockFindCopilotBin,
  };
});

mock.module('../../src/cache', () => {
  mockCacheGetOrFetch = mock(async (_key: string, fetcher: () => Promise<unknown>, _ttl?: number) => fetcher());
  return {
    cache: {
      getOrFetch: mockCacheGetOrFetch,
    },
  };
});

mock.module('../../src/logger', () => ({
  logger: {
    debug: () => {},
    info: () => {},
    warn: () => {},
    error: () => {},
  },
}));

mock.module('node:child_process', () => {
  return {
    spawn: mockSpawn,
  };
});

const { CopilotProvider } = await import('../../src/providers/copilot');

describe('CopilotProvider', () => {
  let provider: InstanceType<typeof CopilotProvider>;
  let previousToken: string | undefined;

  beforeEach(() => {
    previousToken = process.env.COPILOT_GITHUB_TOKEN;
    process.env.COPILOT_GITHUB_TOKEN = 'test-token';
    provider = new CopilotProvider();
    fakeProc = new FakeCopilotProcess();
    mockFindCopilotBin.mockReset();
    mockFindCopilotBin.mockReturnValue('/usr/bin/copilot');
    mockSpawn.mockReset();
    mockSpawn.mockImplementation(() => fakeProc);
    mockCacheGetOrFetch.mockReset();
    mockCacheGetOrFetch.mockImplementation(async (_key: string, fetcher: () => Promise<unknown>) => fetcher());
  });

  afterEach(() => {
    if (previousToken === undefined) {
      delete process.env.COPILOT_GITHUB_TOKEN;
    } else {
      process.env.COPILOT_GITHUB_TOKEN = previousToken;
    }
    fakeProc.stdout.destroy();
    fakeProc.stderr.destroy();
    fakeProc.stdin.destroy();
  });

  it('has correct id, name, and cacheKey', () => {
    expect(provider.id).toBe('copilot');
    expect(provider.name).toBe('Copilot');
    expect(provider.cacheKey).toBe('copilot-quota');
  });

  it('returns false when the Copilot CLI is unavailable', async () => {
    mockFindCopilotBin.mockReturnValue(null);

    expect(await provider.isAvailable()).toBe(false);
  });

  it('returns true when the Copilot CLI and token env are available', async () => {
    expect(await provider.isAvailable()).toBe(true);
  });

  it('returns true when Copilot config has a JSONC logged-in user', async () => {
    delete process.env.COPILOT_GITHUB_TOKEN;
    const bunFileSpy = spyOn(Bun, 'file').mockReturnValue(
      fakeFile({
        exists: true,
        text: [
          '// User settings belong in settings.json.',
          '// This file is managed automatically.',
          '{ "lastLoggedInUser": { "host": "https://github.com", "login": "octocat" } }',
        ].join('\n'),
      }) as any,
    );

    try {
      expect(await provider.isAvailable()).toBe(true);
    } finally {
      bunFileSpy.mockRestore();
    }
  });

  it('returns a missing CLI error when the binary is unavailable', async () => {
    mockFindCopilotBin.mockReturnValue(null);

    const result = await provider.getQuota();

    expect(result.available).toBe(false);
    expect(result.error).toBe('GitHub Copilot CLI not installed. Install `copilot` and log in.');
  });

  it('parses account.getQuota snapshots', async () => {
    setTimeout(() => fakeProc.sendMessage({ jsonrpc: '2.0', id: 2, result: QUOTA_RESULT }), 1);

    const result = await provider.getQuota();

    expect(result.available).toBe(true);
    expect(result.primary?.remaining).toBe(0);
    expect(result.primary?.resetsAt).toBe('2026-05-11T20:00:37.668Z');
    expect(result.models?.['Premium requests'].remaining).toBe(0);
    expect(result.models?.Chat.remaining).toBe(100);
    expect(result.models?.Completions.remaining).toBe(100);
    expect(result.extra?.quotaSnapshots?.premium_interactions.usedRequests).toBe(698);
    expect(result.extra?.quotaSnapshots?.premium_interactions.entitlementRequests).toBe(300);
    expect(result.extra?.quotaSnapshots?.premium_interactions.remainingPercentage).toBe(-132.8);
  });

  it('uses the cache key and TTL', async () => {
    setTimeout(() => fakeProc.sendMessage({ jsonrpc: '2.0', id: 2, result: QUOTA_RESULT }), 1);

    await provider.getQuota();

    expect(mockCacheGetOrFetch).toHaveBeenCalledTimes(1);
    expect(mockCacheGetOrFetch.mock.calls[0][0]).toBe('copilot-quota');
    expect(mockCacheGetOrFetch.mock.calls[0][2]).toBe(300_000);
  });

  it('spawns Copilot in headless stdio mode', async () => {
    setTimeout(() => fakeProc.sendMessage({ jsonrpc: '2.0', id: 2, result: QUOTA_RESULT }), 1);

    await provider.getQuota();

    expect(mockSpawn).toHaveBeenCalledTimes(1);
    const [cmd, args, opts] = mockSpawn.mock.calls[0];
    expect(cmd).toBe('/usr/bin/copilot');
    expect(args).toEqual(['--headless', '--stdio', '--no-auto-update', '--log-level', 'error']);
    expect(opts.stdio).toEqual(['pipe', 'pipe', 'pipe']);
    expect(opts.env.NO_COLOR).toBe('1');
    expect(opts.env.TERM).toBe('dumb');
  });

  it('sends ping and account.getQuota JSON-RPC frames', async () => {
    setTimeout(() => fakeProc.sendMessage({ jsonrpc: '2.0', id: 2, result: QUOTA_RESULT }), 1);

    await provider.getQuota();

    const messages = fakeProc.getWrittenMessages() as Array<{ id: number; method: string }>;
    expect(messages.map((message) => message.method)).toEqual(['ping', 'account.getQuota']);
  });

  it('returns the login message on auth errors', async () => {
    setTimeout(
      () =>
        fakeProc.sendMessage({
          jsonrpc: '2.0',
          id: 2,
          error: { code: 401, message: 'Unauthorized token' },
        }),
      1,
    );

    const result = await provider.getQuota();

    expect(result.available).toBe(false);
    expect(result.error).toBe('Not logged in. Open `agent-bar menu` and choose Provider login.');
  });

  it('returns a timeout error when the CLI does not answer', async () => {
    const result = await provider.getQuota();

    expect(result.available).toBe(false);
    expect(result.error).toBe('GitHub Copilot CLI timed out while fetching usage');
    expect(fakeProc.killed).toBe(true);
  });

  it('returns a generic error for malformed JSON-RPC frames', async () => {
    setTimeout(() => fakeProc.sendMalformedFrame(), 1);

    const result = await provider.getQuota();

    expect(result.available).toBe(false);
    expect(result.error).toBe('Failed to fetch Copilot usage');
  });

  it('returns cached results when cache hits', async () => {
    const cachedResult: ProviderQuota = {
      provider: 'copilot',
      displayName: 'Copilot',
      available: true,
      primary: { remaining: 42, resetsAt: null },
    };
    mockCacheGetOrFetch.mockResolvedValue(cachedResult);

    const result = await provider.getQuota();

    expect(result.primary?.remaining).toBe(42);
  });
});
