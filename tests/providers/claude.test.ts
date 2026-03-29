import { afterEach, beforeEach, describe, expect, it, mock, spyOn } from 'bun:test';
import { CONFIG } from '../../src/config';
import { fakeFile } from '../helpers/mocks';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Valid credentials payload. */
function validCreds(token = 'tok_abc123', plan = 'pro') {
  return {
    claudeAiOauth: {
      accessToken: token,
      subscriptionType: plan,
    },
  };
}

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

// We need to intercept:
//  1. Bun.file() — to control credential reads
//  2. cache.getOrFetch() — to bypass real cache and exercise the fetcher
//  3. global.fetch — to control API responses

let bunFileSpy: ReturnType<typeof spyOn>;
let originalFetch: typeof global.fetch;

// cache mock — we intercept getOrFetch so the fetcher lambda runs immediately
const cacheGetOrFetchMock = mock<(key: string, fetcher: () => Promise<unknown>, ttl: number) => Promise<unknown>>();

// Use mock.module to replace the cache export
mock.module('../../src/cache', () => ({
  cache: {
    getOrFetch: cacheGetOrFetchMock,
  },
}));

// Suppress logger noise during tests
mock.module('../../src/logger', () => ({
  logger: {
    debug: () => {},
    info: () => {},
    warn: () => {},
    error: () => {},
  },
}));

// ---------------------------------------------------------------------------
// Setup / Teardown
// ---------------------------------------------------------------------------

beforeEach(() => {
  // Bun.file spy — default: file not found
  bunFileSpy = spyOn(Bun, 'file').mockReturnValue(fakeFile({ exists: false }) as any);

  // By default cache.getOrFetch just calls the fetcher (no caching)
  cacheGetOrFetchMock.mockImplementation(async (_key, fetcher, _ttl) => fetcher());

  // Save and replace global fetch
  originalFetch = global.fetch;
});

afterEach(() => {
  bunFileSpy.mockRestore();
  cacheGetOrFetchMock.mockReset();
  global.fetch = originalFetch;
});

// ---------------------------------------------------------------------------
// Helper to get a fresh ClaudeProvider per test (module was mocked above)
// ---------------------------------------------------------------------------

