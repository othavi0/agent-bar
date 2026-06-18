import { APP_NAME } from '../app-identity';
import { cache } from '../cache';
import { CONFIG } from '../config';
import { logger } from '../logger';
import { registerProvider } from './registry';
import type { ClaudeQuota, Provider, QuotaWindow } from './types';

interface ClaudeCredentials {
  claudeAiOauth?: {
    accessToken: string;
    subscriptionType?: string;
    /** OAuth rate-limit tier, e.g. `default_claude_max_5x` — carries the Max multiplier. */
    rateLimitTier?: string;
    /** Access-token expiry as epoch milliseconds. */
    expiresAt?: number;
  };
}

/**
 * Resolve the display plan from the subscription type plus the OAuth
 * `rateLimitTier`. `subscriptionType` is coarse ("max"); the tier carries the
 * multiplier ("default_claude_max_5x"), so we surface "max 5x" / "max 20x" when
 * present. Plans without a multiplier (Pro, Free) are returned untouched.
 */
export function deriveClaudePlan(subscriptionType?: string, rateLimitTier?: string): string {
  const sub = subscriptionType?.trim();
  if (!sub) return 'unknown';
  const mult = rateLimitTier?.match(/_(\d+)x$/i)?.[1];
  if (mult && !sub.toLowerCase().includes(`${mult}x`)) {
    return `${sub} ${mult}x`;
  }
  return sub;
}

interface ClaudeUsageResponse {
  five_hour?: {
    utilization: number;
    resets_at?: string;
  };
  seven_day?: {
    utilization: number;
    resets_at?: string;
  };
  seven_day_opus?: {
    utilization: number;
    resets_at?: string;
  } | null;
  seven_day_sonnet?: {
    utilization: number;
    resets_at?: string;
  } | null;
  seven_day_cowork?: {
    utilization: number;
    resets_at?: string;
  } | null;
  extra_usage?: {
    is_enabled: boolean;
    monthly_limit: number;
    used_credits: number;
    utilization: number;
  };
  error?: {
    error_code: string;
    message: string;
  };
}

export class ClaudeProvider implements Provider {
  readonly id = 'claude';
  readonly name = 'Claude';
  readonly cacheKey = 'claude-usage';

  async isAvailable(): Promise<boolean> {
    const file = Bun.file(CONFIG.paths.claude.credentials);
    if (!(await file.exists())) {
      return false;
    }

    try {
      const creds: ClaudeCredentials = await file.json();
      return !!creds.claudeAiOauth?.accessToken;
    } catch {
      return false;
    }
  }

