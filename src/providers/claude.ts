import { CONFIG } from '../config';
import { logger } from '../logger';
import type { Provider, ProviderQuota, QuotaWindow } from './types';

interface ClaudeCredentials {
  claudeAiOauth?: {
    accessToken: string;
    subscriptionType?: string;
  };
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
  error?: {
    error_code: string;
    message: string;
  };
}

export class ClaudeProvider implements Provider {
  readonly id = 'claude';
  readonly name = 'Claude';

  async isAvailable(): Promise<boolean> {
    const file = Bun.file(CONFIG.paths.claude.credentials);
    if (!await file.exists()) {
      return false;
    }

    try {
      const creds: ClaudeCredentials = await file.json();
      return !!creds.claudeAiOauth?.accessToken;
    } catch {
      return false;
    }
  }

  async getQuota(): Promise<ProviderQuota> {
    const base: ProviderQuota = {
      provider: this.id,
      displayName: this.name,
      available: false,
    };

    // Check credentials
    const file = Bun.file(CONFIG.paths.claude.credentials);
    if (!await file.exists()) {
      return { ...base, error: 'Not logged in' };
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

    const plan = creds.claudeAiOauth?.subscriptionType || 'unknown';

    // Fetch usage
    try {
      const controller = new AbortController();
      const timeout = setTimeout(() => controller.abort(), CONFIG.api.timeoutMs);

      const response = await fetch(CONFIG.api.claude.usageUrl, {
        headers: {
          'Authorization': `Bearer ${accessToken}`,
          'anthropic-beta': CONFIG.api.claude.betaHeader,
        },
        signal: controller.signal,
      });

      clearTimeout(timeout);

      if (!response.ok) {
        logger.warn('Claude API error', { status: response.status });
        return { ...base, plan, error: `API error: ${response.status}` };
      }

      const usage: ClaudeUsageResponse = await response.json();

      // Check for token expiration
      if (usage.error?.error_code === 'token_expired') {
        return { ...base, plan, error: 'Token expired - please login again' };
      }

      // Parse quota windows
      let primary: QuotaWindow | undefined;
      let secondary: QuotaWindow | undefined;

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

      return {
        ...base,
        available: true,
        plan,
        primary,
        secondary,
      };
    } catch (error) {
      if (error instanceof Error && error.name === 'AbortError') {
        logger.warn('Claude API timeout');
        return { ...base, plan, error: 'Request timeout' };
      }
      logger.error('Claude API fetch error', { error });
      return { ...base, plan, error: 'Failed to fetch usage' };
    }
  }
}
