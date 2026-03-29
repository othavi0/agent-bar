import { afterEach, beforeEach, describe, expect, it, mock, spyOn } from 'bun:test';
import { EventEmitter } from 'events';
import { Readable, Writable } from 'stream';
import { fakeFile } from '../helpers/mocks';

// ---------------------------------------------------------------------------
// Fake process that simulates codex app-server stdio communication.
// ---------------------------------------------------------------------------

class FakeProcess extends EventEmitter {
  stdout: Readable;
  stdin: Writable;
  private writtenLines: string[] = [];
  private responses: Map<number, object>;
  private autoRespond: boolean;
  killed = false;

  constructor(opts: { responses?: Map<number, object>; autoRespond?: boolean } = {}) {
    super();
    this.responses = opts.responses ?? new Map();
    this.autoRespond = opts.autoRespond ?? true;

    this.stdout = new Readable({ read() {} });
    this.stdin = new Writable({
      write: (chunk, _encoding, callback) => {
        const line = chunk.toString().trim();
        this.writtenLines.push(line);
        try {
          const msg = JSON.parse(line);
          if (this.autoRespond && msg.id !== undefined && this.responses.has(msg.id)) {
            // Small delay to simulate async
            setTimeout(() => {
              this.sendResponse(msg.id, this.responses.get(msg.id)!);
            }, 5);
          }
        } catch {
          // ignore non-json
        }
        callback();
      },
    });
  }

  sendResponse(id: number, result: object) {
    this.stdout.push(`${JSON.stringify({ id, result })}\n`);
  }

  sendLine(line: string) {
    this.stdout.push(`${line}\n`);
  }

  kill() {
    this.killed = true;
  }

  getWrittenMessages(): unknown[] {
    return this.writtenLines.map((l) => JSON.parse(l));
  }
}

// ---------------------------------------------------------------------------
// Module mocks
// ---------------------------------------------------------------------------

let fakeProc: FakeProcess;
let bunFileSpy: ReturnType<typeof spyOn>;

// Mock cache
const cacheGetOrFetchMock = mock<(key: string, fetcher: () => Promise<unknown>, ttl: number) => Promise<unknown>>();

mock.module('../../src/cache', () => ({
  cache: {
    getOrFetch: cacheGetOrFetchMock,
  },
}));

mock.module('../../src/logger', () => ({
  logger: {
    debug: () => {},
    info: () => {},
    warn: () => {},
    error: () => {},
  },
}));

// ---------------------------------------------------------------------------
// Setup
// ---------------------------------------------------------------------------

beforeEach(() => {
  bunFileSpy = spyOn(Bun, 'file').mockReturnValue(fakeFile({ exists: true }) as any);
  // getOrFetch calls the fetcher directly (no caching)
  cacheGetOrFetchMock.mockImplementation(async (_key, fetcher) => fetcher());
});

afterEach(() => {
  bunFileSpy.mockRestore();
  cacheGetOrFetchMock.mockReset();
  if (fakeProc) {
    fakeProc.stdout.destroy();
  }
});

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeResponses(opts: { planType?: string; usedPercent?: number; windowMins?: number; resetsAt?: number }) {
  const initResponse = { capabilities: {} };

  const accountResponse = {
    account: {
      planType: opts.planType ?? 'pro',
    },
  };

  const rateLimitsResponse = {
    rateLimits: {
      limitId: 'codex-default',
      limitName: 'Default',
      primary: {
        usedPercent: opts.usedPercent ?? 30,
        windowDurationMins: opts.windowMins ?? 300,
        resetsAt: opts.resetsAt ?? Math.floor(Date.now() / 1000) + 3600,
      },
      secondary: null,
      planType: opts.planType ?? 'pro',
    },
  };

  const map = new Map<number, object>();
  map.set(0, initResponse);
  map.set(1, accountResponse);
  map.set(2, rateLimitsResponse);
  return map;
}

async function createProviderAndFetch(responses: Map<number, object>) {
  fakeProc = new FakeProcess({ responses, autoRespond: true });

  // Mock child_process.spawn dynamically since codex.ts does `await import('node:child_process')`
  mock.module('node:child_process', () => ({
    spawn: (_cmd: string, _args: string[], _opts: any) => fakeProc,
  }));

  const { CodexProvider } = await import('../../src/providers/codex');
  const p = new CodexProvider();
  return p.getQuota();
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('CodexProvider app-server protocol', () => {
  it('sends initialize message first', async () => {
    const responses = makeResponses({});
    const q = await createProviderAndFetch(responses);

    expect(q.available).toBe(true);
    const messages = fakeProc.getWrittenMessages() as any[];
    expect(messages[0].method).toBe('initialize');
    expect(messages[0].id).toBe(0);
    expect(messages[0].params.clientInfo.name).toBe('agent-bar-omarchy');
  });

  it('sends initialized + account/read + rateLimits/read after init response', async () => {
    const responses = makeResponses({});
    await createProviderAndFetch(responses);

    const messages = fakeProc.getWrittenMessages() as any[];
    // After initialize (msg 0), we expect: initialized, account/read (id:1), rateLimits/read (id:2)
    const methods = messages.map((m) => m.method);
    expect(methods).toContain('initialized');
    expect(methods).toContain('account/read');
    expect(methods).toContain('account/rateLimits/read');
  });

  it('extracts plan type from account/read', async () => {
    const responses = makeResponses({ planType: 'enterprise' });
    const q = await createProviderAndFetch(responses);

    expect(q.plan).toBe('Enterprise');
    expect(q.planType).toBe('enterprise');
  });

  it('parses rate limits into QuotaWindow', async () => {
    const responses = makeResponses({ usedPercent: 40, windowMins: 300 });
    const q = await createProviderAndFetch(responses);

    expect(q.available).toBe(true);
    expect(q.primary?.remaining).toBe(60);
    expect(q.primary?.windowMinutes).toBe(300);
  });

  it('returns null (triggers fallback) on timeout', async () => {
    // Don't respond to any messages
    fakeProc = new FakeProcess({ autoRespond: false });

    mock.module('node:child_process', () => ({
      spawn: () => fakeProc,
    }));

    const { CodexProvider } = await import('../../src/providers/codex');
    const p = new CodexProvider();

    // Override the internal method to use a very short timeout
    const result = await (p as any).fetchRateLimitsViaAppServer(100);
    expect(result).toBeNull();
  });

  it('kills the process after completion', async () => {
    const responses = makeResponses({});
    await createProviderAndFetch(responses);

    expect(fakeProc.killed).toBe(true);
  });

  it('handles process error gracefully', async () => {
    fakeProc = new FakeProcess({ autoRespond: false });

    mock.module('node:child_process', () => ({
      spawn: () => {
        // Emit error after a tick
        setTimeout(() => fakeProc.emit('error', new Error('spawn ENOENT')), 10);
        return fakeProc;
      },
    }));

    const { CodexProvider } = await import('../../src/providers/codex');
    const p = new CodexProvider();

    const result = await (p as any).fetchRateLimitsViaAppServer(500);
    expect(result).toBeNull();
  });

  it('handles process exit before responses', async () => {
    fakeProc = new FakeProcess({ autoRespond: false });

    mock.module('node:child_process', () => ({
      spawn: () => {
        setTimeout(() => fakeProc.emit('exit', 1), 10);
        return fakeProc;
      },
    }));

    const { CodexProvider } = await import('../../src/providers/codex');
    const p = new CodexProvider();

    const result = await (p as any).fetchRateLimitsViaAppServer(500);
    expect(result).toBeNull();
  });
});
