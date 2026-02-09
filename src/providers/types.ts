/**
 * Quota information for a single time window (e.g., 5h, 7d)
 */
export interface QuotaWindow {
  /** Percentage remaining (0-100) */
  remaining: number;
  /** ISO timestamp when quota resets */
  resetsAt: string | null;
  /** Window length in minutes (if provided by provider) */
  windowMinutes?: number | null;
}

/**
 * Full quota data from a provider
 */
export interface ProviderQuota {
  /** Provider identifier */
  provider: string;
  /** Display name for UI */
  displayName: string;
  /** Whether the provider is authenticated/available */
  available: boolean;
  /** Account identifier (email, username, etc.) */
  account?: string;
  /** Subscription plan (if applicable) */
  plan?: string;
  /** Error message if fetch failed */
  error?: string;
  /** Primary quota window (usually daily/5h) */
  primary?: QuotaWindow;
  /** Secondary quota window (usually weekly/7d) */
  secondary?: QuotaWindow;
  /** Per-model weekly quotas (Claude Pro feature) */
  weeklyModels?: Record<string, QuotaWindow>;
  /** Additional quota windows (for providers with multiple models) */
  models?: Record<string, QuotaWindow>;
  /** Extra Usage (Claude Pro feature) */
  extraUsage?: {
    enabled: boolean;
    remaining: number;
    limit: number;
    used: number;
  };
  /** Arbitrary key-value metadata for provider-specific display */
  meta?: Record<string, string>;
}

/**
 * Provider interface - all providers must implement this
 */
export interface Provider {
  /** Unique identifier */
  readonly id: string;
  /** Display name */
  readonly name: string;
  
  /**
   * Check if provider is available (has credentials)
   */
  isAvailable(): Promise<boolean>;
  
  /**
   * Fetch current quota information
   */
  getQuota(): Promise<ProviderQuota>;
}

/**
 * Cache entry with metadata
 */
export interface CacheEntry<T> {
  data: T;
  fetchedAt: number;
  expiresAt: number;
}

/**
 * Aggregated quota data from all providers
 */
export interface AllQuotas {
  providers: ProviderQuota[];
  fetchedAt: string;
}
