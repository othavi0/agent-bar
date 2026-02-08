import { CONFIG } from '../config';
import { logger } from '../logger';
import { cache } from '../cache';
import type { Provider, ProviderQuota, QuotaWindow } from './types';

// Token storage (from antigravity-usage or qbar's own OAuth)
interface TokenData {
  accessToken: string;
  refreshToken?: string;
  expiresAt: number;
  email: string;
  projectId?: string;
}

interface AntigravityConfig {
  version: string;
  activeAccount: string;
  preferences?: { cacheTTL?: number };
}

// API response types
interface QuotaInfo {
  remainingFraction: number;  // 0.0 - 1.0
  resetTime?: string;
  isExhausted?: boolean;
}

interface ModelInfo {
  displayName?: string;
  model?: string;
  quotaInfo?: QuotaInfo;
  modelProvider?: string;
}

interface FetchAvailableModelsResponse {
  models?: Record<string, ModelInfo>;
  defaultAgentModelId?: string;
}

// Model ID to friendly label mapping
const MODEL_LABELS: Record<string, string> = {
  'claude-opus-4-5': 'Claude Opus 4.5',
  'claude-opus-4-5-thinking': 'Claude Opus 4.5 (Thinking)',
  'claude-sonnet-4-5': 'Claude Sonnet 4.5',
  'claude-sonnet-4-5-thinking': 'Claude Sonnet 4.5 (Thinking)',
  'gemini-2.5-flash': 'Gemini 2.5 Flash',
  'gemini-2.5-flash-thinking': 'Gemini 2.5 Flash (Thinking)',
  'gemini-2.5-pro': 'Gemini 2.5 Pro',
  'gemini-3-flash': 'Gemini 3 Flash',
  'gemini-3-pro-high': 'Gemini 3 Pro (High)',
  'gemini-3-pro-low': 'Gemini 3 Pro (Low)',
  'gemini-3-pro-image': 'Gemini 3 Pro Image',
  'gpt-oss-120b-medium': 'GPT-OSS 120B (Medium)',
};

function getModelLabel(modelId: string, displayName?: string): string {
  // Prefer the API's displayName, fallback to our mapping
  return displayName || MODEL_LABELS[modelId] || modelId;
}

export class AntigravityProvider implements Provider {
  readonly id = 'antigravity';
  readonly name = 'Antigravity';
  private readonly GOOGLE_CLIENT_ID = '590579400786-a7rn11flab8l5b7maoq7hg0akn06aoc7.apps.googleusercontent.com';

  private antigravityUsagePath = `${process.env.HOME}/.config/antigravity-usage`;
  private qbarAuthPath = `${CONFIG.paths.config}/auth.json`;

  private async refreshAccessToken(tokens: TokenData, source: 'antigravity-usage' | 'qbar'): Promise<TokenData | null> {
    if (!tokens.refreshToken) return null;

    try {
      const response = await fetch('https://oauth2.googleapis.com/token', {
        method: 'POST',
        headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
        body: new URLSearchParams({
          client_id: this.GOOGLE_CLIENT_ID,
          grant_type: 'refresh_token',
          refresh_token: tokens.refreshToken,
        }),
      });

      if (!response.ok) {
        logger.debug('Token refresh failed', { status: response.status });
        return null;
      }

      const data: any = await response.json();
      const refreshed: TokenData = {
        ...tokens,
        accessToken: data.access_token,
        expiresAt: Date.now() + (data.expires_in * 1000),
      };

      if (data.refresh_token) {
        refreshed.refreshToken = data.refresh_token;
      }

      await this.saveTokens(refreshed, source);
      return refreshed;
    } catch (error) {
      logger.debug('Token refresh error', { error });
      return null;
    }
  }

  private async saveTokens(tokens: TokenData, source: 'antigravity-usage' | 'qbar'): Promise<void> {
    try {
      if (source === 'antigravity-usage') {
        const configFile = Bun.file(`${this.antigravityUsagePath}/config.json`);
        const config: AntigravityConfig = await configFile.json();
        const tokenPath = `${this.antigravityUsagePath}/accounts/${config.activeAccount}/tokens.json`;
        await Bun.write(tokenPath, JSON.stringify(tokens, null, 2));
      } else {
        const file = Bun.file(this.qbarAuthPath);
        let auth: any = {};
        if (await file.exists()) auth = await file.json();
        auth.antigravity = tokens;
        await Bun.write(this.qbarAuthPath, JSON.stringify(auth, null, 2));
      }
    } catch (error) {
      logger.debug('Failed to save refreshed tokens', { error });
    }
  }

  async isAvailable(): Promise<boolean> {
    const tokens = await this.getTokens();
    return tokens !== null;
  }

