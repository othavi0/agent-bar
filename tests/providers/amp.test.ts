import { afterEach, beforeEach, describe, expect, it, mock, spyOn } from 'bun:test';
import type { ProviderQuota } from '../../src/providers/types';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeMockProc(stdout: string, exitCode: number) {
  return {
    stdout: new Response(stdout).body,
    stderr: new Response('').body,
    exited: Promise.resolve(exitCode),
  };
}

const FULL_OUTPUT = [
  'Signed in as user@email.com',
  'Amp Free: $3.50/$5.00 remaining',
  'replenishes +$0.25/hour',
  '+20% bonus for 5 more days',
  'Individual credits: $10.00 remaining',
].join('\n');

const OUTPUT_NO_BONUS = [
  'Signed in as user@email.com',
  'Amp Free: $3.50/$5.00 remaining',
  'replenishes +$0.25/hour',
].join('\n');

const OUTPUT_NO_REPLENISH = ['Signed in as user@email.com', 'Amp Free: $3.50/$5.00 remaining'].join('\n');

const OUTPUT_ZERO_CREDITS = [
  'Signed in as user@email.com',
  'Amp Free: $3.50/$5.00 remaining',
  'replenishes +$0.25/hour',
  'Individual credits: $0.00 remaining',
].join('\n');

const OUTPUT_FULL_QUOTA = [
  'Signed in as user@email.com',
  'Amp Free: $5.00/$5.00 remaining',
  'replenishes +$0.25/hour',
].join('\n');

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

let mockFindAmpBin: ReturnType<typeof mock>;
let mockCacheGetOrFetch: ReturnType<typeof mock>;
let spawnSpy: ReturnType<typeof spyOn> | null = null;

// Mock findAmpBin — default: returns a fake path
mock.module('../../src/amp-cli', () => {
  mockFindAmpBin = mock(() => '/usr/bin/amp');
  return {
    findAmpBin: mockFindAmpBin,
    AMP_MISSING_ERROR: 'Amp CLI not installed. Right-click to install and log in.',
  };
});

// Mock cache — getOrFetch executes the fetcher directly (bypasses cache)
mock.module('../../src/cache', () => {
  mockCacheGetOrFetch = mock(async (_key: string, fetcher: () => Promise<unknown>, _ttl?: number) => fetcher());
  return {
    cache: {
      getOrFetch: mockCacheGetOrFetch,
    },
  };
});

