import { afterEach, beforeEach, describe, expect, it, mock, spyOn } from 'bun:test';
import { CONFIG } from '../../src/config';
import type { QuotaBase } from '../../src/providers/base';
import type { ProviderQuota } from '../../src/providers/types';

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

let mockCacheGetOrFetch: ReturnType<typeof mock>;
let mockLoggerError: ReturnType<typeof mock>;

// Mock cache — getOrFetch executes the fetcher directly (bypasses cache)
mock.module('../../src/cache', () => {
  mockCacheGetOrFetch = mock(async (_key: string, fetcher: () => Promise<unknown>, _ttl?: number) => fetcher());
  return {
    cache: {
      getOrFetch: mockCacheGetOrFetch,
    },
  };
});

// Mock logger with no-ops so we can spy on logger.error
mock.module('../../src/logger', () => {
  mockLoggerError = mock(() => {});
  return {
    logger: {
      debug: () => {},
      info: () => {},
      warn: () => {},
      error: mockLoggerError,
    },
  };
});

// Import after mocks are registered
const { BaseProvider } = await import('../../src/providers/base');

// ---------------------------------------------------------------------------
// Fake subclass — each test controls behaviour via instance fields
// ---------------------------------------------------------------------------

class FakeProvider extends BaseProvider {
  readonly id = 'fake';
  readonly name = 'Fake';
  readonly cacheKey = 'fake-quota';

  available = true;
  rawResult: unknown = { data: 'raw' };
  builtQuota: ProviderQuota = {
    provider: 'fake',
    displayName: 'Fake',
    available: true,
  } as ProviderQuota;
  fetchError: Error | unknown | null = null;
  unavailableMsg = 'Fake provider unavailable';

  override isAvailable(): Promise<boolean> {
    return Promise.resolve(this.available);
  }

  protected override fetchRaw(): Promise<unknown> {
    if (this.fetchError !== null) {
      return Promise.reject(this.fetchError);
    }
    return Promise.resolve(this.rawResult);
  }

  protected override buildQuota(_raw: unknown, _base: QuotaBase): ProviderQuota {
    return this.builtQuota;
  }

  protected override unavailableError(): string {
    return this.unavailableMsg;
  }
}

