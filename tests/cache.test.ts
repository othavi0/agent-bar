import { afterEach, beforeEach, describe, expect, it } from 'bun:test';
import { existsSync } from 'fs';
import { mkdir, readFile, rm, writeFile } from 'fs/promises';
import { tmpdir } from 'os';
import { join } from 'path';
import { Cache } from '../src/cache';

const TEST_DIR = join(tmpdir(), `agent-bar-omarchy-cache-test-${Date.now()}`);

describe('Cache', () => {
  let cache: Cache;

  beforeEach(async () => {
    await mkdir(TEST_DIR, { recursive: true });
    cache = new Cache(TEST_DIR);
  });

  afterEach(async () => {
    await rm(TEST_DIR, { recursive: true, force: true });
  });

  describe('key validation', () => {
    it('accepts alphanumeric keys with hyphens', async () => {
      await cache.set('claude-usage', { ok: true });
      const result = await cache.get('claude-usage');
      expect(result).toEqual({ ok: true });
    });

    it('accepts alphanumeric keys with underscores', async () => {
      await cache.set('codex_quota', { ok: true });
      const result = await cache.get('codex_quota');
      expect(result).toEqual({ ok: true });
    });

    it('rejects path traversal attempts', () => {
      expect(() => cache.get('../etc/passwd')).toThrow('Invalid cache key');
    });

    it('rejects keys with spaces', () => {
      expect(() => cache.get('bad key')).toThrow('Invalid cache key');
    });

    it('rejects empty keys', () => {
      expect(() => cache.get('')).toThrow('Invalid cache key');
    });

    it('rejects keys with slashes', () => {
      expect(() => cache.get('foo/bar')).toThrow('Invalid cache key');
    });

    it('rejects keys with dots', () => {
      expect(() => cache.get('foo.bar')).toThrow('Invalid cache key');
    });
  });

  describe('get()', () => {
    it('returns null when file does not exist', async () => {
      const result = await cache.get('nonexistent');
      expect(result).toBeNull();
    });

    it('returns data when cache is valid', async () => {
      const data = { usage: 42, provider: 'claude' };
      await cache.set('test-key', data, 60_000);

      const result = await cache.get('test-key');
      expect(result).toEqual(data);
    });

    it('returns null when cache is expired', async () => {
      // Write an already-expired entry directly
      const entry = {
        data: { stale: true },
        fetchedAt: Date.now() - 10_000,
        expiresAt: Date.now() - 5_000,
      };
      const path = join(TEST_DIR, 'expired-key.json');
      await writeFile(path, JSON.stringify(entry));

      const result = await cache.get('expired-key');
      expect(result).toBeNull();
    });

    it('returns null when file contains invalid JSON', async () => {
      const path = join(TEST_DIR, 'bad-json.json');
      await writeFile(path, 'this is not json {{{');

      const result = await cache.get('bad-json');
      expect(result).toBeNull();
    });
  });

  describe('set() + get() roundtrip', () => {
    it('stores and retrieves data correctly', async () => {
      const data = { remaining: 85, limit: 100 };
      await cache.set('roundtrip', data, 60_000);

      const result = await cache.get('roundtrip');
      expect(result).toEqual(data);
    });

    it('writes valid CacheEntry JSON to disk', async () => {
      const data = { count: 7 };
      await cache.set('disk-check', data, 30_000);

      const path = join(TEST_DIR, 'disk-check.json');
      expect(existsSync(path)).toBe(true);

      const raw = await readFile(path, 'utf-8');
      const entry = JSON.parse(raw);

      expect(entry.data).toEqual(data);
      expect(typeof entry.fetchedAt).toBe('number');
      expect(typeof entry.expiresAt).toBe('number');
      expect(entry.expiresAt).toBeGreaterThan(entry.fetchedAt);
    });

    it('respects TTL - data expires after ttlMs', async () => {
      // Set with very short TTL
      await cache.set('short-ttl', { value: 1 }, 1);

      // Small delay to ensure expiration
      await Bun.sleep(10);

      const result = await cache.get('short-ttl');
      expect(result).toBeNull();
    });

    it('data remains valid within TTL window', async () => {
      await cache.set('long-ttl', { value: 99 }, 60_000);

      const result = await cache.get('long-ttl');
      expect(result).toEqual({ value: 99 });
    });

    it('handles complex nested data', async () => {
      const data = {
        quotas: [
          { provider: 'claude', pct: 85.5 },
          { provider: 'codex', pct: null },
        ],
        meta: { timestamp: '2026-03-27T00:00:00Z' },
      };

      await cache.set('nested', data, 60_000);
      const result = await cache.get('nested');
      expect(result).toEqual(data);
    });
  });

  describe('invalidate()', () => {
    it('removes an existing cache file', async () => {
      await cache.set('to-delete', { data: true }, 60_000);

      // Confirm it exists
      const before = await cache.get('to-delete');
      expect(before).toEqual({ data: true });

      await cache.invalidate('to-delete');

      const after = await cache.get('to-delete');
      expect(after).toBeNull();
    });

    it('does not throw when file does not exist', async () => {
      // Should silently succeed
      await cache.invalidate('nonexistent-key');
    });

    it('file is actually removed from disk', async () => {
      await cache.set('disk-del', { x: 1 }, 60_000);
      const path = join(TEST_DIR, 'disk-del.json');
      expect(existsSync(path)).toBe(true);

      await cache.invalidate('disk-del');
      expect(existsSync(path)).toBe(false);
    });
  });

  describe('getOrFetch()', () => {
    it('returns cached data without calling fetcher', async () => {
      await cache.set('prefilled', { cached: true }, 60_000);

      let fetcherCalled = false;
      const result = await cache.getOrFetch('prefilled', async () => {
        fetcherCalled = true;
        return { cached: false };
      });

      expect(result).toEqual({ cached: true });
      expect(fetcherCalled).toBe(false);
    });

    it('calls fetcher and caches result on cache miss', async () => {
      const result = await cache.getOrFetch('fresh-fetch', async () => ({ fresh: true }), 60_000);

      expect(result).toEqual({ fresh: true });

      // Verify it was cached
      const cached = await cache.get('fresh-fetch');
      expect(cached).toEqual({ fresh: true });
    });

    it('deduplicates concurrent fetches - fetcher runs only once', async () => {
      let fetchCount = 0;

      const fetcher = async () => {
        fetchCount++;
        // Simulate async work so both calls hit inflight
        await Bun.sleep(50);
        return { count: fetchCount };
      };

      // Fire two concurrent requests for the same key
      const [r1, r2] = await Promise.all([
        cache.getOrFetch('dedup-key', fetcher, 60_000),
        cache.getOrFetch('dedup-key', fetcher, 60_000),
      ]);

      expect(fetchCount).toBe(1);
      expect(r1).toEqual({ count: 1 });
      expect(r2).toEqual({ count: 1 });
    });

    it('re-throws fetcher errors', async () => {
      const failingFetcher = async () => {
        throw new Error('API down');
      };

      expect(cache.getOrFetch('fail-key', failingFetcher, 60_000)).rejects.toThrow('API down');
    });

    it('cleans up inflight on error - next call retries', async () => {
      let callCount = 0;

      const fetcher = async () => {
        callCount++;
        if (callCount === 1) {
          throw new Error('first attempt fails');
        }
        return { recovered: true };
      };

      // First call fails
      try {
        await cache.getOrFetch('retry-key', fetcher, 60_000);
      } catch {
        // expected
      }

      // Second call should retry (inflight was cleaned up)
      const result = await cache.getOrFetch('retry-key', fetcher, 60_000);
      expect(result).toEqual({ recovered: true });
      expect(callCount).toBe(2);
    });

    it('concurrent calls all reject when fetcher fails', async () => {
      const fetcher = async () => {
        await Bun.sleep(50);
        throw new Error('concurrent failure');
      };

      const results = await Promise.allSettled([
        cache.getOrFetch('concurrent-fail', fetcher, 60_000),
        cache.getOrFetch('concurrent-fail', fetcher, 60_000),
      ]);

      expect(results[0].status).toBe('rejected');
      expect(results[1].status).toBe('rejected');
    });
  });

  describe('ensureDir()', () => {
    it('creates cache directory if it does not exist', async () => {
      const freshDir = join(TEST_DIR, 'sub', 'nested');
      const freshCache = new Cache(freshDir);

      await freshCache.ensureDir();
      expect(existsSync(freshDir)).toBe(true);
    });

    it('does not throw if directory already exists', async () => {
      await cache.ensureDir();
      // Call again - should be idempotent
      await cache.ensureDir();
    });

    it('moves legacy cache files into the new directory', async () => {
      const nextDir = join(TEST_DIR, 'new-cache');
      const legacyDir = join(TEST_DIR, 'qbar-cache');
      const legacyWaybarDir = join(TEST_DIR, 'legacy-waybar-cache');
      const migrated = new Cache(nextDir, [legacyDir, legacyWaybarDir]);

      await mkdir(legacyDir, { recursive: true });
      await writeFile(
        join(legacyDir, 'claude-usage.json'),
        JSON.stringify({
          data: { remaining: 87 },
          fetchedAt: Date.now(),
          expiresAt: Date.now() + 60_000,
        }),
      );

      await migrated.ensureDir();

      expect(existsSync(join(nextDir, 'claude-usage.json'))).toBe(true);
      expect(existsSync(legacyDir)).toBe(false);
    });
  });
});