// Import after mocks are registered so the module picks them up
const { AmpProvider } = await import('../../src/providers/amp');

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('AmpProvider', () => {
  let provider: InstanceType<typeof AmpProvider>;

  beforeEach(() => {
    provider = new AmpProvider();
    mockFindAmpBin.mockReset();
    mockFindAmpBin.mockReturnValue('/usr/bin/amp');
    mockCacheGetOrFetch.mockReset();
    mockCacheGetOrFetch.mockImplementation(async (_key: string, fetcher: () => Promise<unknown>) => fetcher());
  });

  afterEach(() => {
    if (spawnSpy) {
      spawnSpy.mockRestore();
      spawnSpy = null;
    }
  });

  // -----------------------------------------------------------------------
  // Static properties
  // -----------------------------------------------------------------------

  it('has correct id, name, and cacheKey', () => {
    expect(provider.id).toBe('amp');
    expect(provider.name).toBe('Amp');
    expect(provider.cacheKey).toBe('amp-quota');
  });

  // -----------------------------------------------------------------------
  // isAvailable
  // -----------------------------------------------------------------------

  describe('isAvailable', () => {
    it('returns true when findAmpBin returns a path', async () => {
      mockFindAmpBin.mockReturnValue('/usr/bin/amp');
      expect(await provider.isAvailable()).toBe(true);
    });

    it('returns false when findAmpBin returns null', async () => {
      mockFindAmpBin.mockReturnValue(null);
      expect(await provider.isAvailable()).toBe(false);
    });
  });

  // -----------------------------------------------------------------------
  // getQuota — binary not found
  // -----------------------------------------------------------------------

  describe('getQuota when binary is missing', () => {
    it('returns available:false with AMP_MISSING_ERROR', async () => {
      mockFindAmpBin.mockReturnValue(null);

      const result = await provider.getQuota();

      expect(result.available).toBe(false);
      expect(result.error).toBe('Amp CLI not installed. Right-click to install and log in.');
      expect(result.provider).toBe('amp');
      expect(result.displayName).toBe('Amp');
    });
  });

  // -----------------------------------------------------------------------
  // getQuota — non-zero exit code
  // -----------------------------------------------------------------------

  describe('getQuota when amp exits with error', () => {
    it('returns available:false with "Not logged in"', async () => {
      spawnSpy = spyOn(Bun, 'spawn').mockReturnValue(makeMockProc('some error output', 1) as any);

      const result = await provider.getQuota();

      expect(result.available).toBe(false);
      expect(result.error).toBe('Not logged in. Open `agent-bar-omarchy menu` and choose Provider login.');
    });
  });

  // -----------------------------------------------------------------------
  // getQuota — no "Signed in" line
  // -----------------------------------------------------------------------

  describe('getQuota when stdout has no Signed in line', () => {
    it('returns available:false with "Not logged in"', async () => {
      spawnSpy = spyOn(Bun, 'spawn').mockReturnValue(
        makeMockProc('Some unexpected output\nwithout sign-in info', 0) as any,
      );

      const result = await provider.getQuota();

      expect(result.available).toBe(false);
      expect(result.error).toBe('Not logged in. Open `agent-bar-omarchy menu` and choose Provider login.');
    });
  });

  // -----------------------------------------------------------------------
  // getQuota — full output parsing
  // -----------------------------------------------------------------------

  describe('getQuota with full output', () => {
    let result: ProviderQuota;

    beforeEach(async () => {
      spawnSpy = spyOn(Bun, 'spawn').mockReturnValue(makeMockProc(FULL_OUTPUT, 0) as any);
      result = await provider.getQuota();
    });

    it('is available', () => {
      expect(result.available).toBe(true);
    });

    it('parses account', () => {
      expect(result.account).toBe('user@email.com');
    });

    it('calculates primary remaining percentage', () => {
      // 3.5 / 5.0 = 70%
      expect(result.primary?.remaining).toBe(70);
    });

    it('sets primary.resetsAt (fullAt) as ISO string', () => {
      expect(result.primary?.resetsAt).toBeDefined();
      // Must be a valid ISO date
      const date = new Date(result.primary!.resetsAt!);
      expect(date.getTime()).not.toBeNaN();
      // fullAt should be in the future
      expect(date.getTime()).toBeGreaterThan(Date.now());
    });

    it('populates models["Free Tier"]', () => {
      expect(result.models).toBeDefined();
      expect(result.models!['Free Tier']).toBeDefined();
      expect(result.models!['Free Tier'].remaining).toBe(70);
    });

    it('populates meta fields', () => {
      expect(result.meta).toBeDefined();
      expect(result.meta!.freeRemaining).toBe('$3.5');
      expect(result.meta!.freeTotal).toBe('$5');
      expect(result.meta!.replenishRate).toBe('+$0.25/hr');
      expect(result.meta!.bonus).toBe('+20% (5d)');
    });

    it('sets extraUsage.enabled when credits > 0', () => {
      expect(result.extraUsage).toBeDefined();
      expect(result.extraUsage!.enabled).toBe(true);
      expect(result.extraUsage!.remaining).toBe(100);
    });

    it('populates models["Credits"]', () => {
      expect(result.models!.Credits).toBeDefined();
      expect(result.models!.Credits.remaining).toBe(100);
    });

    it('populates meta creditsBalance', () => {
      expect(result.meta!.creditsBalance).toBe('$10');
    });
  });

  // -----------------------------------------------------------------------
  // getQuota — output without bonus
  // -----------------------------------------------------------------------

  describe('getQuota without bonus line', () => {
    let result: ProviderQuota;

    beforeEach(async () => {
      spawnSpy = spyOn(Bun, 'spawn').mockReturnValue(makeMockProc(OUTPUT_NO_BONUS, 0) as any);
      result = await provider.getQuota();
    });

    it('is available', () => {
      expect(result.available).toBe(true);
    });

    it('meta does not contain bonus', () => {
      expect(result.meta!.bonus).toBeUndefined();
    });

    it('still has replenishRate', () => {
      expect(result.meta!.replenishRate).toBe('+$0.25/hr');
    });

    it('does not include extraUsage when no credits line', () => {
      expect(result.extraUsage).toBeUndefined();
    });

    it('ETA is calculated without bonus multiplier', () => {
      // Without bonus: (5.0 - 3.5) / 0.25 = 6 hours
      const fullAt = new Date(result.primary!.resetsAt!);
      const hoursToFull = (fullAt.getTime() - Date.now()) / 3_600_000;
      // Allow some tolerance for time elapsed during test (~6 hours)
      expect(hoursToFull).toBeGreaterThan(5.5);
      expect(hoursToFull).toBeLessThan(6.5);
    });
  });

  // -----------------------------------------------------------------------
  // getQuota — output without replenish
  // -----------------------------------------------------------------------

  describe('getQuota without replenish line', () => {
    let result: ProviderQuota;

    beforeEach(async () => {
      spawnSpy = spyOn(Bun, 'spawn').mockReturnValue(makeMockProc(OUTPUT_NO_REPLENISH, 0) as any);
      result = await provider.getQuota();
    });

    it('is available', () => {
      expect(result.available).toBe(true);
    });

    it('meta does not contain replenishRate', () => {
      expect(result.meta!.replenishRate).toBeUndefined();
    });

    it('meta does not contain bonus', () => {
      expect(result.meta!.bonus).toBeUndefined();
    });

    it('primary.resetsAt is null (no ETA without replenish)', () => {
      expect(result.primary?.resetsAt).toBeNull();
    });
  });

  // -----------------------------------------------------------------------
  // Credits parsing
  // -----------------------------------------------------------------------

  describe('credits parsing', () => {
    it('sets extraUsage.enabled when credits > 0', async () => {
      spawnSpy = spyOn(Bun, 'spawn').mockReturnValue(makeMockProc(FULL_OUTPUT, 0) as any);

      const result = await provider.getQuota();
      expect(result.extraUsage).toBeDefined();
      expect(result.extraUsage!.enabled).toBe(true);
    });

    it('does not set extraUsage when no credits line', async () => {
      spawnSpy = spyOn(Bun, 'spawn').mockReturnValue(makeMockProc(OUTPUT_NO_BONUS, 0) as any);

      const result = await provider.getQuota();
      expect(result.extraUsage).toBeUndefined();
    });

    it('does not set extraUsage when credits balance is $0.00', async () => {
      spawnSpy = spyOn(Bun, 'spawn').mockReturnValue(makeMockProc(OUTPUT_ZERO_CREDITS, 0) as any);

      const result = await provider.getQuota();
      expect(result.extraUsage).toBeUndefined();
    });

    it('models["Credits"] has remaining 0 when balance is $0.00', async () => {
      spawnSpy = spyOn(Bun, 'spawn').mockReturnValue(makeMockProc(OUTPUT_ZERO_CREDITS, 0) as any);

      const result = await provider.getQuota();
      expect(result.models!.Credits).toBeDefined();
      expect(result.models!.Credits.remaining).toBe(0);
    });
  });

  // -----------------------------------------------------------------------
  // ETA calculation
  // -----------------------------------------------------------------------

  describe('fullAt ETA calculation', () => {
    it('calculates ETA with bonus multiplier', async () => {
      spawnSpy = spyOn(Bun, 'spawn').mockReturnValue(makeMockProc(FULL_OUTPUT, 0) as any);

      const result = await provider.getQuota();
      const fullAt = new Date(result.primary!.resetsAt!);
      // With bonus: effectiveRate = 0.25 * 1.20 = 0.30
      // hoursToFull = (5.0 - 3.5) / 0.30 = 5.0 hours
      const hoursToFull = (fullAt.getTime() - Date.now()) / 3_600_000;
      expect(hoursToFull).toBeGreaterThan(4.5);
      expect(hoursToFull).toBeLessThan(5.5);
    });

    it('calculates ETA without bonus', async () => {
      spawnSpy = spyOn(Bun, 'spawn').mockReturnValue(makeMockProc(OUTPUT_NO_BONUS, 0) as any);

      const result = await provider.getQuota();
      const fullAt = new Date(result.primary!.resetsAt!);
      // Without bonus: hoursToFull = (5.0 - 3.5) / 0.25 = 6.0 hours
      const hoursToFull = (fullAt.getTime() - Date.now()) / 3_600_000;
      expect(hoursToFull).toBeGreaterThan(5.5);
      expect(hoursToFull).toBeLessThan(6.5);
    });

    it('returns null resetsAt when quota is already full', async () => {
      spawnSpy = spyOn(Bun, 'spawn').mockReturnValue(makeMockProc(OUTPUT_FULL_QUOTA, 0) as any);

      const result = await provider.getQuota();
      // remaining == total, so no ETA needed
      expect(result.primary?.remaining).toBe(100);
      expect(result.primary?.resetsAt).toBeNull();
    });

    it('returns null resetsAt when no replenish rate', async () => {
      spawnSpy = spyOn(Bun, 'spawn').mockReturnValue(makeMockProc(OUTPUT_NO_REPLENISH, 0) as any);

      const result = await provider.getQuota();
      expect(result.primary?.resetsAt).toBeNull();
    });
  });

  // -----------------------------------------------------------------------
  // Bun.spawn is called with correct args
  // -----------------------------------------------------------------------

  describe('spawn invocation', () => {
    it('calls Bun.spawn with [bin, "usage"] and pipe options', async () => {
      spawnSpy = spyOn(Bun, 'spawn').mockReturnValue(makeMockProc(FULL_OUTPUT, 0) as any);

      await provider.getQuota();

      expect(spawnSpy).toHaveBeenCalledTimes(1);
      const [args, opts] = spawnSpy.mock.calls[0];
      expect(args).toEqual(['/usr/bin/amp', 'usage']);
      expect(opts.stdout).toBe('pipe');
      expect(opts.stderr).toBe('pipe');
      expect(opts.env.NO_COLOR).toBe('1');
      expect(opts.env.TERM).toBe('dumb');
    });
  });

  // -----------------------------------------------------------------------
  // cache.getOrFetch integration
  // -----------------------------------------------------------------------

  describe('cache integration', () => {
    it('passes "amp-quota" as the cache key', async () => {
      spawnSpy = spyOn(Bun, 'spawn').mockReturnValue(makeMockProc(FULL_OUTPUT, 0) as any);

      await provider.getQuota();

      expect(mockCacheGetOrFetch).toHaveBeenCalledTimes(1);
      const [key] = mockCacheGetOrFetch.mock.calls[0];
      expect(key).toBe('amp-quota');
    });

    it('returns cached result when cache hits', async () => {
      const cachedResult: ProviderQuota = {
        provider: 'amp',
        displayName: 'Amp',
        available: true,
        account: 'cached@email.com',
      };

      mockCacheGetOrFetch.mockResolvedValue(cachedResult);

      const result = await provider.getQuota();
      expect(result.account).toBe('cached@email.com');
    });
  });

  // -----------------------------------------------------------------------
  // Error handling
  // -----------------------------------------------------------------------

  describe('error handling', () => {
    it('returns "Failed to fetch Amp usage" when cache.getOrFetch throws', async () => {
      mockCacheGetOrFetch.mockRejectedValue(new Error('network failure'));

      const result = await provider.getQuota();

      expect(result.available).toBe(false);
      expect(result.error).toBe('Failed to fetch Amp usage');
    });
  });
});