async function createProvider() {
  const { ClaudeProvider } = await import('../../src/providers/claude');
  return new ClaudeProvider();
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('ClaudeProvider', () => {
  // -----------------------------------------------------------------------
  // Identity
  // -----------------------------------------------------------------------
  describe('identity', () => {
    it("has id 'claude', name 'Claude' and cacheKey 'claude-usage'", async () => {
      const p = await createProvider();
      expect(p.id).toBe('claude');
      expect(p.name).toBe('Claude');
      expect(p.cacheKey).toBe('claude-usage');
    });
  });

  // -----------------------------------------------------------------------
  // isAvailable()
  // -----------------------------------------------------------------------
  describe('isAvailable()', () => {
    it('returns true when credentials exist with accessToken', async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true, json: validCreds() }) as any);
      const p = await createProvider();
      expect(await p.isAvailable()).toBe(true);
    });

    it('returns false when credential file does not exist', async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: false }) as any);
      const p = await createProvider();
      expect(await p.isAvailable()).toBe(false);
    });

    it('returns false when JSON is invalid', async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true, throwOnJson: true }) as any);
      const p = await createProvider();
      expect(await p.isAvailable()).toBe(false);
    });

    it('returns false when accessToken is empty string', async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true, json: { claudeAiOauth: { accessToken: '' } } }) as any);
      const p = await createProvider();
      expect(await p.isAvailable()).toBe(false);
    });

    it('returns false when claudeAiOauth is missing', async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true, json: {} }) as any);
      const p = await createProvider();
      expect(await p.isAvailable()).toBe(false);
    });
  });

  // -----------------------------------------------------------------------
  // getQuota() — credential errors
  // -----------------------------------------------------------------------
  describe('getQuota() credential errors', () => {
    it("returns 'Not logged in' when credential file missing", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: false }) as any);
      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.available).toBe(false);
      expect(q.error).toBe('Not logged in. Open `agent-bar-omarchy menu` and choose Provider login.');
      expect(q.provider).toBe('claude');
    });

    it("returns 'Invalid credentials file' when JSON is malformed", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true, throwOnJson: true }) as any);
      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.available).toBe(false);
      expect(q.error).toBe('Invalid credentials file');
    });

    it("returns 'No access token' when token is missing", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true, json: { claudeAiOauth: {} } }) as any);
      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.available).toBe(false);
      expect(q.error).toBe('No access token');
    });

    it("returns 'No access token' when claudeAiOauth is absent", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true, json: {} }) as any);
      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.available).toBe(false);
      expect(q.error).toBe('No access token');
    });
  });

  // -----------------------------------------------------------------------
  // getQuota() — successful responses
  // -----------------------------------------------------------------------
  describe('getQuota() successful parsing', () => {
    /** Sets up Bun.file with valid creds and fetch with the given usage body. */
    function setupSuccess(usageBody: object, plan = 'pro') {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true, json: validCreds('tok', plan) }) as any);

      global.fetch = mock(() =>
        Promise.resolve({
          ok: true,
          json: () => Promise.resolve(usageBody),
        } as Response),
      );
    }

    it('parses five_hour correctly (utilization 30 -> remaining 70)', async () => {
      setupSuccess({
        five_hour: { utilization: 30, resets_at: '2026-03-27T18:00:00Z' },
      });

      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.available).toBe(true);
      expect(q.primary?.remaining).toBe(70);
      expect(q.primary?.resetsAt).toBe('2026-03-27T18:00:00Z');
    });

    it('parses seven_day correctly (utilization 45 -> remaining 55)', async () => {
      setupSuccess({
        seven_day: { utilization: 45, resets_at: '2026-04-03T00:00:00Z' },
      });

      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.available).toBe(true);
      expect(q.secondary?.remaining).toBe(55);
      expect(q.secondary?.resetsAt).toBe('2026-04-03T00:00:00Z');
    });

    it('parses both five_hour and seven_day together', async () => {
      setupSuccess({
        five_hour: { utilization: 10 },
        seven_day: { utilization: 20 },
      });

      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.available).toBe(true);
      expect(q.primary?.remaining).toBe(90);
      expect(q.secondary?.remaining).toBe(80);
    });

    it('parses weekly models (opus, sonnet, cowork)', async () => {
      setupSuccess({
        seven_day_opus: { utilization: 50, resets_at: '2026-04-03T00:00:00Z' },
        seven_day_sonnet: { utilization: 25, resets_at: '2026-04-03T00:00:00Z' },
        seven_day_cowork: { utilization: 80, resets_at: '2026-04-03T00:00:00Z' },
      });

      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.available).toBe(true);
      expect(q.weeklyModels).toBeDefined();
      expect(q.weeklyModels!.Opus.remaining).toBe(50);
      expect(q.weeklyModels!.Sonnet.remaining).toBe(75);
      expect(q.weeklyModels!.Cowork.remaining).toBe(20);
    });

    it('ignores null weekly model fields', async () => {
      setupSuccess({
        seven_day_opus: null,
        seven_day_sonnet: null,
        seven_day_cowork: null,
      });

      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.available).toBe(true);
      expect(q.weeklyModels).toBeUndefined();
    });

    it('parses extra_usage when enabled', async () => {
      setupSuccess({
        extra_usage: {
          is_enabled: true,
          monthly_limit: 100,
          used_credits: 37.5,
          utilization: 37.5,
        },
      });

      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.available).toBe(true);
      expect(q.extraUsage).toBeDefined();
      expect(q.extraUsage!.enabled).toBe(true);
      expect(q.extraUsage!.remaining).toBe(63);
      expect(q.extraUsage!.limit).toBe(100);
      expect(q.extraUsage!.used).toBe(38); // Math.round(37.5)
    });

    it('ignores extra_usage when not enabled', async () => {
      setupSuccess({
        extra_usage: {
          is_enabled: false,
          monthly_limit: 100,
          used_credits: 0,
          utilization: 0,
        },
      });

      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.available).toBe(true);
      expect(q.extraUsage).toBeUndefined();
    });

    it('includes plan from subscriptionType', async () => {
      setupSuccess({}, 'max_5x');

      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.available).toBe(true);
      expect(q.plan).toBe('max_5x');
    });

    it("defaults plan to 'unknown' when subscriptionType is absent", async () => {
      bunFileSpy.mockReturnValue(
        fakeFile({
          exists: true,
          json: { claudeAiOauth: { accessToken: 'tok' } },
        }) as any,
      );

      global.fetch = mock(() =>
        Promise.resolve({
          ok: true,
          json: () => Promise.resolve({}),
        } as Response),
      );

      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.plan).toBe('unknown');
    });

    it('handles utilization rounding (e.g. 33.7 -> remaining 66)', async () => {
      setupSuccess({
        five_hour: { utilization: 33.7 },
      });

      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.primary?.remaining).toBe(66); // 100 - Math.round(33.7)
    });

    it('handles resetsAt null fallback when not provided', async () => {
      setupSuccess({
        five_hour: { utilization: 0 },
      });

      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.primary?.resetsAt).toBeNull();
    });
  });

  // -----------------------------------------------------------------------
  // getQuota() — error responses
  // -----------------------------------------------------------------------
  describe('getQuota() error handling', () => {
    function setupCreds(plan = 'pro') {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true, json: validCreds('tok', plan) }) as any);
    }

    it('detects token_expired and returns error', async () => {
      setupCreds();
      global.fetch = mock(() =>
        Promise.resolve({
          ok: true,
          json: () =>
            Promise.resolve({
              error: { error_code: 'token_expired', message: 'Token expired' },
            }),
        } as Response),
      );

      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.available).toBe(false);
      expect(q.error).toBe('Token expired. Open `agent-bar-omarchy menu` and choose Provider login.');
      expect(q.plan).toBe('pro');
    });

    it("treats AbortError as 'Request timeout'", async () => {
      setupCreds();

      // Make the fetcher throw an AbortError (simulating controller.abort())
      cacheGetOrFetchMock.mockImplementation(async (_key, _fetcher, _ttl) => {
        const err = new DOMException('The operation was aborted.', 'AbortError');
        throw err;
      });

      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.available).toBe(false);
      expect(q.error).toBe('Request timeout');
      expect(q.plan).toBe('pro');
    });

    it('treats HTTP 4xx/5xx as API error', async () => {
      setupCreds();
      global.fetch = mock(() =>
        Promise.resolve({
          ok: false,
          status: 429,
          json: () => Promise.resolve({}),
        } as Response),
      );

      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.available).toBe(false);
      expect(q.error).toBe('Claude API error: 429');
    });

    it('treats HTTP 500 as API error', async () => {
      setupCreds();
      global.fetch = mock(() =>
        Promise.resolve({
          ok: false,
          status: 500,
          json: () => Promise.resolve({}),
        } as Response),
      );

      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.available).toBe(false);
      expect(q.error).toContain('Claude API error: 500');
    });

    it("treats unexpected errors as 'Failed to fetch Claude usage'", async () => {
      setupCreds();

      cacheGetOrFetchMock.mockImplementation(async () => {
        throw new TypeError('fetch failed');
      });

      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.available).toBe(false);
      expect(q.error).toBe('Failed to fetch Claude usage');
    });
  });

  // -----------------------------------------------------------------------
  // getQuota() — fetch contract
  // -----------------------------------------------------------------------
  describe('getQuota() fetch contract', () => {
    it('sends correct Authorization header and beta header', async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true, json: validCreds('my-secret-token') }) as any);

      const fetchMock = mock(() =>
        Promise.resolve({
          ok: true,
          json: () => Promise.resolve({}),
        } as Response),
      );
      global.fetch = fetchMock;

      const p = await createProvider();
      await p.getQuota();

      expect(fetchMock).toHaveBeenCalledTimes(1);

      const [url, opts] = fetchMock.mock.calls[0] as [string, RequestInit];
      expect(url).toBe(CONFIG.api.claude.usageUrl);
      expect(opts.headers).toEqual({
        Authorization: 'Bearer my-secret-token',
        'anthropic-beta': CONFIG.api.claude.betaHeader,
      });
      expect(opts.signal).toBeDefined();
    });

    it("passes cache key 'claude-usage' and CONFIG.cache.ttlMs to getOrFetch", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true, json: validCreds() }) as any);

      global.fetch = mock(() =>
        Promise.resolve({
          ok: true,
          json: () => Promise.resolve({}),
        } as Response),
      );

      const p = await createProvider();
      await p.getQuota();

      expect(cacheGetOrFetchMock).toHaveBeenCalledTimes(1);
      const [key, _fetcher, ttl] = cacheGetOrFetchMock.mock.calls[0] as [string, unknown, number];
      expect(key).toBe('claude-usage');
      expect(ttl).toBe(CONFIG.cache.ttlMs);
    });
  });
});