// A subclass that overrides toUserFacingError
class FakeProviderCustomError extends FakeProvider {
  protected override toUserFacingError(_error: unknown): string {
    return 'custom error message';
  }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('BaseProvider', () => {
  let provider: FakeProvider;
  let fetchRawSpy: ReturnType<typeof spyOn>;

  beforeEach(() => {
    provider = new FakeProvider();
    mockCacheGetOrFetch.mockReset();
    mockCacheGetOrFetch.mockImplementation(async (_key: string, fetcher: () => Promise<unknown>) => fetcher());
    mockLoggerError.mockReset();
    fetchRawSpy = spyOn(provider, 'fetchRaw' as any);
  });

  afterEach(() => {
    fetchRawSpy.mockRestore();
  });

  // -----------------------------------------------------------------------
  // Availability gate
  // -----------------------------------------------------------------------

  describe('availability gate', () => {
    it('returns base shape with unavailableError when isAvailable returns false', async () => {
      provider.available = false;
      provider.unavailableMsg = 'Fake provider unavailable';

      const result = await provider.getQuota();

      expect(result.provider).toBe('fake');
      expect(result.displayName).toBe('Fake');
      expect(result.available).toBe(false);
      expect(result.error).toBe('Fake provider unavailable');
    });

    it('does NOT call fetchRaw when isAvailable returns false', async () => {
      provider.available = false;

      await provider.getQuota();

      expect(fetchRawSpy).not.toHaveBeenCalled();
    });

    it('does NOT call cache.getOrFetch when isAvailable returns false', async () => {
      provider.available = false;

      await provider.getQuota();

      expect(mockCacheGetOrFetch).not.toHaveBeenCalled();
    });
  });

  // -----------------------------------------------------------------------
  // Success path
  // -----------------------------------------------------------------------

  describe('success path', () => {
    it('calls buildQuota with raw data and base, returns its result', async () => {
      const rawData = { used: 42, total: 100 };
      const expectedQuota: ProviderQuota = {
        provider: 'fake',
        displayName: 'Fake',
        available: true,
        account: 'user@example.com',
      } as ProviderQuota;

      provider.rawResult = rawData;
      provider.builtQuota = expectedQuota;

      const buildQuotaSpy = spyOn(provider, 'buildQuota' as any);
      buildQuotaSpy.mockReturnValue(expectedQuota);

      const result = await provider.getQuota();

      expect(buildQuotaSpy).toHaveBeenCalledTimes(1);
      const [rawArg, baseArg] = buildQuotaSpy.mock.calls[0];
      expect(rawArg).toEqual(rawData);
      expect(baseArg).toEqual({ provider: 'fake', displayName: 'Fake', available: false });

      expect(result).toBe(expectedQuota);

      buildQuotaSpy.mockRestore();
    });

    it('passes the cacheKey to cache.getOrFetch', async () => {
      await provider.getQuota();

      expect(mockCacheGetOrFetch).toHaveBeenCalledTimes(1);
      const [key] = mockCacheGetOrFetch.mock.calls[0];
      expect(key).toBe('fake-quota');
      expect(mockCacheGetOrFetch.mock.calls[0][2]).toBe(CONFIG.cache.ttlMs);
    });

    it('does not call logger.error on success', async () => {
      await provider.getQuota();

      expect(mockLoggerError).not.toHaveBeenCalled();
    });
  });

  // -----------------------------------------------------------------------
  // Error path
  // -----------------------------------------------------------------------

  describe('error path', () => {
    it('returns base shape with error message when fetchRaw throws an Error', async () => {
      provider.fetchError = new Error('network failure');

      const result = await provider.getQuota();

      expect(result.provider).toBe('fake');
      expect(result.displayName).toBe('Fake');
      expect(result.available).toBe(false);
      expect(result.error).toBe('network failure');
    });

    it('calls logger.error with provider id and the thrown error', async () => {
      const err = new Error('network failure');
      provider.fetchError = err;

      await provider.getQuota();

      expect(mockLoggerError).toHaveBeenCalledTimes(1);
      const [msg, ctx] = mockLoggerError.mock.calls[0];
      expect(msg).toBe('Provider quota fetch error');
      expect(ctx.provider).toBe('fake');
      expect(ctx.error).toBe(err);
    });

    it('returns base shape with "Failed to fetch quota" for non-Error thrown values', async () => {
      provider.fetchError = 'some string error';

      const result = await provider.getQuota();

      expect(result.available).toBe(false);
      expect(result.error).toBe('Failed to fetch quota');
    });

    it('calls logger.error even for non-Error thrown values', async () => {
      provider.fetchError = 42;

      await provider.getQuota();

      expect(mockLoggerError).toHaveBeenCalledTimes(1);
    });
  });

  // -----------------------------------------------------------------------
  // toUserFacingError — default behaviour
  // -----------------------------------------------------------------------

  describe('toUserFacingError (default)', () => {
    it('returns error.message when an Error instance is thrown', async () => {
      provider.fetchError = new Error('the specific error message');

      const result = await provider.getQuota();

      expect(result.error).toBe('the specific error message');
    });

    it('returns "Failed to fetch quota" when a plain string is thrown', async () => {
      provider.fetchError = 'plain string';

      const result = await provider.getQuota();

      expect(result.error).toBe('Failed to fetch quota');
    });

    it('returns "Failed to fetch quota" when null is thrown', async () => {
      provider.fetchError = null;
      // Set to a sentinel so fetchRaw actually rejects with null
      fetchRawSpy.mockImplementation(() => Promise.reject(null));

      const result = await provider.getQuota();

      expect(result.error).toBe('Failed to fetch quota');
    });

    it('returns "Failed to fetch quota" when a plain object is thrown', async () => {
      fetchRawSpy.mockImplementation(() => Promise.reject({ code: 404 }));

      const result = await provider.getQuota();

      expect(result.error).toBe('Failed to fetch quota');
    });
  });

  // -----------------------------------------------------------------------
  // toUserFacingError — subclass override
  // -----------------------------------------------------------------------

  describe('toUserFacingError (subclass override)', () => {
    let customProvider: FakeProviderCustomError;

    beforeEach(() => {
      customProvider = new FakeProviderCustomError();
      customProvider.fetchError = new Error('underlying error');
      mockCacheGetOrFetch.mockImplementation(async (_key: string, fetcher: () => Promise<unknown>) => fetcher());
    });

    it('uses the overridden toUserFacingError when fetchRaw throws', async () => {
      const result = await customProvider.getQuota();

      expect(result.available).toBe(false);
      expect(result.error).toBe('custom error message');
    });

    it('returns provider and displayName from the subclass', async () => {
      const result = await customProvider.getQuota();

      expect(result.provider).toBe('fake');
      expect(result.displayName).toBe('Fake');
    });
  });
});
