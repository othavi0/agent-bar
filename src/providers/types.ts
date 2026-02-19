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
 * Canonical window buckets per model/provider limit.
 * Missing windows should be interpreted as unavailable and rendered as N/A.
 */
export interface ModelWindows {
  fiveHour?: QuotaWindow;
  sevenDay?: QuotaWindow;
  other?: QuotaWindow[];
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
  /** Raw provider-specific plan identifier (if applicable) */
  planType?: string;
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
  /** Multi-window model data (5h/7d/other) */
  modelsDetailed?: Record<string, ModelWindows>;
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