  /**
   * Try to get tokens from antigravity-usage or qbar's own auth
   */
  private async getTokens(): Promise<TokenData | null> {
    // 1. Try antigravity-usage tokens first
    try {
      const configFile = Bun.file(`${this.antigravityUsagePath}/config.json`);
      if (await configFile.exists()) {
        const config: AntigravityConfig = await configFile.json();
        if (config.activeAccount) {
          const tokenFile = Bun.file(
            `${this.antigravityUsagePath}/accounts/${config.activeAccount}/tokens.json`
          );
          if (await tokenFile.exists()) {
            const tokens: TokenData = await tokenFile.json();
            // Check if expired
            if (tokens.expiresAt > Date.now()) {
              return tokens;
            }
            logger.debug('antigravity-usage tokens expired, attempting refresh');
            const refreshed = await this.refreshAccessToken(tokens, 'antigravity-usage');
            if (refreshed) return refreshed;
          }
        }
      }
    } catch (error) {
      logger.debug('Failed to read antigravity-usage tokens', { error });
    }

    // 2. Try qbar's own auth
    try {
      const file = Bun.file(this.qbarAuthPath);
      if (await file.exists()) {
        const auth = await file.json();
        if (auth.antigravity?.accessToken) {
          if (auth.antigravity.expiresAt > Date.now()) {
            return auth.antigravity;
          }
          logger.debug('qbar antigravity tokens expired, attempting refresh');
          const refreshed = await this.refreshAccessToken(auth.antigravity, 'qbar');
          if (refreshed) return refreshed;
        }
      }
    } catch (error) {
      logger.debug('Failed to read qbar auth', { error });
    }

    return null;
  }

  async getQuota(): Promise<ProviderQuota> {
    const base: ProviderQuota = {
      provider: this.id,
      displayName: this.name,
      available: false,
    };

    const tokens = await this.getTokens();
    if (!tokens) {
      return { ...base, error: 'Not logged in - use qbar menu → Provider login' };
    }

    // Use cache
    const cacheKey = `antigravity-quota-${tokens.email}`;

    try {
      return await cache.getOrFetch<ProviderQuota>(
        cacheKey,
        async () => await this.fetchQuotaFromAPI(base, tokens),
        CONFIG.cache.antigravityTtlMs
      );
    } catch (error) {
      logger.error('Antigravity quota fetch error', { error });
      return { ...base, account: tokens.email, error: 'Failed to fetch quota' };
    }
  }

  private async fetchQuotaFromAPI(base: ProviderQuota, tokens: TokenData): Promise<ProviderQuota> {
    try {
      const controller = new AbortController();
      const timeout = setTimeout(() => controller.abort(), CONFIG.api.timeoutMs);

      // Build request body - include project if available
      const body: Record<string, string> = {};
      if (tokens.projectId) {
        body.project = tokens.projectId;
      }

      const response = await fetch(
        'https://cloudcode-pa.googleapis.com/v1internal:fetchAvailableModels',
        {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
            'Authorization': `Bearer ${tokens.accessToken}`,
            'User-Agent': 'antigravity',
          },
          body: JSON.stringify(body),
          signal: controller.signal,
        }
      );

      clearTimeout(timeout);

      if (!response.ok) {
        if (response.status === 401 || response.status === 403) {
          return { ...base, account: tokens.email, error: 'Token expired - please login again' };
        }
        throw new Error(`API error: ${response.status}`);
      }

      const data: FetchAvailableModelsResponse = await response.json();
      
      if (!data.models || Object.keys(data.models).length === 0) {
        return { ...base, account: tokens.email, error: 'No model quotas available' };
      }

      // Convert models to our format
      const models: Record<string, QuotaWindow> = {};
      
      for (const [modelId, info] of Object.entries(data.models)) {
        const label = getModelLabel(modelId, info.displayName);
        const quota = info.quotaInfo;
        
        if (quota) {
          // Convert remainingFraction (0.0-1.0) to percentage (0-100)
          // If remainingFraction is null/undefined, treat as 0% (exhausted)
          // If isExhausted is true, also 0%
          let remaining: number;
          if (quota.isExhausted || quota.remainingFraction === null || quota.remainingFraction === undefined) {
            remaining = 0;
          } else {
            remaining = Math.round(quota.remainingFraction * 100);
          }
          
          models[label] = {
            remaining,
            resetsAt: quota.resetTime || null,
          };
        }
      }

      // Find primary (prefer Claude Opus Thinking)
      const primaryKey = Object.keys(models).find(k => k.includes('Claude Opus') && k.includes('Thinking'))
        || Object.keys(models).find(k => k.includes('Claude'))
        || Object.keys(models).find(k => k.includes('Gemini 3 Pro'))
        || Object.keys(models)[0];

      return {
        ...base,
        available: true,
        account: tokens.email,
        primary: primaryKey ? models[primaryKey] : undefined,
        models,
      };
    } catch (error) {
      if (error instanceof Error && error.name === 'AbortError') {
        logger.warn('Antigravity API timeout');
        return { ...base, account: tokens.email, error: 'Request timeout' };
      }
      logger.error('Antigravity API error', { error });
      return { ...base, account: tokens.email, error: 'Failed to fetch quota' };
    }
  }
}
