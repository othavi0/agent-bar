import { mkdir, rename, unlink } from 'fs/promises';
import { join } from 'path';
import { CONFIG } from './config';
import { logger } from './logger';
import type { CacheEntry } from './providers/types';

/**
 * Simple file-based cache with TTL
 */
export class Cache {
  private cacheDir: string;
  /** In-flight fetch deduplication: key → Promise */
  private inflight = new Map<string, Promise<unknown>>();

  constructor(cacheDir: string = CONFIG.paths.cache) {
    this.cacheDir = cacheDir;
  }

  private getPath(key: string): string {
    // Prevent path traversal: only allow safe characters in cache keys
    if (!/^[a-zA-Z0-9_-]+$/.test(key)) {
      throw new Error(`Invalid cache key: "${key}"`);
    }
    return join(this.cacheDir, `${key}.json`);
  }

  async ensureDir(): Promise<void> {
    try {
      await mkdir(this.cacheDir, { recursive: true });
    } catch (error) {
      logger.error('Failed to create cache directory', { error, dir: this.cacheDir });
    }
  }

  /**
   * Get cached data if valid, null otherwise
   */
  async get<T>(key: string): Promise<T | null> {
    const path = this.getPath(key);
    const file = Bun.file(path);

    if (!(await file.exists())) {
      logger.debug('Cache miss (not found)', { key });
      return null;
    }

    try {
      const entry: CacheEntry<T> = await file.json();
      const now = Date.now();

      if (now > entry.expiresAt) {
        logger.debug('Cache miss (expired)', { key, expiredAgo: now - entry.expiresAt });
        return null;
      }

      logger.debug('Cache hit', { key, age: now - entry.fetchedAt });
      return entry.data;
    } catch (error) {
      logger.warn('Cache read error', { key, error });
      return null;
    }
  }

  /**
   * Store data in cache with TTL
   */
  async set<T>(key: string, data: T, ttlMs: number = CONFIG.cache.ttlMs): Promise<void> {
    await this.ensureDir();

    const path = this.getPath(key);
    const now = Date.now();

    const entry: CacheEntry<T> = {
      data,
      fetchedAt: now,
      expiresAt: now + ttlMs,
    };

    try {
      // Atomic write: each provider runs as a separate `agent-bar --provider X`
      // process, so two Waybar polls can write the same key concurrently. Write
      // to a unique temp file then rename (atomic on the same filesystem) so a
      // crash or concurrent writer never leaves a partially-written cache file.
      const tmp = `${path}.${process.pid}.tmp`;
      await Bun.write(tmp, JSON.stringify(entry, null, 2));
      await rename(tmp, path);
      logger.debug('Cache write', { key, ttlMs });
    } catch (error) {
      logger.error('Cache write error', { key, error });
    }
  }

  /**
   * Invalidate a cache entry
   */
  async invalidate(key: string): Promise<void> {
    const path = this.getPath(key);

    try {
      await unlink(path);
      logger.debug('Cache invalidated', { key });
    } catch {
      // File doesn't exist or can't delete - that's fine
      logger.debug('Cache invalidate (no file)', { key });
    }
  }

  /**
   * Get or fetch: returns cached data if valid, otherwise fetches and caches.
   * Deduplicates concurrent requests for the same key.
   */
  async getOrFetch<T>(key: string, fetcher: () => Promise<T>, ttlMs: number = CONFIG.cache.ttlMs): Promise<T> {
    const cached = await this.get<T>(key);
    if (cached !== null) {
      return cached;
    }

    // Deduplicate concurrent fetches for the same key
    const existing = this.inflight.get(key);
    if (existing) {
      return existing as Promise<T>;
    }

    const promise = fetcher()
      .then(async (data) => {
        await this.set(key, data, ttlMs);
        this.inflight.delete(key);
        return data;
      })
      .catch((err) => {
        this.inflight.delete(key);
        throw err;
      });

    this.inflight.set(key, promise);
    return promise as Promise<T>;
  }
}

// Global cache instance
export const cache = new Cache();
