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
  /**
   * Provider-supplied "used" percentage, when it is NOT simply `100 - remaining`.
   * Copilot derives this from usedRequests/entitlementRequests (can exceed 100%
   * with overage). Renderers prefer this in `used` display mode and fall back to
   * `100 - remaining` when undefined. See `toWindowDisplay`.
   */
  used?: number | null;
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
 * Core quota fields shared by all providers
 */
interface QuotaCore {
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
  /** Additional quota windows (for providers with multiple models) */
  models?: Record<string, QuotaWindow>;
}

export interface ClaudeQuotaExtra {
  /** Per-model weekly quotas (Claude Pro feature) */
  weeklyModels?: Record<string, QuotaWindow>;
  /** Extra Usage / additional budget (Claude Pro feature) */
  extraUsage?: {
    enabled: boolean;
    remaining: number;
    limit: number;
    used: number;
  };
}

export interface CodexQuotaExtra {
  /** Multi-window model data (5h/7d/other) */
  modelsDetailed?: Record<string, ModelWindows>;
  /** Credits / extra usage data */
  extraUsage?: {
    enabled: boolean;
    remaining: number;
    limit: number;
    used: number;
  };
}

export interface AmpQuotaExtra {
  /** Arbitrary key-value metadata for provider-specific display */
  meta?: Record<string, string>;
}

export interface CopilotQuotaSnapshot {
  /** Whether this quota bucket is unlimited for the active account */
  isUnlimitedEntitlement: boolean;
  /** Included request allowance for this bucket */
  entitlementRequests: number;
  /** Requests consumed in the current billing/reset window */
  usedRequests: number;
  /** Whether Copilot still allows usage after this bucket is exhausted */
  usageAllowedWithExhaustedQuota: boolean;
  /** Billed/overage request count when provided by Copilot */
  overage: number;
  /** Whether overage is allowed after the quota is exhausted */
  overageAllowedWithExhaustedQuota: boolean;
  /** Raw remaining percentage as returned by Copilot. Can be negative. */
  remainingPercentage: number;
  /** Copilot reset timestamp for this bucket */
  resetDate: string | null;
  /** Extra flags exposed by the CLI when present */
  hasQuota?: boolean;
  tokenBasedBilling?: boolean;
}

export interface CopilotQuotaExtra {
  /** Arbitrary key-value metadata for provider-specific display */
  meta?: Record<string, string>;
  /** Raw quota snapshots keyed by Copilot bucket, e.g. premium_interactions */
  quotaSnapshots?: Record<string, CopilotQuotaSnapshot>;
}

export interface ClaudeQuota extends QuotaCore {
  provider: 'claude';
  extra?: ClaudeQuotaExtra;
}

export interface CodexQuota extends QuotaCore {
  provider: 'codex';
  extra?: CodexQuotaExtra;
}

export interface AmpQuota extends QuotaCore {
  provider: 'amp';
  extra?: AmpQuotaExtra;
}

export interface CopilotQuota extends QuotaCore {
  provider: 'copilot';
  extra?: CopilotQuotaExtra;
}

export interface GenericQuota extends QuotaCore {
  provider: string;
  extra?: Record<string, unknown>;
}

export type ProviderQuota = ClaudeQuota | CodexQuota | AmpQuota | CopilotQuota | GenericQuota;

/**
 * Provider interface - all providers must implement this
 */
export interface Provider {
  /** Unique identifier */
  readonly id: string;
  /** Display name */
  readonly name: string;
  /** Cache key used for storing/retrieving cached quota data */
  readonly cacheKey: string;

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