  async getQuota(): Promise<ClaudeQuota> {
    const base: ClaudeQuota = {
      provider: 'claude',
      displayName: this.name,
      available: false,
    };

    // Check credentials
    const file = Bun.file(CONFIG.paths.claude.credentials);
    if (!(await file.exists())) {
      return { ...base, error: `Not logged in. Open \`${APP_NAME} menu\` and choose Provider login.` };
    }

    let creds: ClaudeCredentials;
    try {
      creds = await file.json();
    } catch (error) {
      logger.error('Failed to parse Claude credentials', { error });
      return { ...base, error: 'Invalid credentials file' };
    }

    const accessToken = creds.claudeAiOauth?.accessToken;
    if (!accessToken) {
      return { ...base, error: 'No access token' };
    }

    const plan = deriveClaudePlan(creds.claudeAiOauth?.subscriptionType, creds.claudeAiOauth?.rateLimitTier);

    // Proactive expiry: a locally-expired access token will be rejected by the API
    // anyway, and we must not refresh it ourselves (single-use refresh token races
    // Claude Code). Short-circuit the guaranteed-failing request.
    const expiresAt = creds.claudeAiOauth?.expiresAt;
    if (typeof expiresAt === 'number' && expiresAt <= Date.now()) {
      return { ...base, plan, error: `Token expired. Open \`${APP_NAME} menu\` and choose Provider login.` };
    }

    // Fetch usage (cached)
    try {
      const usage = await cache.getOrFetch<ClaudeUsageResponse>(
        'claude-usage',
        async () => {
          const controller = new AbortController();
          const timeout = setTimeout(() => controller.abort(), CONFIG.api.timeoutMs);

          const response = await fetch(CONFIG.api.claude.usageUrl, {
            headers: {
              Authorization: `Bearer ${accessToken}`,
              'anthropic-beta': CONFIG.api.claude.betaHeader,
              'User-Agent': CONFIG.api.claude.userAgent,
            },
            signal: controller.signal,
          });

          if (!response.ok) {
            // keep non-200 out of cache
            throw new Error(`Claude API error: ${response.status}`);
          }

          // Keep the abort timer armed through the body read: a server that
          // sends headers fast then stalls the body would otherwise hang.
          const data = await response.json();
          clearTimeout(timeout);
          return data;
        },
        CONFIG.cache.ttlMs,
      );

      // Check for token expiration
      if (usage.error?.error_code === 'token_expired') {
        return { ...base, plan, error: `Token expired. Open \`${APP_NAME} menu\` and choose Provider login.` };
      }

      // Parse quota windows
      let primary: QuotaWindow | undefined;
      let secondary: QuotaWindow | undefined;
      const weeklyModels: Record<string, QuotaWindow> = {};
      let extraUsage: { enabled: boolean; remaining: number; limit: number; used: number } | undefined;

      if (usage.five_hour) {
        const used = Math.round(usage.five_hour.utilization);
        primary = {
          remaining: 100 - used,
          resetsAt: usage.five_hour.resets_at || null,
        };
      }

      if (usage.seven_day) {
        const used = Math.round(usage.seven_day.utilization);
        secondary = {
          remaining: 100 - used,
          resetsAt: usage.seven_day.resets_at || null,
        };
      }

      if (usage.seven_day_opus) {
        const used = Math.round(usage.seven_day_opus.utilization);
        weeklyModels.Opus = {
          remaining: 100 - used,
          resetsAt: usage.seven_day_opus.resets_at || null,
        };
      }

      if (usage.seven_day_sonnet) {
        const used = Math.round(usage.seven_day_sonnet.utilization);
        weeklyModels.Sonnet = {
          remaining: 100 - used,
          resetsAt: usage.seven_day_sonnet.resets_at || null,
        };
      }

      if (usage.seven_day_cowork) {
        const used = Math.round(usage.seven_day_cowork.utilization);
        weeklyModels.Cowork = {
          remaining: 100 - used,
          resetsAt: usage.seven_day_cowork.resets_at || null,
        };
      }

      // Parse Extra Usage (new field)
      if (usage.extra_usage?.is_enabled) {
        const utilization = usage.extra_usage.utilization;
        extraUsage = {
          enabled: true,
          remaining: Math.round(100 - utilization),
          limit: usage.extra_usage.monthly_limit,
          used: Math.round(usage.extra_usage.used_credits),
        };
      }

      const extra: import('./types').ClaudeQuotaExtra = {};
      if (Object.keys(weeklyModels).length > 0) extra.weeklyModels = weeklyModels;
      if (extraUsage) extra.extraUsage = extraUsage;

      return {
        ...base,
        available: true,
        plan,
        primary,
        secondary,
        ...(Object.keys(extra).length > 0 ? { extra } : {}),
      };
    } catch (error) {
      if (error instanceof Error && error.name === 'AbortError') {
        logger.warn('Claude API timeout');
        return { ...base, plan, error: 'Request timeout' };
      }
      // cache.getOrFetch throws for non-200; map to a clean message
      if (error instanceof Error && error.message.startsWith('Claude API error:')) {
        logger.warn('Claude API error', { message: error.message });
        return { ...base, plan, error: error.message };
      }
      logger.error('Claude API fetch error', { error });
      return { ...base, plan, error: 'Failed to fetch Claude usage' };
    }
  }
}

registerProvider(new ClaudeProvider());
