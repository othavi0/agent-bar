import { cache } from '../cache';
import { CONFIG } from '../config';
import { logger } from '../logger';
import type { Provider, ProviderQuota } from './types';

/** The minimal quota shape available before a successful fetch. */
export interface QuotaBase {
  provider: string;
  displayName: string;
  available: false;
}

/**
 * Abstract base for quota providers. Owns the getQuota() orchestration —
 * base object, availability gate, cache wrapper, error handling — so each
 * concrete provider implements only the parts that genuinely differ.
 */
export abstract class BaseProvider implements Provider {
  abstract readonly id: string;
  abstract readonly name: string;
  abstract readonly cacheKey: string;

  abstract isAvailable(): Promise<boolean>;

  /** Fetch raw provider data. The result is cached under `cacheKey`. */
  protected abstract fetchRaw(): Promise<unknown>;

  /** Convert the (cached) raw data from `fetchRaw` into the final quota. */
  protected abstract buildQuota(raw: unknown, base: QuotaBase): ProviderQuota;

  /** Error message shown when the provider is unavailable. */
  protected abstract unavailableError(): string;

  /** Map a thrown fetch error to a user-facing message. Override as needed. */
  protected toUserFacingError(error: unknown): string {
    return error instanceof Error ? error.message : 'Failed to fetch quota';
  }

  protected buildBase(): QuotaBase {
    return { provider: this.id, displayName: this.name, available: false };
  }

  async getQuota(): Promise<ProviderQuota> {
    const base = this.buildBase();

    if (!(await this.isAvailable())) {
      return { ...base, error: this.unavailableError() } as ProviderQuota;
    }

    try {
      const raw = await cache.getOrFetch(this.cacheKey, () => this.fetchRaw(), CONFIG.cache.ttlMs);
      return this.buildQuota(raw, base);
    } catch (error) {
      logger.error('Provider quota fetch error', { provider: this.id, error });
      return { ...base, error: this.toUserFacingError(error) } as ProviderQuota;
    }
  }
}
